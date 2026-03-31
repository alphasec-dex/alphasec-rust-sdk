//! WebSocket Trade API client for AlphaSec
//!
//! Provides low-latency order submission via WebSocket (`/ws-api`) instead of REST.
//! Pattern follows Binance/Bybit TradeWebSocket: request-response correlation
//! with UUID IDs and oneshot channels.

use crate::error::{AlphaSecError, Result};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// Response from the Trade WebSocket API
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradeWsResponse {
    /// Request ID (echoed back)
    pub id: String,
    /// Result on success (typically txHash)
    #[serde(default)]
    pub result: Option<String>,
    /// Error on failure
    #[serde(default)]
    pub error: Option<TradeWsError>,
}

/// Error from the Trade WebSocket API
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradeWsError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
}

/// Pending request waiting for response
struct PendingRequest {
    tx: oneshot::Sender<TradeWsResponse>,
}

/// Internal state for TradeWebSocket
struct TradeWsState {
    write_tx: Option<mpsc::Sender<Message>>,
    pending_requests: HashMap<String, PendingRequest>,
    connected: bool,
}

/// WebSocket Trade API connection manager
///
/// Provides a single-attempt `connect()` to the `/ws-api` endpoint
/// for low-latency order operations. Request-response correlation via UUID.
/// The caller (connector) is responsible for reconnection policy.
pub struct TradeWebSocket {
    ws_api_url: String,
    state: Arc<Mutex<TradeWsState>>,
}

impl Clone for TradeWebSocket {
    fn clone(&self) -> Self {
        Self {
            ws_api_url: self.ws_api_url.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

impl std::fmt::Debug for TradeWebSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TradeWebSocket")
            .field("ws_api_url", &self.ws_api_url)
            .field("state", &"<Mutex>")
            .finish()
    }
}

impl TradeWebSocket {
    /// Create a new TradeWebSocket
    ///
    /// # Arguments
    /// * `ws_api_url` - WebSocket API URL (e.g., `wss://api-qa.dexor.trade/ws-api`)
    pub fn new(ws_api_url: &str) -> Self {
        Self {
            ws_api_url: ws_api_url.to_string(),
            state: Arc::new(Mutex::new(TradeWsState {
                write_tx: None,
                pending_requests: HashMap::new(),
                connected: false,
            })),
        }
    }

    /// Connect to the Trade WebSocket (single attempt).
    ///
    /// Establishes connection and spawns background read/write/ping tasks.
    /// When the connection drops, `is_connected()` becomes `false` and all
    /// pending requests are failed. The caller is responsible for
    /// reconnection (see Binance/Bybit connector pattern).
    pub async fn connect(&self) -> Result<()> {
        info!("📡 Connecting to AlphaSec Trade WebSocket: {}", self.ws_api_url);

        let (ws_stream, _) = connect_async(&self.ws_api_url)
            .await
            .map_err(|e| AlphaSecError::network(format!("Failed to connect to Trade WS: {}", e)))?;

        info!("✅ Connected to AlphaSec Trade WebSocket");

        let (mut write, mut read) = ws_stream.split();
        let (write_tx, mut write_rx) = mpsc::channel::<Message>(100);

        {
            let mut state_guard = self.state.lock().await;
            state_guard.write_tx = Some(write_tx);
            state_guard.connected = true;
        }

        // Write task
        let write_task = tokio::spawn(async move {
            while let Some(msg) = write_rx.recv().await {
                if let Err(e) = write.send(msg).await {
                    error!("❌ Failed to send Trade WS message: {}", e);
                    break;
                }
            }
        });

        // Ping task: every 20s (server 30s timeout)
        let state_ping = self.state.clone();
        let ping_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(20));
            loop {
                interval.tick().await;
                let state_guard = state_ping.lock().await;
                if let Some(tx) = &state_guard.write_tx {
                    if tx.send(Message::Ping(vec![])).await.is_err() {
                        break;
                    }
                    debug!("🏓 Sent ping to AlphaSec Trade WS");
                } else {
                    break;
                }
            }
        });

        // Read loop in background — sets connected=false on disconnect
        let state_read = self.state.clone();
        tokio::spawn(async move {
            while let Some(msg_result) = read.next().await {
                match msg_result {
                    Ok(Message::Text(text)) => {
                        debug!("📨 Trade WS received: {}", text);
                        match serde_json::from_str::<TradeWsResponse>(&text) {
                            Ok(response) => {
                                let mut state_guard = state_read.lock().await;
                                if let Some(pending) =
                                    state_guard.pending_requests.remove(&response.id)
                                {
                                    let _ = pending.tx.send(response);
                                } else {
                                    debug!(
                                        "Received Trade WS response for unknown request: {}",
                                        response.id
                                    );
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "⚠️ Failed to parse Trade WS response: {} - Raw: {}",
                                    e,
                                    text.chars().take(200).collect::<String>()
                                );
                            }
                        }
                    }
                    Ok(Message::Ping(_)) => debug!("Received Ping from AlphaSec Trade WS"),
                    Ok(Message::Pong(_)) => debug!("Received Pong from AlphaSec Trade WS"),
                    Ok(Message::Close(frame)) => {
                        warn!("⚠️ AlphaSec Trade WS closed: {:?}", frame);
                        break;
                    }
                    Err(e) => {
                        error!("❌ AlphaSec Trade WS error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }

            // Cleanup on disconnect
            write_task.abort();
            ping_task.abort();

            let mut state_guard = state_read.lock().await;
            state_guard.write_tx = None;
            state_guard.connected = false;
            warn!("⚠️ AlphaSec Trade WS disconnected");
            for (_, pending) in state_guard.pending_requests.drain() {
                let _ = pending.tx.send(TradeWsResponse {
                    id: String::new(),
                    result: None,
                    error: Some(TradeWsError {
                        code: -1,
                        message: "Connection lost".to_string(),
                    }),
                });
            }
        });

        Ok(())
    }

    /// Send a request and wait for response with 10 second timeout
    pub async fn send_request(&self, req_id: &str, message: String) -> Result<TradeWsResponse> {
        let (tx, rx) = oneshot::channel();

        // Register pending request and get write channel
        let write_tx = {
            let mut state_guard = self.state.lock().await;
            if !state_guard.connected {
                return Err(AlphaSecError::network("Trade WebSocket not connected"));
            }
            state_guard
                .pending_requests
                .insert(req_id.to_string(), PendingRequest { tx });
            state_guard.write_tx.clone()
        };

        // Send message
        if let Some(write_tx) = write_tx {
            write_tx
                .send(Message::Text(message.into()))
                .await
                .map_err(|_| AlphaSecError::network("Failed to send message to Trade WS"))?;
        } else {
            let mut state_guard = self.state.lock().await;
            state_guard.pending_requests.remove(req_id);
            return Err(AlphaSecError::network(
                "Trade WebSocket write channel not available",
            ));
        }

        // Wait for response with 10s timeout
        match tokio::time::timeout(tokio::time::Duration::from_secs(10), rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => {
                let mut state_guard = self.state.lock().await;
                state_guard.pending_requests.remove(req_id);
                Err(AlphaSecError::network("Trade WS request channel closed"))
            }
            Err(_) => {
                let mut state_guard = self.state.lock().await;
                state_guard.pending_requests.remove(req_id);
                Err(AlphaSecError::network("Trade WS request timed out"))
            }
        }
    }

    /// Check if the WebSocket is connected
    pub async fn is_connected(&self) -> bool {
        let state_guard = self.state.lock().await;
        state_guard.connected
    }
}
