use crate::config::MqttConfig;
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

pub struct DebugState {
    pub brightness: AtomicU64,
    pub scale: AtomicU64,
}

impl DebugState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            brightness: AtomicU64::new(f64::NAN.to_bits()),
            scale: AtomicU64::new(f64::NAN.to_bits()),
        })
    }

    pub fn brightness(&self) -> Option<f64> {
        let v = f64::from_bits(self.brightness.load(Ordering::Relaxed));
        if v.is_nan() { None } else { Some(v) }
    }

    pub fn scale(&self) -> Option<f64> {
        let v = f64::from_bits(self.scale.load(Ordering::Relaxed));
        if v.is_nan() { None } else { Some(v) }
    }
}

pub struct DebugReceiver {
    pub state: Arc<DebugState>,
}

impl DebugReceiver {
    pub fn new(cfg: &MqttConfig) -> Option<Self> {
        let topic = cfg.debug_topic.as_ref()?;
        let state = DebugState::new();
        let shared = Arc::clone(&state);

        let mut opts = MqttOptions::new("rotation-debug-receiver", &cfg.broker, cfg.port);
        opts.set_credentials(cfg.username.clone(), cfg.password.clone());
        opts.set_keep_alive(std::time::Duration::from_secs(5));
        let (client, mut connection) = Client::new(opts, 32);

        client.subscribe(topic, QoS::AtMostOnce).expect("MQTT debug subscribe failed");
        println!("[DebugReceiver] Subscribed to {} on {}:{}", topic, cfg.broker, cfg.port);

        thread::spawn(move || {
            for event in connection.iter() {
                match event {
                    Ok(Event::Incoming(Packet::Publish(msg))) => {
                        if let Ok(s) = std::str::from_utf8(&msg.payload) {
                            let brightness = parse_field(s, "brightness");
                            let scale = parse_field(s, "scale");
                            let b_bits = brightness.unwrap_or(f64::NAN).to_bits();
                            let s_bits = scale.unwrap_or(f64::NAN).to_bits();
                            shared.brightness.store(b_bits, Ordering::Relaxed);
                            shared.scale.store(s_bits, Ordering::Relaxed);
                        }
                    }
                    Err(e) => eprintln!("[DebugReceiver] Error: {:?}", e),
                    _ => {}
                }
            }
        });

        Some(Self { state })
    }
}

fn parse_field(json: &str, field: &str) -> Option<f64> {
    let key = format!("\"{}\":", field);
    let start = json.find(&key)? + key.len();
    let rest = json[start..].trim_start();
    if rest.starts_with("null") {
        return None;
    }
    let end = rest.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-').unwrap_or(rest.len());
    rest[..end].parse().ok()
}
