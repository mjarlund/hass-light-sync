use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct LightConfig {
    pub entity_name: String,
    pub position: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub api_endpoint: String,
    pub lights: Vec<LightConfig>,
    pub token: String,
    pub grab_interval: i16,
    pub skip_pixels: i16,
    pub smoothing_factor: f32,
    pub monitor_id: i16,
}

pub fn load_settings(path: &str) -> Settings {
    let settings_file = fs::read_to_string(path).expect("settings.json file does not exist");
    serde_json::from_str(&settings_file).expect("Failed to parse settings")
}
