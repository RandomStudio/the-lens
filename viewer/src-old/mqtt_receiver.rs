use crate::config::MqttConfig;
use crate::receiver::AngleReceiver;
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

pub struct MqttReceiver {
    target: Arc<AtomicU64>,
    current: Cell<f64>,
    last_time: Cell<Instant>,
    lerp_speed: f64,
}

impl MqttReceiver {
    pub fn new(cfg: MqttConfig) -> Self {
        let lerp_speed = cfg.lerp_speed;
        let target = Arc::new(AtomicU64::new(0f64.to_bits()));
        let shared = Arc::clone(&target);

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

        Self {
            target,
            current: Cell::new(0.0),
            last_time: Cell::new(Instant::now()),
            lerp_speed,
        }
    }
}

impl AngleReceiver for MqttReceiver {
    fn angle(&self) -> f64 {
        let now = Instant::now();
        let dt = now.duration_since(self.last_time.get()).as_secs_f64();
        self.last_time.set(now);

        let target = f64::from_bits(self.target.load(Ordering::Relaxed));

        if self.lerp_speed == 0.0 {
            return target;
        }

        let current = self.current.get();
        let delta = ((target - current + 180.0).rem_euclid(360.0)) - 180.0;
        let alpha = 1.0 - (-self.lerp_speed * dt).exp();
        let smoothed = (current + delta * alpha).rem_euclid(360.0);
        self.current.set(smoothed);
        smoothed
    }
}
