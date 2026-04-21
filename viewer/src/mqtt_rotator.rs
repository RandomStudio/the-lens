use rumqttc::{Client, MqttOptions, QoS};
use std::thread;

const BROKER: &str = "10.112.10.10";
const PORT: u16 = 1883;
const TOPIC: &str = "prototype/lens/angle";

pub struct MqttRotator {
    client: Client,
}

impl MqttRotator {
    pub fn new(username: Option<String>, password: Option<String>) -> Self {
        let mut opts = MqttOptions::new("rotation-viewer", BROKER, PORT);
        if let (Some(u), Some(p)) = (username, password) {
            opts.set_credentials(u, p);
        }
        opts.set_keep_alive(std::time::Duration::from_secs(5));
        let (client, mut connection) = Client::new(opts, 32);

        thread::spawn(move || {
            for event in connection.iter() {
                if let Err(e) = event {
                    eprintln!("[MQTT] Error: {:?}", e);
                }
            }
        });

        println!("[MQTT] Publishing to {} on {}:{}", TOPIC, BROKER, PORT);
        Self { client }
    }

    pub fn update(&self, angle: f64) {
        let payload = angle.to_string();
        let _ = self.client.publish(TOPIC, QoS::AtMostOnce, false, payload.as_bytes());
    }
}
