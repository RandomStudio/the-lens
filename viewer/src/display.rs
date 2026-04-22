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
    seq: ImageSequence,
    diamond_seq: ImageSequence,
    light: Light,
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


impl Display {
    pub fn new(
        sequence_path: &str,
        diamond_path: &str,
        index_transform: fn(isize, isize) -> isize,
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

        let mut diamond_seq = ImageSequence::load(diamond_path, index_transform);
        if let Some((_, w2, h2)) = &win2 {
            diamond_seq.set_dimensions(*w2, *h2);
        }

        Self {
            win1_size: (w1, h1),
            win1,
            win1_buffer: vec![0u32; w1 * h1],
            win2,
            win2_buffer: vec![],
            seq,
            diamond_seq,
            light: Light::new(),
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
        // Detect resize and re-decode at new dimensions
        let new_size = self.win1.get_size();
        if new_size != self.win1_size {
            self.win1_size = new_size;
            self.seq.set_dimensions(new_size.0, new_size.1);
        }

        let (w1, h1) = self.win1_size;
        let frame_idx = self.seq.frame_index_at_angle(angle);
        let frame = self.seq.frame_at_angle(angle).to_vec();

        let eased = eased_proximity(frame_idx, self.seq.frame_count());
        let light_brightness = 1.0 - eased;

        let seq_scale = 1.0 + eased;  // 1.0 → 2.0 as eased goes 0 → 1
        self.win1_buffer = scale_from_center(&frame, w1, h1, seq_scale);

        self.win1.update_with_buffer(&self.win1_buffer, w1, h1)
            .unwrap_or_else(|e| eprintln!("[Display] win1 update failed: {}", e));
        let diamond_opacity = eased;

        self.light.update(light_brightness);

        if let Some((ref mut win2, ref mut stored_w, ref mut stored_h)) = self.win2 {
            let (w2, h2) = win2.get_size();
            if (w2, h2) != (*stored_w, *stored_h) {
                *stored_w = w2;
                *stored_h = h2;
                self.diamond_seq.set_dimensions(w2, h2);
            }
            let raw = self.diamond_seq.frame_at_angle(angle);
            self.win2_buffer = raw.iter().map(|&px| {
                let r = (((px >> 16) & 0xFF) as f64 * diamond_opacity) as u32;
                let g = (((px >> 8) & 0xFF) as f64 * diamond_opacity) as u32;
                let b = ((px & 0xFF) as f64 * diamond_opacity) as u32;
                (r << 16) | (g << 8) | b
            }).collect();
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
