use serde::{Serialize, Deserialize};

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
    #[serde(default = "default_grab_interval")]
    pub grab_interval: i16,
    #[serde(default = "default_smoothing_factor")]
    pub smoothing_factor: f32,
    #[serde(default = "default_monitor_id")]
    pub monitor_id: i16,
    #[serde(default = "default_enable_api_calls")]
    pub enable_api_calls: bool,
}

fn default_grab_interval() -> i16 {
    1000
}

fn default_smoothing_factor() -> f32 {
    0.5
}

fn default_monitor_id() -> i16 {
    1
}

fn default_enable_api_calls() -> bool {
    true
}

pub fn load_settings(file_path: &str) -> Settings {
    use std::fs::File;
    use std::io::Read;

    let mut file = match File::open(file_path) {
        Ok(file) => file,
        Err(_) => panic!("❌ Settings file not found. A settings file is required."),
    };

    let mut contents = String::new();
    if file.read_to_string(&mut contents).is_ok() {
        if let Ok(settings) = serde_json::from_str(&contents) {
            return settings;
        }
    }

    panic!("❌ Failed to read or parse the settings file. Please check the file format and content.");
}