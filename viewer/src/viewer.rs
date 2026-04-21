use crate::debug_receiver::DebugState;
use minifb::{Key, Window, WindowOptions};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

const DEBUG_WIN_W: usize = 900;
const DEBUG_WIN_H: usize = 450;

const CACHE_RADIUS: usize = 50;

pub fn display_bounds(index: usize) -> (isize, isize, usize, usize) {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::display::CGDisplay;
        let ids = CGDisplay::active_displays().expect("failed to list displays");
        let mut displays: Vec<(isize, isize, usize, usize)> = ids
            .iter()
            .map(|&id| {
                let b = CGDisplay::new(id).bounds();
                (b.origin.x as isize, b.origin.y as isize,
                 b.size.width as usize, b.size.height as usize)
            })
            .collect();
        displays.sort_by_key(|&(x, y, _, _)| (x, y));
        assert!(index < displays.len(),
            "display {} requested but only {} available", index, displays.len());
        return displays[index];
    }
    #[cfg(not(target_os = "macos"))]
    panic!("display_bounds not implemented for this platform");
}

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
    scale: Option<f64>,
    scales_with_rotate: Option<(usize, f64)>,
    brightness_with_rotate: Option<(usize, f64, f64)>,
    pub match_angle: bool,
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
            scale: None,
            scales_with_rotate: None,
            brightness_with_rotate: None,
            match_angle: false,
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
            scale: None,
            scales_with_rotate: None,
            brightness_with_rotate: None,
            match_angle: false,
        }
    }

    pub fn scale(mut self, factor: f64) -> Self {
        self.scale = Some(factor);
        self
    }

    pub fn scale_factor(&self) -> Option<f64> {
        self.scale
    }

    pub fn with_scales_with_rotate(mut self, target_index: usize, scale: f64) -> Self {
        self.scales_with_rotate = Some((target_index, scale));
        self
    }

    pub fn dynamic_scale_at_angle(&self, angle: f64) -> Option<f64> {
        let (target_index, max_scale) = self.scales_with_rotate?;
        let n = self.paths.len();
        if n == 0 { return None; }
        let target_angle = (target_index as f64 / n as f64) * 360.0;
        let delta = (angle - target_angle + 180.0).rem_euclid(360.0) - 180.0;
        let dist = delta.abs();
        let t = 1.0 - dist / 180.0;
        // Cubic ease-in: slow change when far away, accelerates near target
        let t_eased = t * t * t;
        Some(1.0 + (max_scale - 1.0) * t_eased)
    }

    pub fn with_brightness_with_rotate(mut self, target_index: usize, start_brightness: f64, end_brightness: f64) -> Self {
        self.brightness_with_rotate = Some((target_index, start_brightness, end_brightness));
        self
    }

    pub fn dynamic_brightness_at_angle(&self, angle: f64) -> Option<f64> {
        let (target_index, start_brightness, end_brightness) = self.brightness_with_rotate?;
        let n = self.paths.len();
        if n == 0 { return None; }
        let target_angle = (target_index as f64 / n as f64) * 360.0;
        let delta = (angle - target_angle + 180.0).rem_euclid(360.0) - 180.0;
        let dist = delta.abs(); // 0 at target, 180 when fully opposite
        let t = 1.0 - dist / 180.0; // 0 when opposite, 1 at target
        // Quintic ease-in: stays near start_brightness for most of the range,
        // then logarithmic-like acceleration in the final approach to the target
        let t_eased = t.powf(5.0);
        Some(start_brightness + (end_brightness - start_brightness) * t_eased)
    }

    pub fn set_index_transform(&mut self, f: fn(isize, isize) -> isize) {
        self.index_transform = f;
    }

    pub fn set_dimensions(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.blank = vec![0u32; width * height];
        self.cache.clear();
        self.in_flight.clear();
    }

    pub fn frame_count(&self) -> usize {
        self.paths.len()
    }

    pub fn frame_index_at_angle(&self, angle: f64) -> usize {
        if self.paths.is_empty() { return 0; }
        let n = self.paths.len();
        let idx = ((angle.rem_euclid(360.0) / 360.0) * n as f64) as usize;
        (self.index_transform)(idx.min(n - 1) as isize, n as isize)
            .rem_euclid(n as isize) as usize
    }

    pub fn frame_path_at_angle(&self, angle: f64) -> Option<&std::path::Path> {
        if self.paths.is_empty() { return None; }
        Some(&self.paths[self.frame_index_at_angle(angle)])
    }

    pub fn frame_at_angle(&mut self, angle: f64) -> &[u32] {
        if self.paths.is_empty() {
            return &self.blank;
        }

        let n = self.paths.len();
        let idx = self.frame_index_at_angle(angle);
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

    // Scale so width fills exactly; height overflows top/bottom equally (cover behavior).
    let scale = width as f64 / img_w as f64;
    let scaled_h = (img_h as f64 * scale).round() as usize;
    let y_src_offset = if scaled_h > height { (scaled_h - height) / 2 } else { 0 };
    let y_dst_offset = if scaled_h < height { (height - scaled_h) / 2 } else { 0 };
    let visible_rows = scaled_h.min(height);

    let mut canvas = vec![0u32; width * height];
    let raw = img.as_raw();

    for oy in 0..visible_rows {
        let scaled_y = oy + y_src_offset;
        let sy = ((scaled_y as f64 / scale) as usize).min(img_h - 1);
        let dst_row = oy + y_dst_offset;
        for ox in 0..width {
            let sx = ((ox as f64 / scale) as usize).min(img_w - 1);
            let p = (sy * img_w + sx) * 4;
            canvas[dst_row * width + ox] =
                ((raw[p] as u32) << 16) | ((raw[p + 1] as u32) << 8) | (raw[p + 2] as u32);
        }
    }

    canvas
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

fn apply_brightness(pixel: u32, brightness: f64) -> u32 {
    let r = (((pixel >> 16) & 0xff) as f64 * brightness).min(255.0) as u32;
    let g = (((pixel >>  8) & 0xff) as f64 * brightness).min(255.0) as u32;
    let b = (( pixel        & 0xff) as f64 * brightness).min(255.0) as u32;
    (r << 16) | (g << 8) | b
}

fn rotate_frame(src: &[u32], w: usize, h: usize, angle_deg: f64) -> Vec<u32> {
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let rad = -angle_deg.to_radians();
    let (sin, cos) = rad.sin_cos();
    let mut out = vec![0u32; w * h];
    for oy in 0..h {
        for ox in 0..w {
            let dx = ox as f64 - cx;
            let dy = oy as f64 - cy;
            let sx = (cos * dx - sin * dy + cx).round() as isize;
            let sy = (sin * dx + cos * dy + cy).round() as isize;
            if sx >= 0 && sx < w as isize && sy >= 0 && sy < h as isize {
                out[oy * w + ox] = src[sy as usize * w + sx as usize];
            }
        }
    }
    out
}

fn scale_frame_to(src: &[u32], sw: usize, sh: usize, tw: usize, th: usize) -> Vec<u32> {
    if sw == 0 || sh == 0 { return vec![0u32; tw * th]; }
    let mut out = vec![0u32; tw * th];
    for dy in 0..th {
        let sy = (dy * sh) / th;
        for dx in 0..tw {
            let sx = (dx * sw) / tw;
            out[dy * tw + dx] = src[(sy * sw + sx).min(src.len() - 1)];
        }
    }
    out
}

fn draw_text_to_buf(buf: &mut [u32], buf_w: usize, buf_h: usize,
                    font: &fontdue::Font, size: f32,
                    x: i32, baseline_y: i32, text: &str, color: u32) {
    let cr = ((color >> 16) & 0xff) as f32;
    let cg = ((color >>  8) & 0xff) as f32;
    let cb = ( color        & 0xff) as f32;
    let mut cx = x;
    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        let glyph_top = baseline_y - metrics.ymin - metrics.height as i32;
        for row in 0..metrics.height {
            let py = glyph_top + row as i32;
            if py < 0 || py >= buf_h as i32 { continue; }
            for col in 0..metrics.width {
                let px = cx + col as i32 + metrics.xmin;
                if px < 0 || px >= buf_w as i32 { continue; }
                let alpha = bitmap[row * metrics.width + col];
                if alpha == 0 { continue; }
                let a = alpha as f32 / 255.0;
                let idx = py as usize * buf_w + px as usize;
                let bg = buf[idx];
                let br = ((bg >> 16) & 0xff) as f32;
                let bg_g = ((bg >>  8) & 0xff) as f32;
                let bb = ( bg         & 0xff) as f32;
                buf[idx] = (((br + (cr - br) * a) as u32) << 16)
                          | (((bg_g + (cg - bg_g) * a) as u32) << 8)
                          |  ((bb + (cb - bb) * a) as u32);
            }
        }
        cx += metrics.advance_width as i32;
    }
}

pub struct Viewer {
    windows: Vec<Window>,
    sequences: Vec<ImageSequence>,
    dims: Vec<(usize, usize)>,
    debug_window: Option<Window>,
    debug_font: Option<fontdue::Font>,
    debug_state: Option<Arc<DebugState>>,
}

impl Viewer {
    pub fn new(mut entries: Vec<(ImageSequence, usize)>, is_debug_display: bool) -> Self {
        let mut windows = Vec::with_capacity(entries.len());
        let mut sequences = Vec::with_capacity(entries.len());
        let mut dims = Vec::with_capacity(entries.len());

        for (i, (mut seq, display)) in entries.drain(..).enumerate() {
            let (x, y, w, h) = display_bounds(display);
            seq.set_dimensions(w, h);

            if !is_debug_display {
                let mut win = Window::new(&format!("Window {}", i), w, h, WindowOptions { resize: true, ..WindowOptions::default() })
                    .unwrap_or_else(|_| panic!("failed to create window {}", i));
                win.set_position(x, y);
                win.set_target_fps(60);
                windows.push(win);
            }

            sequences.push(seq);
            dims.push((w, h));
        }

        let (debug_window, debug_font) = if is_debug_display {
            let mut win = Window::new("Debug", DEBUG_WIN_W, DEBUG_WIN_H,
                WindowOptions { resize: true, ..WindowOptions::default() })
                .unwrap_or_else(|_| panic!("failed to create debug window"));
            win.set_target_fps(30);

            let font_paths = [
                "/System/Library/Fonts/Monaco.ttf",
                "/System/Library/Fonts/Menlo.ttc",
                "/System/Library/Fonts/Helvetica.ttc",
            ];
            let font = font_paths.iter().find_map(|path| {
                std::fs::read(path).ok().and_then(|data| {
                    fontdue::Font::from_bytes(data.as_slice(), fontdue::FontSettings::default()).ok()
                })
            });
            if font.is_none() {
                eprintln!("[Debug] No system font found; text will not render.");
            }
            (Some(win), font)
        } else {
            (None, None)
        };

        Self { windows, sequences, dims, debug_window, debug_font, debug_state: None }
    }

    pub fn set_debug_state(&mut self, state: Arc<DebugState>) {
        self.debug_state = Some(state);
    }

    pub fn brightness_at_angle(&self, angle: f64) -> Option<f64> {
        self.sequences.first()?.dynamic_brightness_at_angle(angle)
    }

    pub fn scale_at_angle(&self, angle: f64) -> Option<f64> {
        self.sequences.first()?.dynamic_scale_at_angle(angle)
    }

    pub fn is_open(&self) -> bool {
        if self.windows.is_empty() {
            self.debug_window.as_ref()
                .map_or(false, |w| w.is_open() && !w.is_key_down(Key::Escape) && !w.is_key_down(Key::Q))
        } else {
            self.windows.iter().all(|w| w.is_open() && !w.is_key_down(Key::Escape) && !w.is_key_down(Key::Q))
        }
    }

    pub fn render(&mut self, angle: f64) -> Vec<usize> {
        let mut indices = Vec::with_capacity(self.sequences.len());
        for ((win, seq), &(w, h)) in self.windows.iter_mut()
            .zip(self.sequences.iter_mut())
            .zip(self.dims.iter())
        {
            let scale = seq.dynamic_scale_at_angle(angle).or_else(|| seq.scale_factor());
            let brightness = seq.dynamic_brightness_at_angle(angle);
            let do_rotate = seq.match_angle;
            indices.push(seq.frame_index_at_angle(angle));
            let frame = seq.frame_at_angle(angle);
            let rotated: Vec<u32>;
            let scaled: Vec<u32>;
            let brightened: Vec<u32>;
            let buf: &[u32] = frame;
            let buf: &[u32] = if do_rotate {
                rotated = rotate_frame(buf, w, h, angle);
                &rotated
            } else {
                buf
            };
            let buf: &[u32] = if let Some(s) = scale {
                scaled = scale_from_center(buf, w, h, s);
                &scaled
            } else {
                buf
            };
            let buf: &[u32] = if let Some(b) = brightness {
                brightened = buf.iter().map(|&px| apply_brightness(px, b)).collect();
                &brightened
            } else {
                buf
            };
            win.update_with_buffer(buf, w, h)
                .unwrap_or_else(|e| eprintln!("[Viewer] update failed: {}", e));
        }

        if self.debug_window.is_some() {
            let (fw, fh) = self.dims.first().copied().unwrap_or((0, 0));
            let (frame, brightness, scale, index) = if let Some(seq) = self.sequences.first_mut() {
                let index = seq.frame_index_at_angle(angle);
                let frame = seq.frame_at_angle(angle).to_vec();
                let b = self.debug_state.as_ref().map_or_else(
                    || seq.dynamic_brightness_at_angle(angle),
                    |ds| ds.brightness(),
                );
                let s = self.debug_state.as_ref().map_or_else(
                    || seq.dynamic_scale_at_angle(angle),
                    |ds| ds.scale(),
                );
                (frame, b, s, index)
            } else {
                (vec![], None, None, 0)
            };
            self.render_debug(angle, brightness, scale, index, &frame, fw, fh);
        }

        indices
    }

    fn render_debug(&mut self, angle: f64, brightness: Option<f64>, scale: Option<f64>, index: usize,
                    frame: &[u32], fw: usize, fh: usize) {
        let mut buf = vec![0x111111u32; DEBUG_WIN_W * DEBUG_WIN_H];

        // Right side: 40vw × 40vw image preview
        let preview_dim = DEBUG_WIN_W * 40 / 100;
        let preview_x = DEBUG_WIN_W - preview_dim;
        let preview_y = (DEBUG_WIN_H - preview_dim) / 2;
        if fw > 0 && fh > 0 && !frame.is_empty() {
            let scaled = scale_frame_to(frame, fw, fh, preview_dim, preview_dim);
            for py in 0..preview_dim {
                let dst_y = preview_y + py;
                if dst_y >= DEBUG_WIN_H { break; }
                buf[dst_y * DEBUG_WIN_W + preview_x..dst_y * DEBUG_WIN_W + preview_x + preview_dim]
                    .copy_from_slice(&scaled[py * preview_dim..(py + 1) * preview_dim]);
            }
        }

        // Left centre: three info lines
        let font_size = 26.0f32;
        let line_height = 44usize;
        let lines = [
            format!("angle:      {:.1}", angle),
            format!("index:      {}", index),
            brightness.map_or_else(
                || "brightness: N/A".to_string(),
                |b| format!("brightness: {:.2}", b),
            ),
            scale.map_or_else(
                || "scale:      N/A".to_string(),
                |s| format!("scale:      {:.2}", s),
            ),
            "video:      false".to_string(),
        ];
        let total_h = lines.len() * line_height;
        let text_x = 50i32;
        let text_start_y = ((DEBUG_WIN_H - total_h) / 2 + line_height) as i32;

        if let Some(font) = &self.debug_font {
            for (i, line) in lines.iter().enumerate() {
                let y = text_start_y + (i * line_height) as i32;
                draw_text_to_buf(&mut buf, DEBUG_WIN_W, DEBUG_WIN_H, font, font_size,
                                 text_x, y, line, 0xEEEEEE);
            }
        }

        if let Some(win) = &mut self.debug_window {
            if win.is_open() {
                win.update_with_buffer(&buf, DEBUG_WIN_W, DEBUG_WIN_H)
                    .unwrap_or_else(|e| eprintln!("[Debug] update failed: {}", e));
            }
        }
    }
}
