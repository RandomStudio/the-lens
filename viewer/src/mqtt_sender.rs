use crate::config::MqttConfig;
use rumqttc::{Client, MqttOptions, QoS};
use std::thread;

pub struct MqttSender {
    client: Client,
    topic: String,
    debug_topic: Option<String>,
}

impl MqttSender {
    pub fn new(cfg: MqttConfig) -> Self {
        let mut opts = MqttOptions::new("rotation-sender", &cfg.broker, cfg.port);
        opts.set_credentials(cfg.username, cfg.password);
        opts.set_keep_alive(std::time::Duration::from_secs(5));
        let (client, mut connection) = Client::new(opts, 32);

        thread::spawn(move || {
            for event in connection.iter() {
                if let Err(e) = event {
                    eprintln!("[MqttSender] Error: {:?}", e);
                }
            }
        });

        println!("[MqttSender] Publishing to {} on {}:{}", cfg.topic, cfg.broker, cfg.port);
        Self { client, topic: cfg.topic, debug_topic: cfg.debug_topic }
    }

    pub fn update(&self, angle: f64) {
        let payload = angle.to_string();
        let _ = self.client.publish(&self.topic, QoS::AtMostOnce, false, payload.as_bytes());
    }

    pub fn publish_debug(&self, brightness: Option<f64>, scale: Option<f64>) {
        let Some(ref topic) = self.debug_topic else { return };
        let brightness_str = brightness.map_or("null".to_string(), |v| format!("{:.4}", v));
        let scale_str = scale.map_or("null".to_string(), |v| format!("{:.4}", v));
        let payload = format!(r#"{{"brightness":{},"scale":{}}}"#, brightness_str, scale_str);
        let _ = self.client.publish(topic, QoS::AtMostOnce, false, payload.as_bytes());
    }
}
