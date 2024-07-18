use crate::settings::{load_settings};
use crate::capture::{capture_frame, calculate_average_color, smooth_colors};
use crate::api::{send_rgb};
use captrs::Capturer;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::spawn;
use rayon::prelude::*;
use log::{info, debug, error};
use env_logger::Env;
use colored::*;

mod settings;
mod capture;
mod api;
mod models;

fn rgb_to_ansi_color(r: u32, g: u32, b: u32) -> ColoredString {
    let color_string = format!("⬤"); // Circle emoji
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
    let mut prev_avg_colors = vec![(0u32, 0u32, 0u32); settings.lights.len()];

    loop {
        if let Some(frame) = capture_frame(&mut capturer) {
            debug!("Frame captured successfully.");
            let avg_colors: Vec<_> = settings.lights.par_iter().map(|light| {
                calculate_average_color(&frame, &light.position, settings.skip_pixels)
            }).collect();

            for (i, new_color) in avg_colors.iter().enumerate() {
                let smoothed_color = smooth_colors(prev_avg_colors[i], *new_color, settings.smoothing_factor);
                debug!("Smoothed color: {:?} for light {}", smoothed_color, settings.lights[i].entity_name);
                prev_avg_colors[i] = smoothed_color;
                let brightness = smoothed_color.0.max(smoothed_color.1).max(smoothed_color.2);

                let api_endpoint = settings.api_endpoint.clone();
                let token = settings.token.clone();
                let entity_name = settings.lights[i].entity_name.clone();
                spawn(send_rgb(Arc::clone(&client), api_endpoint.clone(), token.clone(), vec![smoothed_color.0, smoothed_color.1, smoothed_color.2], brightness, entity_name.clone()));
                let color_circle = rgb_to_ansi_color(smoothed_color.0, smoothed_color.1, smoothed_color.2);
                info!("Sending RGB data for {}: {}", entity_name, color_circle);

            }
        } else {
            error!("Failed to capture frame.");
        }

        thread::sleep(Duration::from_millis(settings.grab_interval as u64));
        debug!("Sleeping for {} milliseconds", settings.grab_interval);
    }
}
