use serde::Deserialize;

fn default_lerp_speed() -> f64 { 8.0 }

#[derive(Deserialize, Clone)]
pub struct MqttConfig {
    pub broker: String,
    pub port: u16,
    pub topic: String,
    pub username: String,
    pub password: String,
    #[serde(default = "default_lerp_speed")]
    pub lerp_speed: f64,
}

#[derive(Deserialize, Clone)]
pub struct ScalesWithRotateConfig {
    pub target_index: usize,
    pub scale: f64,
}

#[derive(Deserialize, Clone)]
pub struct BrightnessWithRotateConfig {
    pub target_index: usize,
    pub start_brightness: f64,
    pub end_brightness: f64,
}

#[derive(Deserialize)]
pub struct SequenceConfig {
    pub path: String,
    pub display: usize,
    pub hue_shift: Option<i32>,
    pub hue_opacity: Option<f64>,
    pub scale: Option<f64>,
    pub scales_with_rotate: Option<ScalesWithRotateConfig>,
    pub brightness_with_rotate: Option<BrightnessWithRotateConfig>,
    /// Named index transform. Options: see `resolve_index_transform()`
    pub index_transform: Option<String>,
}

#[derive(Deserialize)]
pub struct Config {
    pub mqtt: MqttConfig,
    /// Angle source: "rotator" or "mqtt"
    pub receiver: String,
    pub mqtt_send: bool,
    pub light_send: bool,
    pub sequences: Vec<SequenceConfig>,
    #[serde(default)]
    pub is_debug_display: bool,
}

impl Config {
    pub fn load(path: &str) -> Self {
        let data = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read config '{}': {}", path, e));
        serde_json::from_str(&data)
            .unwrap_or_else(|e| panic!("Failed to parse config '{}': {}", path, e))
    }
}

pub fn resolve_index_transform(name: Option<&str>) -> fn(isize, isize) -> isize {
    match name {
        Some("reverse_quarter") => |index, total| total - index - (total / 4),
        Some("identity") | None => |idx, _| idx,
        Some(other) => panic!("Unknown index_transform '{}'. Options: identity, reverse_quarter", other),
    }
}
