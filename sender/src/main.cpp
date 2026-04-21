#include <Wire.h>

const uint8_t AS5600_ADDR = 0x36;
const uint8_t RAW_ANGLE_REG = 0x0C;  // high byte; 0x0D is low byte

void setup() {
  Serial.begin(115200);
  Wire.begin();
  Wire.setClock(400000);  // 400kHz fast mode
}

uint16_t readAngle() {
  Wire.beginTransmission(AS5600_ADDR);
  Wire.write(RAW_ANGLE_REG);
  Wire.endTransmission(false);
  Wire.requestFrom(AS5600_ADDR, (uint8_t)2);
  uint16_t hi = Wire.read();
  uint16_t lo = Wire.read();
  return ((hi << 8) | lo) & 0x0FFF;  // 12-bit value, 0–4095
}

void loop() {
  uint16_t raw = readAngle();
  float degrees = (raw * 360.0f) / 4096.0f;
  Serial.println(degrees, 3);
  delay(20);  // 50 Hz
}