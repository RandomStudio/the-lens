#!/usr/bin/python3
"""
AS5600 magnetic rotation sensor → MQTT sender.

Reads angle from the AS5600 over I²C and publishes it as a UTF-8
float string to an MQTT broker at a fixed interval.

Hardware setup:
  Pi GND        → AS5600 GND + DIR
  Pi 3.3 V      → AS5600 VCC
  Pi I²C SCL    → AS5600 SCL  (GPIO 3, pin 5)
  Pi I²C SDA    → AS5600 SDA  (GPIO 2, pin 3)

Enable I²C with: sudo raspi-config  (Interface Options → I2C)
Install smbus:   sudo apt-get install python3-smbus
Install mqtt:    sudo apt-get install python3-paho-mqtt
"""

import smbus
import time
import paho.mqtt.client as mqtt

# ── AS5600 constants ──────────────────────────────────────────────────────────

DEVICE_ADDR = 0x36
ANGLE_REG   = 0x0C   # 12-bit raw angle (2 bytes, big-endian)
MAX_RAW     = 4096   # AS5600 resolution

# ── MQTT config ───────────────────────────────────────────────────────────────

MQTT_BROKER = "10.112.10.10"
MQTT_PORT   = 1883
MQTT_TOPIC  = "the-lens/angle"
SEND_HZ     = 60              # publish rate

# ─────────────────────────────────────────────────────────────────────────────

def read_raw_angle(bus: smbus.SMBus) -> int:
    data = bus.read_i2c_block_data(DEVICE_ADDR, ANGLE_REG, 2)
    return (data[0] << 8) | data[1]


def to_degrees(raw: int, offset: int) -> float:
    """Convert a raw 12-bit reading to degrees [0, 360), zeroed at offset."""
    corrected = (raw - offset) & (MAX_RAW - 1)
    return corrected * 360.0 / MAX_RAW


def main() -> None:
    bus    = smbus.SMBus(1)
    offset = read_raw_angle(bus)
    print(f"Zeroed at raw offset: {offset}")

    client = mqtt.Client(mqtt.CallbackAPIVersion.VERSION2)
    client.connect(MQTT_BROKER, MQTT_PORT, keepalive=60)
    client.loop_start()

    interval = 1.0 / SEND_HZ
    print(f"Publishing to {MQTT_BROKER}:{MQTT_PORT} topic={MQTT_TOPIC} at {SEND_HZ} Hz — Ctrl-C to stop")

    try:
        while True:
            start = time.monotonic()
            angle = to_degrees(read_raw_angle(bus), offset)

            try:
                client.publish(MQTT_TOPIC, f"{angle:.2f}")
            except Exception:
                pass  # Broker unreachable — keep reading sensor, retry next tick

            elapsed = time.monotonic() - start
            time.sleep(max(0.0, interval - elapsed))
    except KeyboardInterrupt:
        print("\nStopped.")
    finally:
        client.loop_stop()
        client.disconnect()
        bus.close()


if __name__ == "__main__":
    main()