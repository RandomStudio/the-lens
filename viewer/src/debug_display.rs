use minifb::{Key, Window, WindowOptions};
use crate::easing::{eased_proximity_scale, eased_proximity_light, eased_proximity_diamond};
use crate::image_sequence::{ImageSequence, scale_frame_to};

const WIN_W: usize = 1400;
const WIN_H: usize = 700;

const BG_COLOR: u32 = 0x0D0D12;
const CIRCLE_FILL_COLOR: u32 = 0x1A1A2E;
const RING_COLOR: u32 = 0x4A4A6A;
const DOT_COLOR: u32 = 0x00E5FF;
const TEXT_COLOR: u32 = 0xE8E8F0;
const DIM_TEXT_COLOR: u32 = 0x666680;

pub struct DebugDisplay {
    window: Window,
    fg_seq: ImageSequence,
    bg_seq: ImageSequence,
    font: Option<fontdue::Font>,
    preview_w: usize,
    preview_h: usize,
    min_scale: f64,
    max_scale: f64,
    brightest_brightness: f64,
    easing_multiplier: f64,
}

impl DebugDisplay {
    pub fn new(sequence_path: &str, index_transform: fn(isize, isize) -> isize, min_scale: f64, max_scale: f64, brightest_brightness: f64, easing_multiplier: f64) -> Self {
        let mut window = Window::new(
            "Lens — Debug",
            WIN_W, WIN_H,
            WindowOptions { resize: true, ..Default::default() },
        ).expect("Failed to create debug window");
        window.set_target_fps(30);

        let fg_path = format!("{}/1", sequence_path.trim_end_matches('/'));
        let bg_path = format!("{}/2", sequence_path.trim_end_matches('/'));
        let mut fg_seq = ImageSequence::load(&fg_path, index_transform);
        let mut bg_seq = ImageSequence::load(&bg_path, index_transform);

        // Size preview panel to maintain native aspect ratio (use whichever sequence has dimensions)
        let right_panel_w = WIN_W / 2;
        let right_panel_h = WIN_H;
        let native = fg_seq.peek_dimensions().or_else(|| bg_seq.peek_dimensions());
        let (preview_w, preview_h) = if let Some((nw, nh)) = native {
            let aspect = nw as f64 / nh as f64;
            let h = right_panel_h;
            let w = (h as f64 * aspect) as usize;
            (w.min(right_panel_w), h)
        } else {
            (right_panel_w, right_panel_h)
        };
        fg_seq.set_dimensions(preview_w, preview_h);
        bg_seq.set_dimensions(preview_w, preview_h);

        let font = load_font();
        if font.is_none() {
            eprintln!("[DebugDisplay] No system font found; text will not render.");
        }

        Self { window, fg_seq, bg_seq, font, preview_w, preview_h, min_scale, max_scale, brightest_brightness, easing_multiplier }
    }

    pub fn is_open(&self) -> bool {
        self.window.is_open()
            && !self.window.is_key_down(Key::Escape)
            && !self.window.is_key_down(Key::Q)
    }

    pub fn render(&mut self, angle: f64) {
        let mut buf = vec![BG_COLOR; WIN_W * WIN_H];

        let frame_count = self.fg_seq.frame_count();
        let frame_idx = self.fg_seq.frame_index_at_angle(angle);
        let light_e = (eased_proximity_light(frame_idx, frame_count) * self.easing_multiplier).clamp(0.0, 1.0);
        let light_brightness = self.brightest_brightness * (1.0 - light_e);
        let diamond_opacity = (eased_proximity_diamond(frame_idx, frame_count) * self.easing_multiplier).clamp(0.0, 1.0);
        let seq_scale = 1.0;

        // Left panel: circle with angle indicator + stats
        let left_w = WIN_W / 2;
        let cx = left_w as f64 / 2.0;
        let cy = WIN_H as f64 / 2.0;
        let circle_r = cy.min(cx) * 0.72;

        draw_circle_fill(&mut buf, WIN_W, WIN_H, cx, cy, circle_r, CIRCLE_FILL_COLOR, 0.35);
        draw_ring(&mut buf, WIN_W, WIN_H, cx, cy, circle_r, RING_COLOR, 0.6, 2.0);

        // Angle dot at position on ring (0° = top, clockwise)
        let angle_rad = angle.to_radians();
        let dot_x = cx + circle_r * angle_rad.sin();
        let dot_y = cy - circle_r * angle_rad.cos();
        draw_glow_dot(&mut buf, WIN_W, WIN_H, dot_x, dot_y, 18.0, DOT_COLOR);

        // Stats text inside circle
        if let Some(ref font) = self.font {
            let lines: &[(&str, String)] = &[
                ("ANGLE", format!("{:.1}°", angle)),
                ("INDEX", format!("{}", frame_idx)),
                ("BRIGHTNESS", format!("{:.3}", light_brightness)),
                ("SCALE", format!("{:.3}x", seq_scale)),
                ("DIAMOND", format!("{:.3}", diamond_opacity)),
            ];

            let label_size = 14.0f32;
            let value_size = 28.0f32;
            let block_h = 58usize;
            let total_h = lines.len() * block_h;
            let start_y = (WIN_H - total_h) / 2;

            for (i, (label, value)) in lines.iter().enumerate() {
                let label_y = (start_y + i * block_h + 18) as i32;
                let value_y = (start_y + i * block_h + 46) as i32;
                let text_x = (cx - 70.0) as i32;

                draw_text(&mut buf, WIN_W, WIN_H, font, label_size,
                          text_x, label_y, label, DIM_TEXT_COLOR);
                draw_text(&mut buf, WIN_W, WIN_H, font, value_size,
                          text_x, value_y, value, TEXT_COLOR);
            }
        }

        // Right panel: image sequence preview
        let preview_x_offset = (WIN_W / 2 - self.preview_w) / 2;  // center in right half... wait, right panel starts at WIN_W/2
        let right_start = WIN_W / 2;
        let panel_w = WIN_W - right_start;
        let px_offset = right_start + (panel_w.saturating_sub(self.preview_w)) / 2;
        let py_offset = (WIN_H.saturating_sub(self.preview_h)) / 2;

        if frame_count > 0 || self.bg_seq.frame_count() > 0 {
            let pw = self.preview_w;
            let ph = self.preview_h;
            let fg_frame = self.fg_seq.frame_at_angle(angle).to_vec();
            let bg_frame = self.bg_seq.frame_at_angle(angle).to_vec();

            let normalize = |frame: Vec<u32>| -> Vec<u32> {
                if frame.len() == pw * ph {
                    frame
                } else {
                    let seq_w = (frame.len() as f64).sqrt() as usize; // approximate, fallback
                    scale_frame_to(&frame, seq_w, seq_w, pw, ph)
                }
            };

            let mut preview = normalize(bg_frame);
            let fg_norm = normalize(fg_frame);
            composite_over_dbg(&fg_norm, &mut preview);

            for py in 0..ph {
                let dst_y = py_offset + py;
                if dst_y >= WIN_H { break; }
                for px in 0..pw {
                    let dst_x = px_offset + px;
                    if dst_x >= WIN_W { break; }
                    buf[dst_y * WIN_W + dst_x] = preview[py * pw + px];
                }
            }
        }

        // Vertical divider
        for y in 0..WIN_H {
            let x = WIN_W / 2;
            blend_pixel(&mut buf, WIN_W, x, y, 0x333345, 0.8);
        }

        let _ = preview_x_offset; // suppress unused warning

        // Scale logical buffer to current window size, letterboxing to preserve aspect ratio
        let (win_w, win_h) = self.window.get_size();
        let final_buf = fit_to_window(&buf, WIN_W, WIN_H, win_w, win_h);
        self.window.update_with_buffer(&final_buf, win_w, win_h)
            .unwrap_or_else(|e| eprintln!("[DebugDisplay] update failed: {}", e));
    }
}

fn composite_over_dbg(fg: &[u32], bg: &mut [u32]) {
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

fn fit_to_window(src: &[u32], src_w: usize, src_h: usize, dst_w: usize, dst_h: usize) -> Vec<u32> {
    if dst_w == src_w && dst_h == src_h {
        return src.to_vec();
    }
    let scale = (dst_w as f64 / src_w as f64).min(dst_h as f64 / src_h as f64);
    let out_w = (src_w as f64 * scale).round() as usize;
    let out_h = (src_h as f64 * scale).round() as usize;
    let off_x = (dst_w - out_w) / 2;
    let off_y = (dst_h - out_h) / 2;

    let mut dst = vec![0u32; dst_w * dst_h];
    for dy in 0..out_h {
        let sy = ((dy as f64 / scale) as usize).min(src_h - 1);
        for dx in 0..out_w {
            let sx = ((dx as f64 / scale) as usize).min(src_w - 1);
            dst[(off_y + dy) * dst_w + (off_x + dx)] = src[sy * src_w + sx];
        }
    }
    dst
}

fn load_font() -> Option<fontdue::Font> {
    let paths = [
        "/System/Library/Fonts/SFNSMono.ttf",
        "/System/Library/Fonts/Supplemental/SF Mono Regular.otf",
        "/System/Library/Fonts/Menlo.ttc",
        "/System/Library/Fonts/Monaco.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
    ];
    paths.iter().find_map(|p| {
        std::fs::read(p).ok().and_then(|data| {
            fontdue::Font::from_bytes(data.as_slice(), fontdue::FontSettings::default()).ok()
        })
    })
}

fn blend_pixel(buf: &mut [u32], buf_w: usize, x: usize, y: usize, color: u32, alpha: f64) {
    let idx = y * buf_w + x;
    if idx >= buf.len() { return; }
    let bg = buf[idx];
    let cr = ((color >> 16) & 0xff) as f64;
    let cg = ((color >>  8) & 0xff) as f64;
    let cb = ( color        & 0xff) as f64;
    let br = ((bg >> 16) & 0xff) as f64;
    let bg_g = ((bg >>  8) & 0xff) as f64;
    let bb = ( bg        & 0xff) as f64;
    let r = (br + (cr - br) * alpha) as u32;
    let g = (bg_g + (cg - bg_g) * alpha) as u32;
    let b = (bb + (cb - bb) * alpha) as u32;
    buf[idx] = (r << 16) | (g << 8) | b;
}

fn draw_circle_fill(
    buf: &mut [u32], buf_w: usize, buf_h: usize,
    cx: f64, cy: f64, radius: f64, color: u32, alpha: f64,
) {
    let x0 = (cx - radius - 1.0).max(0.0) as usize;
    let x1 = (cx + radius + 1.0).min(buf_w as f64) as usize;
    let y0 = (cy - radius - 1.0).max(0.0) as usize;
    let y1 = (cy + radius + 1.0).min(buf_h as f64) as usize;

    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f64 - cx;
            let dy = py as f64 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= radius {
                // Soft edge
                let edge_alpha = if dist > radius - 1.5 {
                    (radius - dist) / 1.5
                } else {
                    1.0
                };
                blend_pixel(buf, buf_w, px, py, color, alpha * edge_alpha);
            }
        }
    }
}

fn draw_ring(
    buf: &mut [u32], buf_w: usize, buf_h: usize,
    cx: f64, cy: f64, radius: f64, color: u32, alpha: f64, width: f64,
) {
    let outer = radius + width;
    let x0 = (cx - outer - 1.0).max(0.0) as usize;
    let x1 = (cx + outer + 1.0).min(buf_w as f64) as usize;
    let y0 = (cy - outer - 1.0).max(0.0) as usize;
    let y1 = (cy + outer + 1.0).min(buf_h as f64) as usize;

    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f64 - cx;
            let dy = py as f64 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let ring_dist = (dist - radius).abs();
            if ring_dist < width + 1.0 {
                let a = (1.0 - (ring_dist / (width + 1.0))).clamp(0.0, 1.0) * alpha;
                blend_pixel(buf, buf_w, px, py, color, a);
            }
        }
    }
}

fn draw_glow_dot(
    buf: &mut [u32], buf_w: usize, buf_h: usize,
    cx: f64, cy: f64, radius: f64, color: u32,
) {
    let glow_r = radius * 3.5;
    let x0 = (cx - glow_r - 1.0).max(0.0) as usize;
    let x1 = (cx + glow_r + 1.0).min(buf_w as f64) as usize;
    let y0 = (cy - glow_r - 1.0).max(0.0) as usize;
    let y1 = (cy + glow_r + 1.0).min(buf_h as f64) as usize;

    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f64 - cx;
            let dy = py as f64 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < glow_r {
                let alpha = if dist < radius {
                    1.0
                } else {
                    // Exponential glow falloff
                    let t = (dist - radius) / (glow_r - radius);
                    (1.0 - t).powf(2.5) * 0.6
                };
                blend_pixel(buf, buf_w, px, py, color, alpha);
            }
        }
    }
}

fn draw_text(
    buf: &mut [u32], buf_w: usize, buf_h: usize,
    font: &fontdue::Font, size: f32,
    x: i32, baseline_y: i32, text: &str, color: u32,
) {
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
