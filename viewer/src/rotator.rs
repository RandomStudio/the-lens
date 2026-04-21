use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

const BROKER: &str = "10.112.10.10";
const PORT: u16 = 1883;
const TOPIC: &str = "the-lens/angle";

pub struct Rotator {
    angle: Arc<AtomicU64>,
}

impl Rotator {
    pub fn start() -> Self {
        let angle = Arc::new(AtomicU64::new(0f64.to_bits()));
        let shared = Arc::clone(&angle);

        thread::spawn(move || {
            let mut opts = MqttOptions::new("rotation-viewer", BROKER, PORT);
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

        Self { angle }
    }

    pub fn angle(&self) -> f64 {
        f64::from_bits(self.angle.load(Ordering::Relaxed))
    }
}
