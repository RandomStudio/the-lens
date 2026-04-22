use crate::receiver::AngleReceiver;
use std::io::BufRead;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct Rotator {
    angle: Arc<AtomicU64>,
}

impl Rotator {
    pub fn new() -> Self {
        let angle = Arc::new(AtomicU64::new(0f64.to_bits()));
        let shared = Arc::clone(&angle);

        thread::spawn(move || loop {
            let port_name = match find_teensy_port() {
                Some(p) => p,
                None => {
                    eprintln!("[Rotator] No Teensy found, retrying...");
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
            };

            println!("[Rotator] Opening {}", port_name);

            let port = match serialport::new(&port_name, 115200)
                .timeout(Duration::from_secs(2))
                .open()
            {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[Rotator] Failed to open {}: {}", port_name, e);
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
            };

            let reader = std::io::BufReader::new(port);
            for line in reader.lines() {
                match line {
                    Ok(s) => {
                        if let Ok(value) = s.trim().parse::<f64>() {
                            shared.store(value.to_bits(), Ordering::Relaxed);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        eprintln!("[Rotator] Timeout — is Teensy sending?");
                    }
                    Err(e) => {
                        eprintln!("[Rotator] Read error: {}", e);
                        break;
                    }
                }
            }

            eprintln!("[Rotator] Disconnected, retrying...");
            thread::sleep(Duration::from_secs(2));
        });

        Self { angle }
    }

}

impl AngleReceiver for Rotator {
    fn angle(&self) -> f64 {
        f64::from_bits(self.angle.load(Ordering::Relaxed))
    }
}

fn find_teensy_port() -> Option<String> {
    let ports = serialport::available_ports().ok()?;
    ports
        .into_iter()
        .find(|p| p.port_name.contains("usbmodem"))
        .map(|p| p.port_name)
}
