use minifb::{Key, Window, WindowOptions};
use std::path::Path;

const TOTAL_FRAMES: usize = 60;

pub struct ImageSequence {
    frames: Vec<Vec<u32>>,
}

impl ImageSequence {
    pub fn load(folder: &str, width: usize, height: usize) -> Self {
        let img_size = width;

        let top_pad = if img_size < height {
            (height - img_size) / 2
        } else {
            0
        };

        let src_start = if img_size > height {
            (img_size - height) / 2
        } else {
            0
        };

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

        println!(
            "[INFO] Loading {} images from '{}' → {}×{} on {}×{} canvas",
            entries.len(),
            folder,
            img_size,
            img_size,
            width,
            height
        );

        let frames = entries
            .iter()
            .map(|entry| {
                let img = image::open(entry.path())
                    .unwrap_or_else(|_| panic!("Failed to open {:?}", entry.path()));

                let img = img
                    .resize_exact(
                        img_size as u32,
                        img_size as u32,
                        image::imageops::FilterType::Lanczos3,
                    )
                    .to_rgba8();

                let mut canvas = vec![0u32; width * height];

                for (src_y, row_pixels) in img.rows().enumerate() {
                    if src_y < src_start {
                        continue;
                    }
                    let dst_y = top_pad + (src_y - src_start);
                    if dst_y >= height {
                        break;
                    }
                    let row_start = dst_y * width;
                    for (src_x, pixel) in row_pixels.enumerate() {
                        let [r, g, b, _] = pixel.0;
                        canvas[row_start + src_x] =
                            ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
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
        let mut window1 = Window::new(
            "Display 1",
            w1_size.0,
            w1_size.1,
            WindowOptions {
                borderless: true,
                resize: false,
                ..WindowOptions::default()
            },
        )
        .expect("Failed to create window 1");

        window1.set_position(w1_pos.0, w1_pos.1);
        window1.set_target_fps(60);

        let mut window2 = Window::new(
            "Display 2",
            w2_size.0,
            w2_size.1,
            WindowOptions {
                borderless: true,
                resize: false,
                ..WindowOptions::default()
            },
        )
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
