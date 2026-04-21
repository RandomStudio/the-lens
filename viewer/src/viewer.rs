use minifb::{Key, Window, WindowOptions};
use rayon::prelude::*;
use std::path::Path;

const TOTAL_FRAMES: usize = 60;

pub struct ImageSequence {
    frames: Vec<Vec<u32>>,
}

impl ImageSequence {
    pub fn load(folder: &str, width: usize, height: usize) -> Self {
        let blank_frame = || vec![0u32; width * height];

        let path = Path::new(folder);
        if !path.exists() {
            eprintln!("[WARN] '{}' does not exist — using blank frames.", folder);
            return Self { frames: vec![blank_frame(); TOTAL_FRAMES] };
        }

        let mut entries: Vec<_> = std::fs::read_dir(path)
            .expect("Failed to read image folder")
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                n.ends_with(".png") || n.ends_with(".jpg") || n.ends_with(".jpeg")
            })
            .collect();

        entries.sort_by_key(|e| e.file_name());

        if entries.is_empty() {
            eprintln!("[WARN] No images in '{}' — using blank frames.", folder);
            return Self { frames: vec![blank_frame(); TOTAL_FRAMES] };
        }

        println!("[INFO] Loading {} images from '{}'", entries.len(), folder);

        let frames = entries
            .par_iter()
            .map(|entry| {
                let img = image::open(entry.path())
                    .unwrap_or_else(|_| panic!("Failed to open {:?}", entry.path()))
                    .to_rgba8();

                let img_w = img.width() as usize;
                let img_h = img.height() as usize;

                // Centre vertically; crop if taller than window
                let src_y0 = if img_h > height { (img_h - height) / 2 } else { 0 };
                let dst_y0 = if img_h < height { (height - img_h) / 2 } else { 0 };
                let rows   = img_h.min(height);
                let cols   = img_w.min(width);

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
            })
            .collect();

        Self { frames }
    }

    pub fn frame_at_angle(&self, angle: f64) -> &[u32] {
        let n = self.frames.len();
        let idx = (angle.rem_euclid(360.0) / 360.0 * n as f64) as usize;
        &self.frames[idx.min(n - 1)]
    }
}

pub struct Viewer {
    window1: Window,
    window2: Window,
    seq1: ImageSequence,
    seq2: ImageSequence,
    w1: (usize, usize),
    w2: (usize, usize),
}

impl Viewer {
    pub fn new(
        seq1: ImageSequence,
        w1_size: (usize, usize),
        w1_pos: (isize, isize),
        seq2: ImageSequence,
        w2_size: (usize, usize),
        w2_pos: (isize, isize),
    ) -> Self {
        let fs_opts = WindowOptions {
            borderless: true,
            topmost: true,
            resize: false,
            ..WindowOptions::default()
        };

        let mut window1 = Window::new("Display 1", w1_size.0, w1_size.1, fs_opts.clone())
            .expect("Failed to create window 1");
        window1.set_position(w1_pos.0, w1_pos.1);
        window1.set_target_fps(60);

        let mut window2 = Window::new("Display 2", w2_size.0, w2_size.1, fs_opts)
            .expect("Failed to create window 2");
        window2.set_position(w2_pos.0, w2_pos.1);
        window2.set_target_fps(60);

        Self { window1, window2, seq1, seq2, w1: w1_size, w2: w2_size }
    }

    pub fn is_open(&self) -> bool {
        self.window1.is_open()
            && self.window2.is_open()
            && !self.window1.is_key_down(Key::Escape)
            && !self.window2.is_key_down(Key::Escape)
    }

    pub fn render(&mut self, angle: f64) {
        self.window1
            .update_with_buffer(self.seq1.frame_at_angle(angle), self.w1.0, self.w1.1)
            .expect("Window 1 update failed");

        self.window2
            .update_with_buffer(self.seq2.frame_at_angle(angle), self.w2.0, self.w2.1)
            .expect("Window 2 update failed");
    }
}
