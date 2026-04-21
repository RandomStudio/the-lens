mod config;
mod light;
mod mqtt_receiver;
mod mqtt_sender;
mod receiver;
mod rotator;
mod viewer;

use config::{Config, resolve_index_transform};
use light::Light;
use mqtt_receiver::MqttReceiver;
use mqtt_sender::MqttSender;
use receiver::AngleReceiver;
use rotator::Rotator;
use viewer::{ImageSequence, Viewer};

fn main() {
    let cfg = Config::load("./config.json");

    let receiver: Box<dyn AngleReceiver> = match cfg.receiver.as_str() {
        "mqtt" => Box::new(MqttReceiver::new(cfg.mqtt.clone())),
        _ => Box::new(Rotator::new()),
    };

    let mqtt_sender = if cfg.mqtt_send {
        Some(MqttSender::new(cfg.mqtt))
    } else {
        None
    };

    let sequences: Vec<(ImageSequence, usize)> = cfg.sequences.into_iter().map(|s| {
        let transform = resolve_index_transform(s.index_transform.as_deref());
        let mut seq = ImageSequence::load(&s.path, transform);
        if let Some(scale) = s.scale {
            seq = seq.scale(scale);
        }
        if let Some(swr) = s.scales_with_rotate {
            seq = seq.with_scales_with_rotate(swr.target_index, swr.scale);
        }
        if let Some(bwr) = s.brightness_with_rotate {
            seq = seq.with_brightness_with_rotate(bwr.target_index, bwr.start_brightness, bwr.end_brightness);
        }
        println!("[INFO] '{}' on display {}: {} frames", s.path, s.display, seq.frame_count());
        (seq, s.display)
    }).collect();

    let light = if cfg.light_send {
        Some(Light::new())
    } else {
        None
    };

    let mut viewer = Viewer::new(sequences, cfg.is_debug_display);

    while viewer.is_open() {
        let angle = receiver.angle();
        let _frame_indices = viewer.render(angle);
        let brightness = viewer.brightness_at_angle(angle);
        print!("\rAngle: {:6.2}°, indices: {:?}, brightness: {:.3}", angle, _frame_indices, brightness.unwrap_or(1.0));
        let scale = viewer.scale_at_angle(angle);
        if let Some(ref l) = light {
            l.update(brightness.unwrap_or(1.0));
        }
        if let Some(ref s) = mqtt_sender {
            s.update(angle);
            s.publish_debug(brightness, scale);
        }
    }

    if let Some(ref l) = light {
        l.turn_off();
    }
}
