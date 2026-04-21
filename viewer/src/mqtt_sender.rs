use crate::config::MqttConfig;
use rumqttc::{Client, MqttOptions, QoS};
use std::thread;

pub struct MqttSender {
    client: Client,
    topic: String,
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
        Self { client, topic: cfg.topic }
    }

    pub fn update(&self, angle: f64) {
        let payload = angle.to_string();
        let _ = self.client.publish(&self.topic, QoS::AtMostOnce, false, payload.as_bytes());
    }
}
