use crate::config::MqttConfig;
use rumqttc::{Client, MqttOptions, QoS};
use std::thread;

pub struct MqttSender {
    client: Client,
    topic: String,
}

impl MqttSender {
    pub fn new(cfg: &MqttConfig) -> Self {
        let mut opts = MqttOptions::new("rotation-sender", &cfg.broker, cfg.port);
        opts.set_credentials(cfg.username.clone(), cfg.password.clone());
        opts.set_keep_alive(std::time::Duration::from_secs(5));
        let (client, mut connection) = Client::new(opts, 64);

        thread::spawn(move || {
            for _ in connection.iter() {}
        });

        println!("[MqttSender] Publishing to {} on {}:{}", cfg.topic, cfg.broker, cfg.port);
        Self { client, topic: cfg.topic.clone() }
    }

    pub fn publish_angle(&self, angle: f64) {
        let _ = self.client.publish(
            &self.topic,
            QoS::AtMostOnce,
            false,
            format!("{:.4}", angle).as_bytes(),
        );
    }
}
