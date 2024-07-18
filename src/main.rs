#[allow(unused_must_use)]

extern crate captrs;
extern crate reqwest;

use captrs::*;
use serde::{Deserialize, Serialize};
use std::{time::Duration};
use console::Emoji;

#[derive(Serialize, Deserialize)]
struct LightConfig {
    entity_name: String,
    position: String, // Possible values: "top", "bottom", "left", "right"
}

#[derive(Serialize, Deserialize)]
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

fn capture_frame(capturer: &mut Capturer) -> Frame {
    let (width, height) = capturer.geometry();
    let buffer = match capturer.capture_frame() {
        Ok(frame) => frame,
        Err(error) => {
            println!("{} Failed to grab frame: {:?}", Emoji("❗ ", ""), error);
            std::thread::sleep(Duration::from_millis(100));
            return capture_frame(capturer);
        }
    };

    Frame {
        width: width as u32,
        height: height as u32,
        buffer: buffer.iter().flat_map(|pixel| vec![pixel.r, pixel.g, pixel.b]).collect(),
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
    rgb_vec: &Vec<u64>,
    brightness: &u64,
    entity_name: &String,
) {
    let api_body = HASSApiBody {
        entity_id: entity_name.clone(),
        rgb_color: [rgb_vec[0], rgb_vec[1], rgb_vec[2]],
        brightness: *brightness,
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

    let steps = settings.skip_pixels as u64;
    let grab_interval = settings.grab_interval as u64;
    let smoothing_factor = settings.smoothing_factor;

    // create a capture device
    let mut capturer =
        Capturer::new(settings.monitor_id as usize)
            .expect("❌ Failed to get Capture Object");

    // get the resolution of the monitor
    let (w, h) = capturer.geometry();
    let size = (w as u64 * h as u64) / steps;

    // create http client
    let client = reqwest::Client::new();

    let (mut prev_r, mut prev_g, mut prev_b) = (0, 0, 0);

    println!();

    let mut last_timestamp = std::time::Instant::now();

    loop {
        // Capture frame and calculate average colors for different positions
        let frame = capture_frame(&mut capturer);
        let top_avg = calculate_average_color(&frame, "top");
        let bottom_avg = calculate_average_color(&frame, "bottom");
        let left_avg = calculate_average_color(&frame, "left");
        let right_avg = calculate_average_color(&frame, "right");

        // Placeholder for brightness calculation
        let brightness = 128;

        for light in &settings.lights {
            let avg_color = match light.position.as_str() {
                "top" => &top_avg,
                "bottom" => &bottom_avg,
                "left" => &left_avg,
                "right" => &right_avg,
                _ => &top_avg, // Default case
            };
            send_rgb(&client, &settings, avg_color, &brightness, &light.entity_name).await;
        }

        std::thread::sleep(Duration::from_millis(settings.grab_interval as u64));
    }
}
