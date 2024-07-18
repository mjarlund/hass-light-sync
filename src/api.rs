use crate::models::HASSApiBody;
use std::sync::Arc;
use reqwest::{Client, Error};

/// Asynchronously sends RGB data to a specified API endpoint.
pub async fn send_rgb(
    client: Arc<Client>,
    api_endpoint: String,
    token: String,
    rgb_vec: Vec<u32>,
    brightness: u32,
    entity_name: String,
) -> Result<(), Error> {
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
        .map(|_| ())
}

