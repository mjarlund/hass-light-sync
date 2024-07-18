use crate::settings::{load_settings};
use crate::capture::{capture_frame, calculate_average_color};
use crate::api::{send_rgb};
use captrs::Capturer;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::spawn;
use rayon::prelude::*;

mod settings;
mod capture;
mod api;
mod models;

#[tokio::main]
async fn main() {
    let settings = load_settings("settings.json");
    let client = Arc::new(reqwest::Client::new());
    let mut capturer = Capturer::new(settings.monitor_id as usize).expect("Failed to get Capture Object");
    let mut prev_avg_colors = vec![(0u32, 0u32, 0u32); settings.lights.len()];

    loop {
        if let Some(frame) = capture_frame(&mut capturer) {
            let avg_colors: Vec<_> = settings.lights.par_iter().map(|light| {
                calculate_average_color(&frame, &light.position, settings.skip_pixels)
            }).collect();

            for (i, avg_color) in avg_colors.iter().enumerate() {
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
                let entity_name = settings.lights[i].entity_name.clone();

                // Asynchronous call to send RGB data
                spawn(send_rgb(Arc::clone(&client), api_endpoint, token, smoothed_avg_color, brightness, entity_name));
            }
        }

        thread::sleep(Duration::from_millis(settings.grab_interval as u64));
    }
}
