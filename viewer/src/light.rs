use std::net::UdpSocket;

const ARTNET_ADDR: &str = "255.255.255.255:6454";

// Aputure INFINIMAT startChannel 87 (1-indexed) → 0-indexed offset 86
const INFINIMAT_START: usize = 86;

pub struct Light {
    socket: UdpSocket,
}

impl Light {
    pub fn new() -> Self {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind ArtNet socket");
        socket.set_broadcast(true).expect("Failed to enable broadcast");
        Self { socket }
    }

    pub fn turn_off(&self) {
        let dmx = [0u8; 512];
        let packet = artnet_dmx_packet(0, &dmx);
        let _ = self.socket.send_to(&packet, ARTNET_ADDR);
    }

    pub fn update(&self, brightness: f64) {
        let intensity = (brightness.clamp(0.0, 1.0) * 255.0).round() as u8;

        let mut dmx = [0u8; 512];

        // Mode 4: RGBWW 8-bit — studio white via RGB channels (same layout that worked before)
        dmx[INFINIMAT_START]     = intensity; // Ch1: Intensity
        dmx[INFINIMAT_START + 1] = 255;       // Ch2: Red
        dmx[INFINIMAT_START + 2] = 240;       // Ch3: Green  (~5500 K studio white)
        dmx[INFINIMAT_START + 3] = 220;       // Ch4: Blue

        let packet = artnet_dmx_packet(0, &dmx);
        let _ = self.socket.send_to(&packet, ARTNET_ADDR);
    }
}

fn artnet_dmx_packet(universe: u16, dmx: &[u8; 512]) -> Vec<u8> {
    let mut p = Vec::with_capacity(18 + 512);
    p.extend_from_slice(b"Art-Net\0");
    p.push(0x00); p.push(0x50);           // OpCode ArtDmx, little-endian
    p.push(0x00); p.push(0x0e);           // protocol version 14, big-endian
    p.push(0);                             // sequence (0 = disabled)
    p.push(0);                             // physical
    p.push((universe & 0xff) as u8);      // universe low byte
    p.push(((universe >> 8) & 0x7f) as u8); // universe high byte (15-bit)
    p.push(0x02); p.push(0x00);           // length 512, big-endian
    p.extend_from_slice(dmx);
    p
}
