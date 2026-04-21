use serialport::SerialPort;
use std::io::{BufRead, BufReader};
use std::time::Duration;

fn find_teensy_port() -> Option<String> {
    let ports = serialport::available_ports().ok()?;
    for port in ports {
        // On macOS, Teensy shows up as /dev/tty.usbmodem*
        if port.port_name.contains("usbmodem") {
            return Some(port.port_name);
        }
    }
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port_name = std::env::args()
        .nth(1)
        .or_else(find_teensy_port)
        .ok_or("No Teensy found. Plug it in or pass the port path as an argument.")?;

    println!("Opening {}", port_name);

    let port = serialport::new(&port_name, 115200)
        .timeout(Duration::from_secs(2))
        .open()?;

    let reader = BufReader::new(port);

    for line in reader.lines() {
        match line {
            Ok(s) => {
                let trimmed = s.trim();
                match trimmed.parse::<f32>() {
                    Ok(angle) => {
                        println!("angle: {:7.3}°", angle);
                    }
                    Err(_) => {
                        // Probably a startup message or garbled byte — show it raw
                        eprintln!("(non-numeric) {}", trimmed);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                eprintln!("timeout waiting for data — is the Teensy sending?");
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}