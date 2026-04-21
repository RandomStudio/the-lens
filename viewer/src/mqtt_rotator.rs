use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use crate::rotator::Rotator;

const BROKER: &str = "10.112.10.10";
const PORT: u16 = 1883;
const TOPIC: &str = "prototype/lens/angle";

pub struct MqttRotator {
    angle: Arc<AtomicU64>,
    username: Option<String>,
    password: Option<String>,
}

impl MqttRotator {
    pub fn new(rotator: &Rotator, username: Option<String>, password: Option<String>) -> Self {
        Self {
            angle: rotator.shared(),
            username,
            password,
        }
    }

    pub fn start(&self) {
        let shared = Arc::clone(&self.angle);
        let username = self.username.clone();
        let password = self.password.clone();
        thread::spawn(move || {
            let mut opts = MqttOptions::new("rotation-viewer", BROKER, PORT);
            if let (Some(u), Some(p)) = (username, password) {
                opts.set_credentials(u, p);
            }
            opts.set_keep_alive(std::time::Duration::from_secs(5));
            let (client, mut connection) = Client::new(opts, 32);
            client.subscribe(TOPIC, QoS::AtMostOnce).unwrap();
            println!("[MQTT] Subscribed to {} on {}:{}", TOPIC, BROKER, PORT);

            for event in connection.iter() {
                match event {
                    Ok(Event::Incoming(Packet::Publish(p))) => {
                        if let Ok(text) = std::str::from_utf8(&p.payload) {
                            if let Ok(value) = text.trim().parse::<f64>() {
                                shared.store(value.to_bits(), Ordering::Relaxed);
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

    pub fn update(&self, value: f64) {
        self.angle.store(value.to_bits(), Ordering::Relaxed);
    }
}
