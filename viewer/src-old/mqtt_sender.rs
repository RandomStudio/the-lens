use crate::config::MqttConfig;
use rumqttc::{Client, MqttOptions, QoS};
use std::thread;

fn make_client(id: &str, cfg: &MqttConfig) -> Client {
    let mut opts = MqttOptions::new(id, &cfg.broker, cfg.port);
    opts.set_credentials(cfg.username.clone(), cfg.password.clone());
    opts.set_keep_alive(std::time::Duration::from_secs(5));
    let (client, mut connection) = Client::new(opts, 32);
    let id = id.to_string();
    thread::spawn(move || {
        for event in connection.iter() {
            if let Err(e) = event {
                eprintln!("[{}] Error: {:?}", id, e);
            }
        }
    });
    client
}

pub struct MqttSender {
    client: Client,
    topic: String,
}

impl MqttSender {
    pub fn new(cfg: &MqttConfig) -> Self {
        let client = make_client("rotation-sender", cfg);
        println!("[MqttSender] Publishing to {} on {}:{}", cfg.topic, cfg.broker, cfg.port);
        Self { client, topic: cfg.topic.clone() }
    }

    pub fn update(&self, angle: f64) {
        let payload = angle.to_string();
        let _ = self.client.publish(&self.topic, QoS::AtMostOnce, false, payload.as_bytes());
    }
}

pub struct DebugSender {
    client: Client,
    topic: String,
}

impl DebugSender {
    pub fn new(cfg: &MqttConfig) -> Option<Self> {
        let topic = cfg.debug_topic.clone()?;
        let client = make_client("rotation-debug-sender", cfg);
        println!("[DebugSender] Publishing debug to {} on {}:{}", topic, cfg.broker, cfg.port);
        Some(Self { client, topic })
    }

    pub fn publish(&self, brightness: Option<f64>, scale: Option<f64>) {
        let b = brightness.map_or("null".to_string(), |v| format!("{:.4}", v));
        let s = scale.map_or("null".to_string(), |v| format!("{:.4}", v));
        let payload = format!(r#"{{"brightness":{},"scale":{}}}"#, b, s);
        let _ = self.client.publish(&self.topic, QoS::AtMostOnce, false, payload.as_bytes());
    }
}
