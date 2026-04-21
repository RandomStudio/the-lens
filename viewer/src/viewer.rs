use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, WindowBuilder};

use crate::light::Light;
use crate::mqtt_sender::MqttSender;
use crate::receiver::AngleReceiver;

const CACHE_RADIUS: usize = 50;
const DEFAULT_HUE_OPACITY: f64 = 0.35;

type SbContext = softbuffer::Context<Arc<winit::window::Window>>;
type SbSurface = softbuffer::Surface<Arc<winit::window::Window>, Arc<winit::window::Window>>;

pub struct ImageSequence {
    paths: Arc<Vec<PathBuf>>,
    width: usize,
    height: usize,
    blank: Vec<u32>,
    cache: HashMap<usize, Vec<u32>>,
    in_flight: HashSet<usize>,
    result_tx: mpsc::Sender<(usize, Vec<u32>)>,
    result_rx: mpsc::Receiver<(usize, Vec<u32>)>,
    index_transform: fn(isize, isize) -> isize,
    hue_shift: Option<i32>,
    pub hue_opacity: f64,
    scale: Option<f64>,
}

impl ImageSequence {
    pub fn load(folder: &str, index_transform: fn(isize, isize) -> isize) -> Self {
        let path = Path::new(folder);
        let (tx, rx) = mpsc::channel();

        if !path.exists() {
            eprintln!("[WARN] '{}' does not exist — will show blank frames.", folder);
            return Self::blank_instance(tx, rx);
        }

        let mut entries: Vec<_> = std::fs::read_dir(path)
            .expect("failed to read image folder")
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                n.ends_with(".png") || n.ends_with(".jpg") || n.ends_with(".jpeg")
            })
            .collect();

        entries.sort_by_key(|e| e.file_name());

        if entries.is_empty() {
            eprintln!("[WARN] No images in '{}' — will show blank frames.", folder);
            return Self::blank_instance(tx, rx);
        }

        let paths: Vec<PathBuf> = entries.iter().map(|e| e.path()).collect();
        println!("[INFO] Found {} images in '{}'", paths.len(), folder);

        Self {
            paths: Arc::new(paths),
            width: 0, height: 0, blank: vec![],
            cache: HashMap::new(), in_flight: HashSet::new(),
            result_tx: tx, result_rx: rx,
            index_transform,
            hue_shift: None,
            hue_opacity: DEFAULT_HUE_OPACITY,
            scale: None,
        }
    }

    pub fn empty() -> Self {
        let (tx, rx) = mpsc::channel();
        Self::blank_instance(tx, rx)
    }

    fn blank_instance(
        tx: mpsc::Sender<(usize, Vec<u32>)>,
        rx: mpsc::Receiver<(usize, Vec<u32>)>,
    ) -> Self {
        Self {
            paths: Arc::new(vec![]),
            width: 0, height: 0, blank: vec![],
            cache: HashMap::new(), in_flight: HashSet::new(),
            result_tx: tx, result_rx: rx,
            index_transform: |idx, _n| idx,
            hue_shift: None,
            hue_opacity: DEFAULT_HUE_OPACITY,
            scale: None,
        }
    }

    pub fn hue_shift(mut self, start_hue: i32) -> Self {
        self.hue_shift = Some(start_hue);
        self
    }

    pub fn hue_opacity(mut self, opacity: f64) -> Self {
        self.hue_opacity = opacity;
        self
    }

    pub fn scale(mut self, factor: f64) -> Self {
        self.scale = Some(factor);
        self
    }

    pub fn scale_factor(&self) -> Option<f64> {
        self.scale
    }

    pub fn hue_color_at_angle(&self, angle: f64) -> Option<(u8, u8, u8)> {
        self.hue_shift.map(|start| {
            let hue = (angle + start as f64).rem_euclid(360.0);
            hue_to_rgb(hue)
        })
    }

    pub fn set_index_transform(&mut self, f: fn(isize, isize) -> isize) {
        self.index_transform = f;
    }

    pub fn set_dimensions(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.blank = vec![0u32; width * height];
    }

    pub fn frame_count(&self) -> usize {
        self.paths.len()
    }

    pub fn frame_at_angle(&mut self, angle: f64) -> &[u32] {
        if self.paths.is_empty() {
            return &self.blank;
        }

        let n = self.paths.len();
        let idx = ((angle.rem_euclid(360.0) / 360.0) * n as f64) as usize;
        let idx = (self.index_transform)(idx.min(n - 1) as isize, n as isize)
            .rem_euclid(n as isize) as usize;

        while let Ok((i, frame)) = self.result_rx.try_recv() {
            self.in_flight.remove(&i);
            self.cache.insert(i, frame);
        }

        let window: HashSet<usize> = window_indices(idx, n).collect();
        self.cache.retain(|k, _| window.contains(k));
        self.in_flight.retain(|k| window.contains(k));

        for wi in window_indices(idx, n) {
            if self.cache.contains_key(&wi) { continue; }
            if !self.in_flight.insert(wi) { continue; }
            let paths = Arc::clone(&self.paths);
            let tx = self.result_tx.clone();
            let (w, h) = (self.width, self.height);
            rayon::spawn(move || {
                let frame = decode_frame(&paths[wi], w, h);
                let _ = tx.send((wi, frame));
            });
        }

        if !self.cache.contains_key(&idx) {
            let frame = decode_frame(&self.paths[idx], self.width, self.height);
            self.in_flight.remove(&idx);
            self.cache.insert(idx, frame);
        }

        &self.cache[&idx]
    }
}

fn window_indices(center: usize, n: usize) -> impl Iterator<Item = usize> {
    let n_i = n as isize;
    let c = center as isize;
    let r = CACHE_RADIUS as isize;
    (0..=(2 * r)).map(move |step| {
        let offset = if step == 0 { 0 }
            else if step % 2 == 1 { (step + 1) / 2 }
            else { -(step / 2) };
        ((c + offset).rem_euclid(n_i)) as usize
    })
}

fn decode_frame(path: &Path, width: usize, height: usize) -> Vec<u32> {
    let img = image::open(path)
        .unwrap_or_else(|_| panic!("failed to open {:?}", path))
        .to_rgba8();

    let img_w = img.width() as usize;
    let img_h = img.height() as usize;
    let src_y0 = if img_h > height { (img_h - height) / 2 } else { 0 };
    let dst_y0 = if img_h < height { (height - img_h) / 2 } else { 0 };
    let rows = img_h.min(height);
    let cols = img_w.min(width);

    let mut canvas = vec![0u32; width * height];
    let raw = img.as_raw();
    let stride = img_w * 4;

    for out_row in 0..rows {
        let src = (src_y0 + out_row) * stride;
        let dst = (dst_y0 + out_row) * width;
        for col in 0..cols {
            let p = src + col * 4;
            canvas[dst + col] =
                ((raw[p] as u32) << 16) | ((raw[p + 1] as u32) << 8) | (raw[p + 2] as u32);
        }
    }

    canvas
}

fn hue_to_rgb(hue: f64) -> (u8, u8, u8) {
    let h = hue / 60.0;
    let i = h.floor() as u32 % 6;
    let f = h - h.floor();
    let q = 1.0 - f;
    let (r, g, b): (f64, f64, f64) = match i {
        0 => (1.0, f,   0.0),
        1 => (q,   1.0, 0.0),
        2 => (0.0, 1.0, f  ),
        3 => (0.0, q,   1.0),
        4 => (f,   0.0, 1.0),
        _ => (1.0, 0.0, q  ),
    };
    ((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
}

fn scale_from_center(src: &[u32], w: usize, h: usize, scale: f64) -> Vec<u32> {
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let mut out = vec![0u32; w * h];
    for oy in 0..h {
        for ox in 0..w {
            let sx = (cx + (ox as f64 - cx) / scale).round() as isize;
            let sy = (cy + (oy as f64 - cy) / scale).round() as isize;
            if sx >= 0 && sx < w as isize && sy >= 0 && sy < h as isize {
                out[oy * w + ox] = src[sy as usize * w + sx as usize];
            }
        }
    }
    out
}

fn blend_hue(pixel: u32, (hr, hg, hb): (u8, u8, u8), alpha: f64) -> u32 {
    let ia = 1.0 - alpha;
    let r = (((pixel >> 16) & 0xff) as f64 * ia + hr as f64 * alpha) as u32;
    let g = (((pixel >>  8) & 0xff) as f64 * ia + hg as f64 * alpha) as u32;
    let b = (( pixel        & 0xff) as f64 * ia + hb as f64 * alpha) as u32;
    (r << 16) | (g << 8) | b
}

struct DisplayEntry {
    surface: SbSurface,
    _context: SbContext,
    _window: Arc<winit::window::Window>,
    seq: ImageSequence,
    w: usize,
    h: usize,
}

pub struct Viewer {
    entries: Vec<DisplayEntry>,
}

impl Viewer {
    pub fn new(event_loop: &EventLoop<()>, mut sequences: Vec<(ImageSequence, usize)>) -> Self {
        let mut monitors: Vec<_> = event_loop.available_monitors().collect();
        monitors.sort_by_key(|m| { let p = m.position(); (p.x, p.y) });

        let entries = sequences.drain(..).enumerate().map(|(i, (mut seq, display_idx))| {
            assert!(
                display_idx < monitors.len(),
                "display {} requested but only {} available", display_idx, monitors.len()
            );
            let monitor = monitors[display_idx].clone();
            let size = monitor.size();
            let w = size.width as usize;
            let h = size.height as usize;
            seq.set_dimensions(w, h);

            println!("[INFO] Window {} on display {} ({}x{})", i, display_idx, w, h);

            let window = Arc::new(
                WindowBuilder::new()
                    .with_fullscreen(Some(Fullscreen::Borderless(Some(monitor))))
                    .build(event_loop)
                    .unwrap_or_else(|e| panic!("failed to create window {}: {}", i, e))
            );

            let context = softbuffer::Context::new(window.clone())
                .expect("failed to create softbuffer context");
            let mut surface = softbuffer::Surface::new(&context, window.clone())
                .expect("failed to create softbuffer surface");
            surface.resize(
                NonZeroU32::new(w as u32).unwrap(),
                NonZeroU32::new(h as u32).unwrap(),
            ).expect("failed to resize surface");

            DisplayEntry { surface, _context: context, _window: window, seq, w, h }
        }).collect();

        Self { entries }
    }

    pub fn run(
        mut self,
        event_loop: EventLoop<()>,
        mut receiver: Box<dyn AngleReceiver>,
        light: Option<Light>,
        mqtt_sender: Option<MqttSender>,
    ) {
        let _ = event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::AboutToWait => {
                    let angle = receiver.angle();

                    for entry in &mut self.entries {
                        let hue_color = entry.seq.hue_color_at_angle(angle);
                        let hue_opacity = entry.seq.hue_opacity;
                        let scale = entry.seq.scale_factor();
                        let frame = entry.seq.frame_at_angle(angle);

                        let blended: Vec<u32>;
                        let scaled: Vec<u32>;
                        let buf: &[u32] = if let Some(color) = hue_color {
                            blended = frame.iter().map(|&px| blend_hue(px, color, hue_opacity)).collect();
                            &blended
                        } else {
                            frame
                        };
                        let buf: &[u32] = if let Some(s) = scale {
                            scaled = scale_from_center(buf, entry.w, entry.h, s);
                            &scaled
                        } else {
                            buf
                        };

                        if let Ok(mut sb_buf) = entry.surface.buffer_mut() {
                            sb_buf.copy_from_slice(buf);
                            let _ = sb_buf.present();
                        }
                    }

                    if let Some(ref l) = light { l.update(angle); }
                    if let Some(ref s) = mqtt_sender { s.update(angle); }
                }

                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    elwt.exit();
                }

                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput {
                        event: winit::event::KeyEvent {
                            state: ElementState::Pressed,
                            ref logical_key,
                            ..
                        },
                        ..
                    },
                    ..
                } => {
                    if matches!(logical_key, Key::Named(NamedKey::Escape))
                        || matches!(logical_key, Key::Character(c) if c.as_str() == "q")
                    {
                        elwt.exit();
                    }
                }

                Event::LoopExiting => {
                    if let Some(ref l) = light { l.turn_off(); }
                }

                _ => {}
            }
        });
    }
}
