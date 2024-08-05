use crate::models::HASSApiBody;
use serde::{Serialize, Deserialize};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{connect_async, WebSocketStream};
use futures_util::stream::{SplitSink};
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::error::Error;
use log::{info, error};

#[derive(Serialize, Deserialize)]
struct LightCommand {
    id: u64,
    r#type: String,
    domain: String,
    service: String,
    service_data: HASSApiBody,
}

#[derive(Serialize, Deserialize)]
struct AuthMessage {
    r#type: String,
    access_token: String,
}

#[derive(Serialize, Deserialize)]
struct ErrorMessage {
    code: String,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct ResultMessage {
    id: u64,
    r#type: String,
    success: bool,
    error: Option<ErrorMessage>,
}

pub struct WebSocketClient {
    write: SplitSink<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
    id_counter: Arc<AtomicU64>,
}

impl WebSocketClient {
    pub async fn new(api_endpoint: String, token: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let (ws_stream, _) = connect_async(api_endpoint).await?;
        let (mut write, mut read) = ws_stream.split();

        // Authenticate with the WebSocket server
        let auth_message = AuthMessage {
            r#type: "auth".to_string(),
            access_token: token,
        };

        write.send(Message::Text(serde_json::to_string(&auth_message)?)).await?;

        // Wait for auth_ok message
        while let Some(message) = read.next().await {
            let message = message?;
            if let Message::Text(text) = message {
                let json_msg: serde_json::Value = serde_json::from_str(&text)?;
                if json_msg["type"] == "auth_ok" {
                    info!("✅ Successfully authenticated with Home Assistant");
                    break;
                }
            }
        }

        Ok(WebSocketClient {
            write,
            id_counter: Arc::new(AtomicU64::new(1)),
        })
    }

    pub async fn send_rgb(
        &mut self,
        rgb_vec: Vec<u32>,
        brightness: u32,
        entity_name: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let id = self.id_counter.fetch_add(1, Ordering::SeqCst);

        // Create the light command message
        let light_command = LightCommand {
            id,
            r#type: "call_service".to_string(),
            domain: "light".to_string(),
            service: "turn_on".to_string(),
            service_data: HASSApiBody {
                entity_id: entity_name.clone(),
                rgb_color: [rgb_vec[0], rgb_vec[1], rgb_vec[2]],
                brightness,
            },
        };

        // Serialize the light command message
        let command_json = match serde_json::to_string(&light_command) {
            Ok(json) => json,
            Err(e) => {
                error!("❌ Failed to serialize light command: {:?}", e);
                return Err(Box::new(e));
            }
        };

        if let Err(e) = self.write.send(Message::Text(command_json)).await {
            error!("❌ Failed to send light command: {:?}", e);
            return Err(Box::new(e));
        }

        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.write.close().await.map_err(|e| {
            error!("❌ Failed to close WebSocket connection: {:?}", e);
            Box::new(e) as Box<dyn Error + Send + Sync>
        })
    }
}
