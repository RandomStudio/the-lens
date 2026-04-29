use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;
use crate::easing::{eased_proximity_light, eased_proximity_diamond};
use crate::image_sequence::ImageSequence;
use crate::light::Light;
use crate::surface::{WindowedSurface, create_fullscreen_surface, sorted_monitors};

const SEQUENCE_DISPLAY: usize = 1;
const DIAMOND_DISPLAY: usize = 0;

pub struct Display {
    seq_win: WindowedSurface,
    diamond_win: Option<WindowedSurface>,
    diamond: Vec<u32>,
    diamond_w: usize,
    diamond_h: usize,
    fg_seq: ImageSequence,
    bg_seq: ImageSequence,
    light: Light,
    min_scale: f64,
    max_scale: f64,
    brightest_brightness: f64,
    easing_multiplier: f64,
}

fn composite_over(fg: &[u32], bg: &mut [u32]) {
    let n = fg.len().min(bg.len());
    for i in 0..n {
        let f = fg[i];
        let a = (f >> 24) & 0xff;
        if a == 0 { continue; }
        if a == 0xff {
            bg[i] = f & 0x00ff_ffff;
            continue;
        }
        let af = a as u32;
        let inv = 255 - af;
        let fr = (f >> 16) & 0xff;
        let fg_g = (f >> 8) & 0xff;
        let fb = f & 0xff;
        let br = (bg[i] >> 16) & 0xff;
        let bg_g = (bg[i] >> 8) & 0xff;
        let bb = bg[i] & 0xff;
        let r = (fr * af + br * inv) / 255;
        let g = (fg_g * af + bg_g * inv) / 255;
        let b = (fb * af + bb * inv) / 255;
        bg[i] = (r << 16) | (g << 8) | b;
    }
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

fn rotate_diamond(
    src: &[u32], src_w: usize, src_h: usize,
    dst: &mut Vec<u32>, dst_w: usize, dst_h: usize,
    angle_deg: f64, opacity: f64,
) {
    let angle_rad = angle_deg.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    let cx_src = src_w as f64 / 2.0;
    let cy_src = src_h as f64 / 2.0;
    let cx_dst = dst_w as f64 / 2.0;
    let cy_dst = dst_h as f64 / 2.0;

    let scale = (dst_w as f64 / src_w as f64).min(dst_h as f64 / src_h as f64);

    dst.resize(dst_w * dst_h, 0);
    for v in dst.iter_mut() { *v = 0; }

    for py in 0..dst_h {
        for px in 0..dst_w {
            let dx = (px as f64 - cx_dst) / scale;
            let dy = (py as f64 - cy_dst) / scale;

            let sx = cx_src + dx * cos_a + dy * sin_a;
            let sy = cy_src - dx * sin_a + dy * cos_a;

            if sx >= 0.0 && sx < (src_w - 1) as f64 && sy >= 0.0 && sy < (src_h - 1) as f64 {
                let x0 = sx as usize;
                let y0 = sy as usize;
                let xf = sx - x0 as f64;
                let yf = sy - y0 as f64;

                let p00 = src[y0 * src_w + x0];
                let p10 = src[y0 * src_w + x0 + 1];
                let p01 = src[(y0 + 1) * src_w + x0];
                let p11 = src[(y0 + 1) * src_w + x0 + 1];

                let r = bilerp(p00, p10, p01, p11, xf, yf, 16, opacity);
                let g = bilerp(p00, p10, p01, p11, xf, yf, 8, opacity);
                let b = bilerp(p00, p10, p01, p11, xf, yf, 0, opacity);

                dst[py * dst_w + px] = (r << 16) | (g << 8) | b;
            }
        }
    }
}

#[inline]
fn bilerp(p00: u32, p10: u32, p01: u32, p11: u32, xf: f64, yf: f64, shift: u32, opacity: f64) -> u32 {
    let c00 = ((p00 >> shift) & 0xFF) as f64;
    let c10 = ((p10 >> shift) & 0xFF) as f64;
    let c01 = ((p01 >> shift) & 0xFF) as f64;
    let c11 = ((p11 >> shift) & 0xFF) as f64;
    let top = c00 + (c10 - c00) * xf;
    let bot = c01 + (c11 - c01) * xf;
    ((top + (bot - top) * yf) * opacity) as u32
}

fn load_diamond(path: &str) -> (Vec<u32>, usize, usize) {
    let result = image::ImageReader::open(path)
        .and_then(|r| r.with_guessed_format())
        .and_then(|r| r.decode().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));

    match result {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let w = rgba.width() as usize;
            let h = rgba.height() as usize;
            let raw = rgba.as_raw();
            let pixels = (0..w * h).map(|i| {
                let p = i * 4;
                ((raw[p] as u32) << 16) | ((raw[p + 1] as u32) << 8) | (raw[p + 2] as u32)
            }).collect();
            println!("[Display] Loaded diamond image '{}' ({}x{})", path, w, h);
            (pixels, w, h)
        }
        Err(e) => {
            eprintln!("[Display] Failed to load diamond image '{}': {}", path, e);
            (vec![], 0, 0)
        }
    }
}

impl Display {
    pub fn new(
        event_loop: &ActiveEventLoop,
        sequence_path: &str,
        diamond_path: &str,
        index_transform: fn(isize, isize) -> isize,
        min_scale: f64,
        max_scale: f64,
        brightest_brightness: f64,
        easing_multiplier: f64,
    ) -> Self {
        let monitors = sorted_monitors(event_loop);
        let seq_monitor = monitors.get(SEQUENCE_DISPLAY).cloned()
            .or_else(|| monitors.first().cloned())
            .expect("no monitors available");
        let diamond_monitor = monitors.get(DIAMOND_DISPLAY).cloned();

        let seq_win = create_fullscreen_surface(event_loop, "Lens — Sequence", seq_monitor);

        let fg_path = format!("{}/1", sequence_path.trim_end_matches('/'));
        let bg_path = format!("{}/2", sequence_path.trim_end_matches('/'));
        let mut fg_seq = ImageSequence::load(&fg_path, index_transform);
        let mut bg_seq = ImageSequence::load(&bg_path, index_transform);
        fg_seq.set_dimensions(seq_win.width as usize, seq_win.height as usize);
        bg_seq.set_dimensions(seq_win.width as usize, seq_win.height as usize);

        let diamond_win = diamond_monitor.map(|m|
            create_fullscreen_surface(event_loop, "Lens — Diamond", m)
        );
        if diamond_win.is_none() {
            eprintln!("[Display] Display {} not available — diamond window skipped", DIAMOND_DISPLAY);
        }

        let (diamond, diamond_w, diamond_h) = load_diamond(diamond_path);

        Self {
            seq_win,
            diamond_win,
            diamond,
            diamond_w,
            diamond_h,
            fg_seq,
            bg_seq,
            light: Light::new(),
            min_scale,
            max_scale,
            brightest_brightness,
            easing_multiplier,
        }
    }

    pub fn request_redraws(&self) {
        self.seq_win.window.request_redraw();
        if let Some(ref dw) = self.diamond_win {
            dw.window.request_redraw();
        }
    }

    pub fn handles_window(&self, id: WindowId) -> bool {
        self.seq_win.window.id() == id
            || self.diamond_win.as_ref().map_or(false, |w| w.window.id() == id)
    }

    pub fn resize_window(&mut self, id: WindowId, width: u32, height: u32) {
        if self.seq_win.window.id() == id {
            self.seq_win.resize(width, height);
            self.fg_seq.set_dimensions(width as usize, height as usize);
            self.bg_seq.set_dimensions(width as usize, height as usize);
        } else if let Some(ref mut dw) = self.diamond_win {
            if dw.window.id() == id {
                dw.resize(width, height);
            }
        }
    }

    pub fn render_window(&mut self, id: WindowId, angle: f64) {
        if self.seq_win.window.id() == id {
            self.render_sequence(angle);
        } else if let Some(ref mut dw) = self.diamond_win {
            if dw.window.id() == id {
                let frame_idx = self.fg_seq.frame_index_at_angle(angle);
                let frame_count = self.fg_seq.frame_count();
                let diamond_opacity = (eased_proximity_diamond(frame_idx, frame_count) * self.easing_multiplier).clamp(0.0, 1.0);
                let mut buf = vec![0u32; (dw.width as usize) * (dw.height as usize)];
                if self.diamond_w > 0 {
                    rotate_diamond(
                        &self.diamond, self.diamond_w, self.diamond_h,
                        &mut buf, dw.width as usize, dw.height as usize,
                        angle, diamond_opacity,
                    );
                }
                dw.write_rgb(&buf);
                dw.present();
            }
        }
    }

    fn render_sequence(&mut self, angle: f64) {
        let (w, h) = (self.seq_win.width as usize, self.seq_win.height as usize);
        let frame_idx = self.fg_seq.frame_index_at_angle(angle);
        let fg_frame = self.fg_seq.frame_at_angle(angle).to_vec();
        let bg_frame = self.bg_seq.frame_at_angle(angle).to_vec();

        let frame_count = self.fg_seq.frame_count();
        let light_e = (eased_proximity_light(frame_idx, frame_count) * self.easing_multiplier).clamp(0.0, 1.0);
        let light_brightness = self.brightest_brightness * (1.0 - light_e);
        let seq_scale: f64 = 1.0;
        let _ = (self.min_scale, self.max_scale);

        let identity_scale = (seq_scale - 1.0).abs() < f64::EPSILON;
        let mut composed = if identity_scale { bg_frame } else { scale_from_center(&bg_frame, w, h, seq_scale) };
        let fg_to_composite = if identity_scale { fg_frame } else { scale_from_center(&fg_frame, w, h, seq_scale) };
        composite_over(&fg_to_composite, &mut composed);

        self.seq_win.write_rgb(&composed);
        self.seq_win.present();

        self.light.update(light_brightness);

        print!("\rAngle: {:6.2}°  idx: {:4}  brightness: {:.3}  scale: {:.3}  ",
               angle, frame_idx, light_brightness, seq_scale);
    }

    pub fn turn_off_light(&self) {
        self.light.turn_off();
    }
}
