use crate::settings::{load_settings};
use crate::capture::{capture_frame, calculate_average_color, smooth_colors};
use crate::api::{send_rgb};
use captrs::Capturer;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::spawn;
use log::{info, debug, error};
use env_logger::Env;
use colored::*;
use std::collections::{HashMap, HashSet};

mod settings;
mod capture;
mod api;
mod models;

fn rgb_to_ansi_color(r: u32, g: u32, b: u32) -> ColoredString {
    let color_string = format!("⬤");
    color_string.truecolor(r as u8, g as u8, b as u8)
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("Loading settings from 'settings.json'.");
    let settings = load_settings("settings.json");
    let client = Arc::new(reqwest::Client::new());

    info!("Initializing capture with monitor ID: {}", settings.monitor_id);
    let mut capturer = Capturer::new(settings.monitor_id as usize).expect("Failed to get Capture Object");
    let mut prev_avg_colors: HashMap<String, (u32, u32, u32)> = HashMap::new();

    loop {
        if let Some(frame) = capture_frame(&mut capturer) {
            debug!("Frame captured successfully.");

            // Collect unique positions
            let unique_positions: Vec<String> = settings.lights.iter()
                .map(|light| light.position.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

            // Calculate color once per unique position
            let position_colors: HashMap<String, [u32; 3]> = unique_positions.iter()
                .map(|pos| (pos.clone(), calculate_average_color(&frame, pos, settings.skip_pixels)))
                .collect();

            for light in settings.lights.iter() {
                let new_color = position_colors.get(&light.position).unwrap();
                let smoothed_color = smooth_colors(
                    *prev_avg_colors.entry(light.position.clone()).or_insert((0, 0, 0)),
                    *new_color,
                    settings.smoothing_factor
                );

                prev_avg_colors.insert(light.position.clone(), smoothed_color);
                let brightness = smoothed_color.0.max(smoothed_color.1).max(smoothed_color.2);

                let api_endpoint = settings.api_endpoint.clone();
                let token = settings.token.clone();
                spawn(send_rgb(Arc::clone(&client), api_endpoint, token, vec![smoothed_color.0, smoothed_color.1, smoothed_color.2], brightness, light.entity_name.clone()));
                let color_circle = rgb_to_ansi_color(smoothed_color.0, smoothed_color.1, smoothed_color.2);
                info!("Sending RGB data for {} at position {}: {}", light.entity_name, light.position, color_circle);
            }
        } else {
            error!("Failed to capture frame.");
        }

        thread::sleep(Duration::from_millis(settings.grab_interval as u64));
        debug!("Sleeping for {} milliseconds", settings.grab_interval);
    }
}

