#[allow(unused_must_use)]

extern crate captrs;
extern crate reqwest;

use captrs::*;
use serde::{Deserialize, Serialize};
use std::{time::Duration};
use console::Emoji;
use tokio::task;

#[derive(Serialize, Deserialize, Clone)]
struct LightConfig {
    entity_name: String,
    position: String, // Possible values: "top", "bottom", "left", "right"
}

#[derive(Serialize, Deserialize, Clone)]
struct Settings {
    api_endpoint: String,
    lights: Vec<LightConfig>,
    token: String,
    grab_interval: i16,
    skip_pixels: i16,
    smoothing_factor: f32,
    monitor_id: i16,
}

#[derive(Serialize, Deserialize)]
struct HASSApiBody {
    entity_id: String,
    rgb_color: [u64; 3],
    brightness: u64,
}

struct Frame {
    width: u32,
    height: u32,
    buffer: Vec<u8>,
}

fn capture_frame(capturer: &mut Capturer) -> Option<Frame> {
    let (width, height) = capturer.geometry();
    match capturer.capture_frame() {
        Ok(frame) => Some(Frame {
            width: width as u32,
            height: height as u32,
            buffer: frame.iter().flat_map(|pixel| vec![pixel.r, pixel.g, pixel.b]).collect(),
        }),
        Err(error) => {
            println!("{} Failed to grab frame: {:?}", Emoji("❗ ", ""), error);
            None
        }
    }
}

fn calculate_average_color(frame: &Frame, position: &str) -> Vec<u64> {
    let (x_start, x_end, y_start, y_end) = match position {
        "top" => (0, frame.width, 0, frame.height / 3),
        "bottom" => (0, frame.width, 2 * frame.height / 3, frame.height),
        "left" => (0, frame.width / 3, 0, frame.height),
        "right" => (2 * frame.width / 3, frame.width, 0, frame.height),
        _ => (0, frame.width, 0, frame.height),
    };

    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;
    let mut count = 0u64;

    for y in y_start..y_end {
        for x in (x_start..x_end).step_by(3) {
            let offset = ((y * frame.width + x) * 3) as usize;
            r_sum += frame.buffer[offset] as u64;
            g_sum += frame.buffer[offset + 1] as u64;
            b_sum += frame.buffer[offset + 2] as u64;
            count += 1;
        }
    }

    vec![r_sum / count, g_sum / count, b_sum / count]
}

async fn send_rgb(
    client: &reqwest::Client,
    settings: &Settings,
    rgb_vec: Vec<u64>,
    brightness: u64,
    entity_name: String,
) {
    let api_body = HASSApiBody {
        entity_id: entity_name,
        rgb_color: [rgb_vec[0], rgb_vec[1], rgb_vec[2]],
        brightness,
    };

    let url = format!("{}/api/services/light/turn_on", settings.api_endpoint);
    let token = format!("Bearer {}", settings.token);

    client
        .post(&url)
        .header("Authorization", token)
        .json(&api_body)
        .send()
        .await
        .expect("Failed to send RGB data");
}

#[tokio::main]
async fn main() {
    let term = console::Term::stdout();
    term.set_title("HASS-Light-Sync running...");

    println!("{}hass-light-sync - Starting...", Emoji("💡 ", ""));
    println!("{}Reading config...", Emoji("⚙️ ", ""));
    // read settings
    let settingsfile =
        std::fs::read_to_string("settings.json").expect("❌ settings.json file does not exist");

    let settings: Settings =
        serde_json::from_str(settingsfile.as_str()).expect("❌ Failed to parse settings. Please read the configuration section in the README");

    println!("{}Config loaded successfully!", Emoji("✅ ", ""));

    let grab_interval = settings.grab_interval as u64;
    let smoothing_factor = settings.smoothing_factor;

    // create a capture device
    let mut capturer =
        Capturer::new(settings.monitor_id as usize)
            .expect("❌ Failed to get Capture Object");

    // create http client
    let client = reqwest::Client::new();

    let mut prev_avg_colors = vec![(0u64, 0u64, 0u64); settings.lights.len()];

    loop {
        // Capture frame and skip if no frame is fetched
        let frame = match capture_frame(&mut capturer) {
            Some(frame) => frame,
            None => continue,
        };

        let mut tasks = Vec::new();

        for (i, light) in settings.lights.iter().enumerate() {
            let avg_color = calculate_average_color(&frame, &light.position);

            let (sm_r, sm_g, sm_b) = (
                smoothing_factor * prev_avg_colors[i].0 as f32 + (1.0 - smoothing_factor) * avg_color[0] as f32,
                smoothing_factor * prev_avg_colors[i].1 as f32 + (1.0 - smoothing_factor) * avg_color[1] as f32,
                smoothing_factor * prev_avg_colors[i].2 as f32 + (1.0 - smoothing_factor) * avg_color[2] as f32,
            );

            prev_avg_colors[i] = (sm_r as u64, sm_g as u64, sm_b as u64);

            let smoothed_avg_color = vec![sm_r as u64, sm_g as u64, sm_b as u64];

            let brightness = *smoothed_avg_color.iter().max().unwrap();

            let client_clone = client.clone();
            let settings_clone = settings.clone();
            let entity_name = light.entity_name.clone();

            let task = task::spawn(async move {
                send_rgb(&client_clone, &settings_clone, smoothed_avg_color, brightness, entity_name).await;
            });

            tasks.push(task);
        }

        for task in tasks {
            task.await.unwrap();
        }

        std::thread::sleep(Duration::from_millis(grab_interval));
    }
}
