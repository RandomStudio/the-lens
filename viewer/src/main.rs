mod config;
mod debug_display;
mod display;
mod easing;
mod image_sequence;
mod light;
mod mqtt_receiver;
mod mqtt_sender;
mod receiver;
mod rotator;

use config::{Config, resolve_index_transform};
use debug_display::DebugDisplay;
use display::Display;
use mqtt_receiver::MqttReceiver;
use mqtt_sender::MqttSender;
use receiver::AngleReceiver;
use rotator::Rotator;

fn main() {
    let cfg = Config::load("./config.json");
    let transform = resolve_index_transform(&cfg.index_transform);

    if cfg.is_debug_screen {
        let receiver = MqttReceiver::new(cfg.mqtt.clone());
        let mut disp = DebugDisplay::new(&cfg.sequence_path, transform, cfg.max_scale, cfg.brightest_brightness, cfg.easing_multiplier);
        while disp.is_open() {
            let angle = receiver.angle();
            disp.render(angle);
        }
    } else {
        let receiver = Rotator::new();
        let sender = MqttSender::new(&cfg.mqtt);
        let mut disp = Display::new(&cfg.sequence_path, &cfg.diamond_path, transform, cfg.max_scale, cfg.brightest_brightness, cfg.easing_multiplier);
        while disp.is_open() {
            let angle = receiver.angle();
            sender.publish_angle(angle);
            disp.render(angle);
        }
        disp.turn_off_light();
    }
}
