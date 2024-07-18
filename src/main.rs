use captrs::*;
use serde::{Deserialize, Serialize};
use std::{time::Duration, sync::Arc, thread};
use reqwest;
use rayon::prelude::*;

#[derive(Serialize, Deserialize, Clone)]
struct LightConfig {
    entity_name: String,
    position: String,
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
    rgb_color: [u32; 3],
    brightness: u32,
}

#[derive(Clone)]
struct Frame {
    width: u32,
    height: u32,
    buffer: Vec<u8>,
}

fn capture_frame(capturer: &mut Capturer) -> Option<Frame> {
    let (width, height) = capturer.geometry();  // Get the dimensions directly from capturer
    capturer.capture_frame().ok().map(|frame| {
        Frame {
            width: width as u32,
            height: height as u32,
            buffer: frame.into_iter().flat_map(|pixel| vec![pixel.r, pixel.g, pixel.b]).collect(),
        }
    })
}

fn calculate_average_color(frame: &Frame, position: &str, skip_pixels: i16) -> Vec<u32> {
    let (x_start, x_end, y_start, y_end) = match position {
        "top" => (0, frame.width, 0, frame.height / 3),
        "bottom" => (0, frame.width, 2 * frame.height / 3, frame.height),
        "left" => (0, frame.width / 3, 0, frame.height),
        "right" => (2 * frame.width / 3, frame.width, 0, frame.height),
        _ => (0, frame.width, 0, frame.height),
    };

    let mut color_sum = (0u64, 0u64, 0u64);
    let mut count = 0u64;
    for y in (y_start..y_end).step_by(skip_pixels as usize) {
        for x in (x_start..x_end).step_by(3 * skip_pixels as usize) {
            let offset = (y * frame.width + x) * 3;
            color_sum.0 += frame.buffer[offset as usize] as u64;
            color_sum.1 += frame.buffer[offset as usize + 1] as u64;
            color_sum.2 += frame.buffer[offset as usize + 2] as u64;
            count += 1;
        }
    }

    vec![(color_sum.0 / count) as u32, (color_sum.1 / count) as u32, (color_sum.2 / count) as u32]
}

async fn send_rgb(
    client: Arc<reqwest::Client>,
    api_endpoint: String,
    token: String,
    rgb_vec: Vec<u32>,
    brightness: u32,
    entity_name: String,
) {
    let api_body = HASSApiBody {
        entity_id: entity_name,
        rgb_color: [rgb_vec[0], rgb_vec[1], rgb_vec[2]],
        brightness,
    };

    let url = format!("{}/api/services/light/turn_on", api_endpoint);
    client.post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&api_body)
        .send()
        .await
        .expect("Failed to send RGB data");
}

#[tokio::main]
async fn main() {
    let settingsfile = std::fs::read_to_string("settings.json").expect("settings.json file does not exist");
    let settings: Settings = serde_json::from_str(&settingsfile).expect("Failed to parse settings");

    let client = Arc::new(reqwest::Client::new());
    let mut capturer = Capturer::new(settings.monitor_id as usize).expect("Failed to get Capture Object");
    let mut prev_avg_colors = vec![(0u32, 0u32, 0u32); settings.lights.len()];

    loop {
        if let Some(frame) = capture_frame(&mut capturer) {
            let avg_colors: Vec<_> = settings.lights.par_iter().map(|light| {
                calculate_average_color(&frame, &light.position, settings.skip_pixels)
            }).collect();

            for (i, avg_color) in avg_colors.into_iter().enumerate() {
                let (sm_r, sm_g, sm_b) = (
                    (settings.smoothing_factor * prev_avg_colors[i].0 as f32 + (1.0 - settings.smoothing_factor) * avg_color[0] as f32) as u32,
                    (settings.smoothing_factor * prev_avg_colors[i].1 as f32 + (1.0 - settings.smoothing_factor) * avg_color[1] as f32) as u32,
                    (settings.smoothing_factor * prev_avg_colors[i].2 as f32 + (1.0 - settings.smoothing_factor) * avg_color[2] as f32) as u32,
                );

                prev_avg_colors[i] = (sm_r, sm_g, sm_b);
                let smoothed_avg_color = vec![sm_r, sm_g, sm_b];
                let brightness = *smoothed_avg_color.iter().max().unwrap_or(&0);

                let api_endpoint = settings.api_endpoint.clone();
                let token = settings.token.clone();

                tokio::spawn(send_rgb(Arc::clone(&client), api_endpoint, token, smoothed_avg_color, brightness, settings.lights[i].entity_name.clone()));
            }
        }

        thread::sleep(Duration::from_millis(settings.grab_interval as u64));
    }
}
