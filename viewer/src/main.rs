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
use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::new().unwrap();
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
        if let Some(hue) = s.hue_shift {
            seq = seq.hue_shift(hue);
        }
        if let Some(opacity) = s.hue_opacity {
            seq = seq.hue_opacity(opacity);
        }
        if let Some(scale) = s.scale {
            seq = seq.scale(scale);
        }
        println!("[INFO] '{}' on display {}: {} frames", s.path, s.display, seq.frame_count());
        (seq, s.display)
    }).collect();

    let light = if cfg.light_send {
        Some(Light::new())
    } else {
        None
    };

    let viewer = Viewer::new(&event_loop, sequences);
    viewer.run(event_loop, receiver, light, mqtt_sender);
}
