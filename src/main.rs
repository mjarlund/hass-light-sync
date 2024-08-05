use crate::settings::{load_settings, Settings};
use crate::capture::{calculate_average_color, smooth_colors};
use crate::api::WebSocketClient;
use scap::{
    capturer::{Capturer, Options},
    frame::Frame,
};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
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

    if settings.monitor_id > targets.len() as i16 {
        error!("❌ Invalid monitor_id. Must be less than the number of monitors.");
        return;
    }

    let selected_targets = targets.into_iter()
        .filter(|t| t.id as i16 == settings.monitor_id)
        .collect::<Vec<_>>();

    info!("🎯 Selected target: {:?}", selected_targets);

    let options = Options {
        fps: 30,
        targets: selected_targets,
        show_cursor: false,
        show_highlight: false,
        excluded_targets: None,
        output_type: scap::frame::FrameType::BGRAFrame,
        output_resolution: scap::capturer::Resolution::_720p,
        ..Default::default()
    };

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    tokio::spawn(handle_signals(stop_clone));

    let websocket_url = settings.api_endpoint.clone();
    let token = settings.token.clone();

    let ws_client = Arc::new(Mutex::new(WebSocketClient::new(websocket_url, token).await.expect("Failed to create WebSocket client")));

    let prev_avg_colors: HashMap<String, (u32, u32, u32)> = HashMap::new();

    let mut capturer = Capturer::new(options);
    capturer.start_capture();

    let mut last_process_time = Instant::now();

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }

        match capturer.get_next_frame() {
            Ok(Frame::BGRA(frame)) => {
                if last_process_time.elapsed() >= Duration::from_millis(settings.grab_interval as u64) {
                    let frame_clone = frame.clone();
                    let settings_clone = settings.clone();
                    let ws_client_clone = Arc::clone(&ws_client);
                    let mut prev_avg_colors_clone = prev_avg_colors.clone();

                    tokio::spawn(async move {
                        process_frame(&frame_clone, &settings_clone, &ws_client_clone, &mut prev_avg_colors_clone).await;
                    });

                    last_process_time = Instant::now();
                }
            }
            _ => {
                warn!("⚠️ Failed to get next RGB frame. Retrying...");
            }
        }
    }

    capturer.stop_capture();
    ws_client.lock().await.close().await.expect("Failed to close WebSocket connection");
}

async fn process_frame(
    frame: &scap::frame::BGRAFrame,
    settings: &Settings,
    ws_client: &Arc<Mutex<WebSocketClient>>,
    prev_avg_colors: &mut HashMap<String, (u32, u32, u32)>,
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
    let mut tasks = Vec::new();
    for light in &settings.lights {
        if let Some(smoothed_color) = position_colors.get(&light.position) {
            let brightness = smoothed_color.0.max(smoothed_color.1).max(smoothed_color.2);
            let entity_name = light.entity_name.clone();
            let rgb_vec = vec![smoothed_color.0, smoothed_color.1, smoothed_color.2];
            let ws_client_clone = Arc::clone(&ws_client);
            tasks.push(tokio::spawn(async move {
                if let Err(e) = ws_client_clone.lock().await.send_rgb(rgb_vec, brightness, entity_name).await {
                    error!("❌ Failed to send RGB data: {:?}", e);
                }
            }));
        }
    }

    // Await all tasks
    for task in tasks {
        let _ = task.await;
    }
}
