use crate::settings::load_settings;
use crate::capture::{calculate_average_color, smooth_colors};
use crate::api::send_rgb;
use scap::{
    capturer::{Capturer, Options},
    frame::Frame,
};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use log::{info, error, warn};
use env_logger::Env;
use colored::*;
use std::collections::HashMap;
use tokio::signal;

mod settings;
mod capture;
mod api;
mod models;

fn rgb_to_ansi_color(r: u32, g: u32, b: u32) -> ColoredString {
    "⬤".to_string().truecolor(r as u8, g as u8, b as u8)
}

async fn handle_signals(stop: Arc<AtomicBool>) {
    let _ = signal::ctrl_c().await;
    stop.store(true, Ordering::SeqCst);
    info!("Received Ctrl+C, stopping capture...");
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    if !scap::is_supported() {
        error!("❌ Platform not supported");
        return;
    }
    info!("✅ Platform supported");

    if !scap::has_permission() && !scap::request_permission() {
        error!("❌ Permission denied");
        return;
    }
    info!("✅ Permission granted");

    let targets = scap::get_targets();
    info!("🎯 Available targets: {:?}", targets);

    let settings = load_settings("settings.json");

    if settings.monitor_id >= targets.len() as i16 {
        error!("❌ Invalid monitor_id. Must be less than the number of monitors.");
        return;
    }

    let selected_targets = targets.into_iter()
        .filter(|t| t.id as i16 == settings.monitor_id)
        .collect::<Vec<_>>();

    info!("🎯 Selected target: {:?}", selected_targets);

    let options = Options {
        fps: 60,
        targets: selected_targets,
        show_cursor: true,
        show_highlight: true,
        excluded_targets: None,
        output_type: scap::frame::FrameType::RGB,
        output_resolution: scap::capturer::Resolution::_720p,
        ..Default::default()
    };

    let mut capturer = Capturer::new(options);
    capturer.start_capture();

    let client = Arc::new(reqwest::Client::new());
    let mut prev_avg_colors: HashMap<String, (u32, u32, u32)> = HashMap::new();
    let stop = Arc::new(AtomicBool::new(false));

    let stop_clone = Arc::clone(&stop);
    tokio::spawn(handle_signals(stop_clone));

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }

        match capturer.get_next_frame() {
            Ok(Frame::RGB(frame)) => process_frame(&frame, &settings, &client, &mut prev_avg_colors).await,
            _ => {
                warn!("⚠️ Failed to get next RGB frame. Retrying...");
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
        }

        tokio::time::sleep(Duration::from_millis(settings.grab_interval as u64)).await;
    }

    capturer.stop_capture();
}

async fn process_frame(
    frame: &scap::frame::RGBFrame,
    settings: &settings::Settings,
    client: &Arc<reqwest::Client>,
    prev_avg_colors: &mut HashMap<String, (u32, u32, u32)>
) {
    let positions = vec!["top", "bottom", "left", "right"];
    let mut position_colors = HashMap::new();

    // Calculate and smooth colors for each position
    for &pos in &positions {
        let new_color = calculate_average_color(&frame, pos);
        let smoothed_color = smooth_colors(
            *prev_avg_colors.entry(pos.to_string()).or_insert((0, 0, 0)),
            new_color,
            settings.smoothing_factor,
        );
        prev_avg_colors.insert(pos.to_string(), smoothed_color);
        position_colors.insert(pos.to_string(), smoothed_color);
    }

    // Log the smoothed colors for each position in a single log statement
    let mut log_message = String::from("🌈 Smoothed colors: ");
    for pos in &positions {
        if let Some(smoothed_color) = position_colors.get(*pos) {
            let color_circle = rgb_to_ansi_color(smoothed_color.0, smoothed_color.1, smoothed_color.2);
            log_message.push_str(&format!("{}: {} ", pos, color_circle));
        }
    }
    info!("{}", log_message);

    // Assign smoothed colors to lights and send data if API calls are enabled
    for light in &settings.lights {
        if let Some(smoothed_color) = position_colors.get(&light.position) {
            if settings.enable_api_calls {
                let brightness = smoothed_color.0.max(smoothed_color.1).max(smoothed_color.2);
                let send_rgb_future = send_rgb(
                    Arc::clone(&client),
                    settings.api_endpoint.clone(),
                    settings.token.clone(),
                    vec![smoothed_color.0, smoothed_color.1, smoothed_color.2],
                    brightness,
                    light.entity_name.clone(),
                );
                tokio::spawn(send_rgb_future);
            }
        }
    }
}
