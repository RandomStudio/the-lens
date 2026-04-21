use crate::config::MqttConfig;
use crate::receiver::AngleReceiver;
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

pub struct MqttReceiver {
    angle: Arc<AtomicU64>,
}

impl MqttReceiver {
    pub fn new(cfg: MqttConfig) -> Self {
        let angle = Arc::new(AtomicU64::new(0f64.to_bits()));
        let shared = Arc::clone(&angle);

        let mut opts = MqttOptions::new("rotation-receiver", &cfg.broker, cfg.port);
        opts.set_credentials(cfg.username, cfg.password);
        opts.set_keep_alive(std::time::Duration::from_secs(5));
        let (client, mut connection) = Client::new(opts, 32);

        client.subscribe(&cfg.topic, QoS::AtMostOnce).expect("MQTT subscribe failed");

        println!("[MqttReceiver] Subscribed to {} on {}:{}", cfg.topic, cfg.broker, cfg.port);

        thread::spawn(move || {
            for event in connection.iter() {
                match event {
                    Ok(Event::Incoming(Packet::Publish(msg))) => {
                        if let Ok(s) = std::str::from_utf8(&msg.payload) {
                            if let Ok(value) = s.trim().parse::<f64>() {
                                shared.store(value.to_bits(), Ordering::Relaxed);
                            }
                        }
                    }
                    Err(e) => eprintln!("[MqttReceiver] Error: {:?}", e),
                    _ => {}
                }
            }
        });

        Self { angle }
    }
}

impl AngleReceiver for MqttReceiver {
    fn angle(&self) -> f64 {
        f64::from_bits(self.angle.load(Ordering::Relaxed))
    }
}
