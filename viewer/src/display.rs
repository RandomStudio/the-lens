use minifb::{Key, Window, WindowOptions};
use crate::easing::eased_proximity;
use crate::image_sequence::ImageSequence;
use crate::light::Light;

const SEQUENCE_DISPLAY: usize = 1;
const DIAMOND_DISPLAY: usize = 0;

pub struct Display {
    win1: Window,
    win1_size: (usize, usize),
    win1_buffer: Vec<u32>,
    win2: Option<(Window, usize, usize)>,
    win2_buffer: Vec<u32>,
    diamond: Vec<u32>,
    diamond_w: usize,
    diamond_h: usize,
    seq: ImageSequence,
    light: Light,
    max_scale: f64,
    brightest_brightness: f64,
}

fn display_bounds(index: usize) -> Option<(isize, isize, usize, usize)> {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::display::CGDisplay;
        let ids = CGDisplay::active_displays().ok()?;
        let mut displays: Vec<(isize, isize, usize, usize)> = ids
            .iter()
            .map(|&id| {
                let b = CGDisplay::new(id).bounds();
                (b.origin.x as isize, b.origin.y as isize,
                 b.size.width as usize, b.size.height as usize)
            })
            .collect();
        displays.sort_by_key(|&(x, y, _, _)| (x, y));
        return displays.get(index).copied();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = index;
        Some((0, 0, 1920, 1080))
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
        sequence_path: &str,
        diamond_path: &str,
        index_transform: fn(isize, isize) -> isize,
        max_scale: f64,
        brightest_brightness: f64,
    ) -> Self {
        let (x1, y1, w1, h1) = display_bounds(SEQUENCE_DISPLAY)
            .unwrap_or_else(|| {
                eprintln!("[Display] Display {} not available, using fallback size", SEQUENCE_DISPLAY);
                (0, 0, 1280, 720)
            });

        let mut seq = ImageSequence::load(sequence_path, index_transform);
        seq.set_dimensions(w1, h1);

        let mut win1 = Window::new(
            "Lens — Sequence",
            w1, h1,
            WindowOptions { resize: true, ..Default::default() },
        ).expect("Failed to create sequence window");
        win1.set_position(x1, y1);
        win1.set_target_fps(60);

        let win2 = display_bounds(DIAMOND_DISPLAY).and_then(|(x2, y2, w2, h2)| {
            let mut win = Window::new(
                "Lens — Diamond",
                w2, h2,
                WindowOptions { resize: true, ..Default::default() },
            ).ok()?;
            win.set_position(x2, y2);
            win.set_target_fps(60);
            Some((win, w2, h2))
        });

        if win2.is_none() {
            eprintln!("[Display] Display {} not available — diamond window skipped", DIAMOND_DISPLAY);
        }

        let (diamond, diamond_w, diamond_h) = load_diamond(diamond_path);

        Self {
            win1_size: (w1, h1),
            win1,
            win1_buffer: vec![0u32; w1 * h1],
            win2,
            win2_buffer: vec![],
            diamond,
            diamond_w,
            diamond_h,
            seq,
            light: Light::new(),
            max_scale,
            brightest_brightness,
        }
    }

    pub fn is_open(&self) -> bool {
        let w1 = self.win1.is_open()
            && !self.win1.is_key_down(Key::Escape)
            && !self.win1.is_key_down(Key::Q);
        let w2 = self.win2.as_ref()
            .map_or(true, |(w, _, _)| w.is_open()
                && !w.is_key_down(Key::Escape)
                && !w.is_key_down(Key::Q));
        w1 && w2
    }

    pub fn render(&mut self, angle: f64) {
        let new_size = self.win1.get_size();
        if new_size != self.win1_size {
            self.win1_size = new_size;
            self.seq.set_dimensions(new_size.0, new_size.1);
        }

        let (w1, h1) = self.win1_size;
        let frame_idx = self.seq.frame_index_at_angle(angle);
        let frame = self.seq.frame_at_angle(angle).to_vec();

        let eased = eased_proximity(frame_idx, self.seq.frame_count());
        let light_brightness = self.brightest_brightness * (1.0 - eased);
        let seq_scale = 1.0 + (self.max_scale - 1.0) * eased;
        let diamond_opacity = eased;

        self.win1_buffer = scale_from_center(&frame, w1, h1, seq_scale);
        self.win1.update_with_buffer(&self.win1_buffer, w1, h1)
            .unwrap_or_else(|e| eprintln!("[Display] win1 update failed: {}", e));

        self.light.update(light_brightness);

        if let Some((ref mut win2, ref mut stored_w, ref mut stored_h)) = self.win2 {
            let (w2, h2) = win2.get_size();
            if (w2, h2) != (*stored_w, *stored_h) {
                *stored_w = w2;
                *stored_h = h2;
            }
            if self.diamond_w > 0 {
                rotate_diamond(
                    &self.diamond, self.diamond_w, self.diamond_h,
                    &mut self.win2_buffer, w2, h2,
                    angle, diamond_opacity,
                );
            } else {
                self.win2_buffer.resize(w2 * h2, 0);
            }
            win2.update_with_buffer(&self.win2_buffer, w2, h2)
                .unwrap_or_else(|e| eprintln!("[Display] win2 update failed: {}", e));
        }

        print!("\rAngle: {:6.2}°  idx: {:4}  brightness: {:.3}  scale: {:.3}  diamond: {:.3}  ",
               angle, frame_idx, light_brightness, seq_scale, diamond_opacity);
    }

    pub fn turn_off_light(&self) {
        self.light.turn_off();
    }
}
