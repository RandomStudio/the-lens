use serde::Deserialize;

fn default_lerp_speed() -> f64 { 8.0 }
fn default_diamond_path() -> String { "./sequences/diamond".to_string() }
fn default_index_transform() -> String { "identity".to_string() }
fn default_min_scale() -> f64 { 0.3 }
fn default_max_scale() -> f64 { 2.0 }
fn default_brightest_brightness() -> f64 { 1.0 }
fn default_easing_multiplier() -> f64 { 1.0 }

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

#[derive(Deserialize)]
pub struct Config {
    pub mqtt: MqttConfig,
    pub sequence_path: String,
    #[serde(default = "default_diamond_path")]
    pub diamond_path: String,
    #[serde(default = "default_index_transform")]
    pub index_transform: String,
    #[serde(default)]
    pub is_debug_screen: bool,
    #[serde(default = "default_min_scale")]
    pub min_scale: f64,
    #[serde(default = "default_max_scale")]
    pub max_scale: f64,
    #[serde(default = "default_brightest_brightness")]
    pub brightest_brightness: f64,
    #[serde(default = "default_easing_multiplier")]
    pub easing_multiplier: f64,
}

impl Config {
    pub fn load(path: &str) -> Self {
        let data = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read config '{}': {}", path, e));
        serde_json::from_str(&data)
            .unwrap_or_else(|e| panic!("Failed to parse config '{}': {}", path, e))
    }
}

pub fn resolve_index_transform(name: &str) -> fn(isize, isize) -> isize {
    match name {
        "reverse_quarter" => |index, total| total - index - (total / 4),
        "identity" | "" => |idx, _| idx,
        other => panic!("Unknown index_transform '{}'. Options: identity, reverse_quarter", other),
    }
}
