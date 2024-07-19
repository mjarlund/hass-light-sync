use crate::settings::{load_settings};
use crate::capture::{capture_frame, calculate_average_color, smooth_colors};
use crate::api::{send_rgb};
use captrs::Capturer;
use std::sync::Arc;
use std::time::Duration;
use log::{info, error};
use env_logger::Env;
use colored::*;
use std::collections::{HashMap, HashSet};
use rayon::prelude::*;

mod settings;
mod capture;
mod api;
mod models;

fn rgb_to_ansi_color(r: u32, g: u32, b: u32) -> ColoredString {
    let color_string = "⬤".to_string();
    color_string.truecolor(r as u8, g as u8, b as u8)
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let settings = load_settings("settings.json");
    let client = Arc::new(reqwest::Client::new());
    let mut capturer = Capturer::new(settings.monitor_id as usize).expect("Failed to get Capture Object");
    let mut prev_avg_colors: HashMap<String, (u32, u32, u32)> = HashMap::new();

    loop {
        if let Some(frame) = capture_frame(&mut capturer) {
            // Convert HashSet to Vec for parallel processing
            let unique_positions: Vec<_> = settings.lights.iter()
                .map(|light| light.position.clone())
                .collect::<HashSet<_>>()  // Collect positions into HashSet to deduplicate
                .into_iter()
                .collect();  // Convert to Vec

            // Parallel processing of positions to calculate colors
            let position_colors: HashMap<String, [u32; 3]> = unique_positions.par_iter()
                .map(|pos| (pos.clone(), calculate_average_color(&frame, pos, settings.skip_pixels)))
                .collect();

            let futures: Vec<_> = settings.lights.iter().map(|light| {
                let new_color = position_colors.get(&light.position).unwrap();
                let smoothed_color = smooth_colors(
                    *prev_avg_colors.entry(light.position.clone()).or_insert((0, 0, 0)),
                    *new_color,
                    settings.smoothing_factor
                );
                prev_avg_colors.insert(light.position.clone(), smoothed_color);
                let brightness = smoothed_color.0.max(smoothed_color.1).max(smoothed_color.2);

                send_rgb(
                    Arc::clone(&client),
                    settings.api_endpoint.clone(),
                    settings.token.clone(),
                    vec![smoothed_color.0, smoothed_color.1, smoothed_color.2],
                    brightness,
                    light.entity_name.clone()
                )
            }).collect();

            let _results = futures::future::join_all(futures).await;

            for light in &settings.lights {
                let smoothed_color = prev_avg_colors.get(&light.position).unwrap();
                let color_circle = rgb_to_ansi_color(smoothed_color.0, smoothed_color.1, smoothed_color.2);
                info!("Sent RGB data for {} at position {}: {}", light.entity_name, light.position, color_circle);
            }
        } else {
            error!("Failed to capture frame.");
        }

        tokio::time::sleep(Duration::from_millis(settings.grab_interval as u64)).await;
    }
}

