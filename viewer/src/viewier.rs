use minifb::{Key, Window, WindowOptions};
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

// ─── Configuration ────────────────────────────────────────────────────────────

const MQTT_BROKER: &str = "10.112.10.10";
const MQTT_PORT:   u16  = 1883;
const MQTT_TOPIC:  &str = "the-lens/angle";

const IMAGE_SEQUENCE_FOLDER_1: &str = "./sequence1";
const IMAGE_SEQUENCE_FOLDER_2: &str = "./sequence2";

const TOTAL_FRAMES: usize = 60;

// Set each window's position (top-left corner in desktop coordinates) and
// resolution to match your physical monitors.
// In Windows: right-click desktop → Display Settings to find each
// monitor's resolution and position.
const WINDOW_1_X: isize = 0;
const WINDOW_1_Y: isize = 0;
const WINDOW_1_W: usize = 1920;
const WINDOW_1_H: usize = 1080;

const WINDOW_2_X: isize = 1920; // typically X = width of monitor 1
const WINDOW_2_Y: isize = 0;
const WINDOW_2_W: usize = 1920;
const WINDOW_2_H: usize = 1080;

// ─── Image loading ────────────────────────────────────────────────────────────

/// Load all images from a folder, scaled to `width × width` (square, filling
/// full window width), composited onto a `width × height` black canvas.
/// If the square is taller than the window it is clipped symmetrically top
/// and bottom; if shorter it is centred with black bars.
fn load_image_sequence(
    folder:        &str,
    window_width:  usize,
    window_height: usize,
) -> Vec<Vec<u32>> {
    let img_size = window_width; // square edge length = window width

    // Vertical offset into the canvas when image is shorter than window
    let top_pad: usize = if img_size < window_height {
        (window_height - img_size) / 2
    } else {
        0
    };

    // First source row to copy when image is taller than window
    let src_start: usize = if img_size > window_height {
        (img_size - window_height) / 2
    } else {
        0
    };

    let blank_frame = || vec![0u32; window_width * window_height];

    let path = Path::new(folder);
    if !path.exists() {
        eprintln!("[WARN] '{}' does not exist — using blank frames.", folder);
        return vec![blank_frame(); TOTAL_FRAMES];
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
        return vec![blank_frame(); TOTAL_FRAMES];
    }

    println!(
        "[INFO] Loading {} images from '{}' → {}×{} on {}×{} canvas",
        entries.len(), folder, img_size, img_size, window_width, window_height
    );

    entries
        .iter()
        .map(|entry| {
            let img = image::open(entry.path())
                .unwrap_or_else(|_| panic!("Failed to open {:?}", entry.path()));

            let img = img.resize_exact(
                img_size as u32,
                img_size as u32,
                image::imageops::FilterType::Lanczos3,
            ).to_rgba8();

            let mut canvas = vec![0u32; window_width * window_height];

            for (src_y, row_pixels) in img.rows().enumerate() {
                if src_y < src_start {
                    continue; // skip clipped top rows
                }
                let dst_y = top_pad + (src_y - src_start);
                if dst_y >= window_height {
                    break; // clipped bottom
                }
                let row_start = dst_y * window_width;
                for (src_x, pixel) in row_pixels.enumerate() {
                    let [r, g, b, _] = pixel.0;
                    canvas[row_start + src_x] =
                        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                }
            }

            canvas
        })
        .collect()
}

// ─── Angle → frame index ──────────────────────────────────────────────────────

fn angle_to_frame(angle: f64, num_frames: usize) -> usize {
    let idx = (angle.rem_euclid(360.0) / 360.0 * num_frames as f64) as usize;
    idx.min(num_frames - 1)
}

// ─── MQTT listener ────────────────────────────────────────────────────────────

fn spawn_mqtt_listener(shared_angle: Arc<Mutex<f64>>) {
    thread::spawn(move || {
        let mut opts = MqttOptions::new("rotation-viewer", MQTT_BROKER, MQTT_PORT);
        opts.set_keep_alive(std::time::Duration::from_secs(5));
        let (client, mut connection) = Client::new(opts, 32);
        client.subscribe(MQTT_TOPIC, QoS::AtMostOnce).unwrap();
        println!("[MQTT] Subscribed to {} on {}:{}", MQTT_TOPIC, MQTT_BROKER, MQTT_PORT);

        for event in connection.iter() {
            match event {
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    if let Ok(text) = std::str::from_utf8(&p.payload) {
                        if let Ok(angle) = text.trim().parse::<f64>() {
                            *shared_angle.lock().unwrap() = angle;
                        } else {
                            eprintln!("[MQTT] Bad angle: '{}'", text);
                        }
                    }
                }
                Err(e) => eprintln!("[MQTT] Error: {:?}", e),
                _ => {}
            }
        }
    });
}

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    println!("=== Rotation Viewer ===");
    println!("Sequence 1 : {} @ {}×{} pos ({},{})", IMAGE_SEQUENCE_FOLDER_1, WINDOW_1_W, WINDOW_1_H, WINDOW_1_X, WINDOW_1_Y);
    println!("Sequence 2 : {} @ {}×{} pos ({},{})", IMAGE_SEQUENCE_FOLDER_2, WINDOW_2_W, WINDOW_2_H, WINDOW_2_X, WINDOW_2_Y);
    println!("MQTT broker: {}:{}", MQTT_BROKER, MQTT_PORT);
    println!();

    let seq1 = load_image_sequence(IMAGE_SEQUENCE_FOLDER_1, WINDOW_1_W, WINDOW_1_H);
    let seq2 = load_image_sequence(IMAGE_SEQUENCE_FOLDER_2, WINDOW_2_W, WINDOW_2_H);

    let num_frames_1 = seq1.len();
    let num_frames_2 = seq2.len();

    let shared_angle: Arc<Mutex<f64>> = Arc::new(Mutex::new(0.0));
    spawn_mqtt_listener(Arc::clone(&shared_angle));

    // ── Window 1 ──────────────────────────────────────────────────────────────
    let mut window1 = Window::new(
        "Display 1",
        WINDOW_1_W,
        WINDOW_1_H,
        WindowOptions {
            borderless: true,
            resize: false,
            ..WindowOptions::default()
        },
    )
    .expect("Failed to create window 1");

    window1.set_position(WINDOW_1_X, WINDOW_1_Y);

    // ── Window 2 ──────────────────────────────────────────────────────────────
    let mut window2 = Window::new(
        "Display 2",
        WINDOW_2_W,
        WINDOW_2_H,
        WindowOptions {
            borderless: true,
            resize: false,
            ..WindowOptions::default()
        },
    )
    .expect("Failed to create window 2");

    window2.set_position(WINDOW_2_X, WINDOW_2_Y);

    window1.set_target_fps(60);
    window2.set_target_fps(60);

    println!("[INFO] Running — ESC in either window to quit.");

    // ── Render loop ───────────────────────────────────────────────────────────
    while window1.is_open()
        && window2.is_open()
        && !window1.is_key_down(Key::Escape)
        && !window2.is_key_down(Key::Escape)
    {
        let angle = *shared_angle.lock().unwrap();

        let frame1 = angle_to_frame(angle, num_frames_1);
        let frame2 = angle_to_frame(angle, num_frames_2);

        window1
            .update_with_buffer(&seq1[frame1], WINDOW_1_W, WINDOW_1_H)
            .expect("Window 1 update failed");

        window2
            .update_with_buffer(&seq2[frame2], WINDOW_2_W, WINDOW_2_H)
            .expect("Window 2 update failed");
    }

    println!("[INFO] Exiting.");
}
