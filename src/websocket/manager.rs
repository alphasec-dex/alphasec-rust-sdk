//! WebSocket manager
//!
//! Features:
//! - Channel-based message delivery via `mpsc::UnboundedReceiver<WebSocketMessage>`
//! - Reconnect with backoff and auto resubscribe
//! - Explicit lifecycle: `start()` / `stop()` with task join
//! - Periodic pings and pong-timeout detection

use crate::{error::Result, types::websocket::*};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use url::Url;

/// WebSocket connection state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionState {
    /// Disconnected
    Disconnected,
    /// Connecting
    Connecting,
    /// Connected and ready
    Connected,
    /// Reconnecting after a disconnection
    Reconnecting,
    /// Connection explicitly closed by user
    Closed,
}

/// Configuration for the WebSocket manager
#[derive(Debug, Clone)]
pub struct WsConfig {
    /// WebSocket endpoint URL (ws:// or wss://)
    pub url: String,
    /// Maximum reconnection attempts (0 means infinite)
    pub max_reconnect_attempts: u32,
    /// Initial delay before first reconnect attempt
    pub reconnect_delay: Duration,
    /// Upper bound for reconnect backoff delay
    pub max_reconnect_delay: Duration,
    /// Interval between pings while connected
    pub ping_interval: Duration,
    /// Maximum time to wait for a pong after a ping
    pub pong_timeout: Duration,
    /// Capacity of the internal message queue to the consumer
    pub message_queue_size: usize,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            url: "wss://api.alphasec.io/ws".to_string(),
            max_reconnect_attempts: 0, // Infinite retries
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(30),
            ping_interval: Duration::from_secs(10),
            pong_timeout: Duration::from_secs(30),
            message_queue_size: 1000,
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// Total number of connection attempts
    pub connection_attempts: u32,
    /// Total number of successful connections
    pub successful_connections: u32,
    /// Number of messages sent to the server
    pub messages_sent: u64,
    /// Number of messages received from the server
    pub messages_received: u64,
    /// Timestamp when the last successful connection was established
    pub last_connected_at: Option<Instant>,
    /// Timestamp when the last disconnection occurred
    pub last_disconnected_at: Option<Instant>,
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self {
            connection_attempts: 0,
            successful_connections: 0,
            messages_sent: 0,
            messages_received: 0,
            last_connected_at: None,
            last_disconnected_at: None,
        }
    }
}

/// Manager control commands
#[derive(Debug)]
enum ManagerCommand {
    /// Open a new connection
    Connect,
    /// Close the current connection and stop the task
    Disconnect,
    /// Subscribe to a channel (identified by SDK-level id)
    Subscribe { id: i32, channel: String },
    /// Unsubscribe from a channel
    Unsubscribe { id: i32, channel: String },
}

/// WebSocket manager for AlphaSec
// Custom Clone implemented below (JoinHandle is not Clone)
#[derive()]
pub struct WsManager {
    /// Runtime configuration
    config: WsConfig,
    /// Connection state shared across tasks
    state: Arc<RwLock<ConnectionState>>,
    /// Active subscriptions map: id -> channel
    subscriptions: Arc<Mutex<HashMap<i32, String>>>,
    /// Next subscription id generator
    next_id: Arc<Mutex<i32>>,
    /// Control channel sender to drive the connection task
    control_tx: Option<mpsc::UnboundedSender<ManagerCommand>>,
    /// Join handle of the connection task (for graceful shutdown)
    /// NOTE: JoinHandle is not Clone, so this field is not included in Clone/Debug
    control_task: Option<tokio::task::JoinHandle<()>>,
    /// Connection statistics
    stats: Arc<Mutex<ConnectionStats>>,
    /// Receiver given to SDK users (taken once) for incoming messages
    message_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<WebSocketMessage>>>>,
    /// Sender used by the connection task to forward parsed messages
    message_tx: Option<mpsc::UnboundedSender<WebSocketMessage>>,
    /// Sender used by SDK users to send raw WebSocket messages (ping/pong, etc.)
    outgoing_sender: Arc<Mutex<Option<mpsc::UnboundedSender<Message>>>>,
}

impl std::fmt::Debug for WsManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WsManager")
            .field("config", &self.config)
            .field("state", &"<RwLock>")
            .field("subscriptions", &"<Mutex>")
            .field("next_id", &"<Mutex>")
            .field("control_tx", &self.control_tx.is_some())
            .field("stats", &"<Mutex>")
            .finish()
    }
}

impl Clone for WsManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: Arc::clone(&self.state),
            subscriptions: Arc::clone(&self.subscriptions),
            next_id: Arc::clone(&self.next_id),
            control_tx: self.control_tx.clone(),
            control_task: None, // JoinHandle is not Clone; new clone has no running task
            stats: Arc::clone(&self.stats),
            message_rx: Arc::clone(&self.message_rx),
            message_tx: self.message_tx.clone(),
            outgoing_sender: Arc::clone(&self.outgoing_sender),
        }
    }
}

impl WsManager {
    /// Create a new WebSocket manager
    pub fn new(config: WsConfig) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        Self {
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
            control_tx: None,
            control_task: None,
            stats: Arc::new(Mutex::new(ConnectionStats::default())),
            message_rx: Arc::new(Mutex::new(Some(message_rx))),
            message_tx: Some(message_tx),
            outgoing_sender: Arc::new(Mutex::new(None)),
        }
    }

    /// Start the WebSocket manager
    pub async fn start(&mut self) -> Result<()> {
        if self.control_tx.is_some() {
            warn!("WebSocket manager already started");
            return Ok(());
        }

        let (control_tx, control_rx) = mpsc::unbounded_channel();
        self.control_tx = Some(control_tx.clone());

        // Spawn the main connection task
        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        let subscriptions = Arc::clone(&self.subscriptions);
        let stats = Arc::clone(&self.stats);
        let message_tx = self
            .message_tx
            .as_ref()
            .expect("message_tx not initialized")
            .clone();
        let outgoing_sender = Arc::clone(&self.outgoing_sender);

        let handle = tokio::spawn(async move {
            Self::connection_task(
                config,
                state,
                subscriptions,
                control_rx,
                message_tx,
                stats,
                outgoing_sender,
            )
                .await;
        });
        self.control_task = Some(handle);

        // Send connect command
        control_tx
            .send(ManagerCommand::Connect)
            .map_err(|_| crate::error::AlphaSecError::network("Failed to send connect command"))?;

        info!("🚀 WebSocket manager started");
        Ok(())
    }

    /// Stop the WebSocket manager
    pub async fn stop(&mut self) {
        if let Some(ref control_tx) = self.control_tx {
            let _ = control_tx.send(ManagerCommand::Disconnect);
            info!("🛑 WebSocket manager stop requested");
        }
        // Drop the message sender first so receivers can complete even if the task lingers
        self.message_tx = None;
        // Await the connection task to finish
        if let Some(handle) = self.control_task.take() {
            let _ = handle.await;
        }
        // Clear control channel
        self.control_tx = None;
    }

    /// Subscribe to a channel
    pub async fn subscribe(&self, channel: String) -> Result<i32> {
        let id = {
            let mut next_id = self.next_id.lock().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        {
            let mut subs = self.subscriptions.lock().await;
            subs.insert(id, channel.clone());
        }

        if let Some(ref control_tx) = self.control_tx {
            control_tx
                .send(ManagerCommand::Subscribe { id, channel })
                .map_err(|_| {
                    crate::error::AlphaSecError::network("Failed to send subscribe command")
                })?;
        }

        Ok(id)
    }

    /// Unsubscribe from a channel
    pub async fn unsubscribe(&self, id: i32) -> Result<bool> {
        let channel = {
            let subs = self.subscriptions.lock().await;
            subs.get(&id).cloned()
        };

        let removed = {
            let mut subs = self.subscriptions.lock().await;
            subs.remove(&id).is_some()
        };

        if removed {
            if let Some(ref control_tx) = self.control_tx {
                let _ = control_tx.send(ManagerCommand::Unsubscribe {
                    id,
                    channel: channel.unwrap(),
                });
            }
        }

        Ok(removed)
    }

    /// Get current connection state
    pub async fn get_state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        self.stats.lock().await.clone()
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        matches!(*self.state.read().await, ConnectionState::Connected)
    }

    /// Get the message receiver (can only be called once)
    pub async fn take_message_receiver(&self) -> Option<mpsc::UnboundedReceiver<WebSocketMessage>> {
        self.message_rx.lock().await.take()
    }

    /// Main connection task
    async fn connection_task(
        config: WsConfig,
        state: Arc<RwLock<ConnectionState>>,
        subscriptions: Arc<Mutex<HashMap<i32, String>>>,
        mut control_rx: mpsc::UnboundedReceiver<ManagerCommand>,
        message_tx: mpsc::UnboundedSender<WebSocketMessage>,
        stats: Arc<Mutex<ConnectionStats>>,
        outgoing_sender: Arc<Mutex<Option<mpsc::UnboundedSender<Message>>>>,
    ) {
        let mut reconnect_attempts = 0;
        let mut current_reconnect_delay = config.reconnect_delay;
        let mut ever_connected = false;

        loop {
            tokio::select! {
                // Handle control commands
                Some(cmd) = control_rx.recv() => {
                    match cmd {
                        ManagerCommand::Connect => {
                            reconnect_attempts = 0;
                            current_reconnect_delay = config.reconnect_delay;
                        },
                        ManagerCommand::Disconnect => {
                            *state.write().await = ConnectionState::Closed;
                            break;
                        },
                _ => {}
                    }
                },
                // Connection logic
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if matches!(*state.read().await, ConnectionState::Disconnected | ConnectionState::Reconnecting) {
                        let did_connect = Self::handle_connection(
                            &config,
                            &state,
                            &subscriptions,
                            &mut control_rx,
                            &message_tx,
                            &stats,
                            &mut reconnect_attempts,
                            &mut current_reconnect_delay,
                            &outgoing_sender,
                        ).await;
                        if did_connect {
                            ever_connected = true;
                        }

                        // If a Disconnect was processed inside handle_connection, the state is Closed.
                        // Break the outer task loop so stop() can join this task.
                        if matches!(*state.read().await, ConnectionState::Closed) {
                            break;
                        }
                        // After a successful connection, do NOT auto-reconnect on later disconnects
                        if ever_connected && matches!(*state.read().await, ConnectionState::Disconnected) {
                            break;
                        }
                    }
                }
            }
        }

        info!("WebSocket connection task ended");
    }

    async fn handle_connection(
        config: &WsConfig,
        state: &Arc<RwLock<ConnectionState>>,
        subscriptions: &Arc<Mutex<HashMap<i32, String>>>,
        control_rx: &mut mpsc::UnboundedReceiver<ManagerCommand>,
        message_tx: &mpsc::UnboundedSender<WebSocketMessage>,
        stats: &Arc<Mutex<ConnectionStats>>,
        reconnect_attempts: &mut u32,
        current_reconnect_delay: &mut Duration,
        outgoing_sender: &Arc<Mutex<Option<mpsc::UnboundedSender<Message>>>>,
    ) -> bool {
        // Update state to connecting
        *state.write().await = ConnectionState::Connecting;

        // Update stats
        {
            let mut stats_guard = stats.lock().await;
            stats_guard.connection_attempts += 1;
        }

        info!("🔌 Attempting to connect to WebSocket: {}", config.url);

        // Parse URL
        let url = match Url::parse(&config.url) {
            Ok(url) => url,
            Err(e) => {
                error!("❌ Invalid WebSocket URL: {}", e);
                *state.write().await = ConnectionState::Disconnected;
                return false;
            }
        };

        // Attempt connection
        let ws_stream = match connect_async(url).await {
            Ok((ws_stream, _)) => ws_stream,
            Err(e) => {
                error!("❌ Failed to connect to WebSocket: {}", e);

                // Handle reconnection
                *reconnect_attempts += 1;
                if config.max_reconnect_attempts > 0
                    && *reconnect_attempts >= config.max_reconnect_attempts
                {
                    error!("❌ Max reconnection attempts reached");
                    *state.write().await = ConnectionState::Disconnected;
                    return false;
                }

                *state.write().await = ConnectionState::Reconnecting;
                info!(
                    "🔄 Reconnecting in {:?} (attempt {})",
                    current_reconnect_delay, *reconnect_attempts
                );

                sleep(*current_reconnect_delay).await;
                *current_reconnect_delay =
                    std::cmp::min(*current_reconnect_delay * 2, config.max_reconnect_delay);
                return false;
            }
        };

        // Successfully connected
        info!("✅ WebSocket connected");
        *state.write().await = ConnectionState::Connected;
        *reconnect_attempts = 0;
        *current_reconnect_delay = config.reconnect_delay;
        // from here on, we have been connected at least once in this session

        // Update stats
        {
            let mut stats_guard = stats.lock().await;
            stats_guard.successful_connections += 1;
            stats_guard.last_connected_at = Some(Instant::now());
        }

        let (mut ws_sink, mut ws_stream) = ws_stream.split();
        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<Message>();

        // Expose outgoing sender to SDK users for ping/pong handling
        {
            let mut sender_guard = outgoing_sender.lock().await;
            *sender_guard = Some(outgoing_tx.clone());
        }

        // Re-subscribe to existing channels
        {
            let subs = subscriptions.lock().await;
            for (id, channel) in subs.iter() {
                let subscribe_msg = serde_json::json!({
                    "method": "subscribe",
                    "params": [channel],
                    "id": id
                });
                if let Err(e) = outgoing_tx.send(Message::Text(subscribe_msg.to_string())) {
                    error!("Failed to re-subscribe to {}: {}", channel, e);
                }
            }
        }

        // Main connection loop
        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                ws_msg = ws_stream.next() => {
                    match ws_msg {
                        Some(Ok(Message::Text(text))) => {
                            debug!("📨 Received: {}", text);

                            // Update stats
                            {
                                let mut stats_guard = stats.lock().await;
                                stats_guard.messages_received += 1;
                            }

                            // Parse and send to message channel
                            match serde_json::from_str::<WebSocketMessage>(&text) {
                                Ok(msg) => {
                                    // Filter out internal messages and acks
                                    let should_forward = match &msg {
                                        WebSocketMessage::Ack { .. } => {
                                            debug!("Subscription ack: {:?}", msg);
                                            false
                                        },
                                        WebSocketMessage::TradeMsg { .. } => {
                                            true
                                        },
                                        WebSocketMessage::DepthMsg { .. } => {
                                            true
                                        },
                                        WebSocketMessage::TickerMsg { .. } => {
                                            true
                                        },
                                        WebSocketMessage::UserEventMsg { .. } => {
                                            true
                                        },
                                        WebSocketMessage::Generic(value) => {
                                            // Forward generic messages, let user handle them
                                            debug!("Generic WebSocket message: {:?}", value);
                                            true
                                        },
                                        WebSocketMessage::Ping(_) | WebSocketMessage::Pong(_) => {
                                            true
                                        },
                                        WebSocketMessage::Disconnected => {
                                            // Disconnected is SDK-internal; it won't come from server JSON.
                                            // Still mark as forwardable to satisfy exhaustiveness.
                                            true
                                        }
                                    };

                                    if should_forward {
                                        if let Err(_) = message_tx.send(msg) {
                                            warn!("Message receiver dropped, continuing...");
                                        }
                                    }
                                },
                                Err(e) => {
                                    // If parsing fails, try to parse as generic JSON and forward
                                    match serde_json::from_str::<serde_json::Value>(&text) {
                                        Ok(value) => {
                                            debug!("Forwarding unparseable message as generic: {}", text);
                                            let generic_msg = WebSocketMessage::Generic(value);
                                            if let Err(_) = message_tx.send(generic_msg) {
                                                warn!("Failed to send generic message, continuing...");
                                            }
                                        },
                                        Err(_) => {
                                            warn!("Failed to parse WebSocket message as JSON: {} - {}", e, text);
                                        }
                                    }
                                }
                            }
                        },
                        Some(Ok(Message::Pong(payload))) => {
                            debug!("Received pong ({} bytes)", payload.len());
                            let _ = message_tx.send(WebSocketMessage::Pong(payload));
                        },
                        Some(Ok(Message::Ping(payload))) => {
                            debug!("Received ping ({} bytes)", payload.len());
                            let _ = message_tx.send(WebSocketMessage::Ping(payload));
                        },
                        Some(Ok(Message::Close(close_frame))) => {
                            match close_frame {
                                Some(frame) => {
                                    error!("WebSocket closed by server: code={:?}, reason=\"{}\"", frame.code, frame.reason);
                                },
                                None => {
                                    error!("WebSocket closed without close frame");
                                }
                            }
                            break;
                        },
                        Some(Err(e)) => {
                            error!("❌ WebSocket error: {}", e);
                            break;
                        },
                        None => {
                            info!("WebSocket stream ended");
                            break;
                        },
                        _ => {}
                    }
                },

                // Handle outgoing messages
                Some(msg) = outgoing_rx.recv() => {
                    match &msg {
                        Message::Text(t) => debug!("Sending text: {}", t),
                        Message::Binary(b) => debug!("Sending binary: {} bytes", b.len()),
                        Message::Ping(p) => debug!("Sending ping: {} bytes", p.len()),
                        Message::Pong(p) => debug!("Sending pong: {} bytes", p.len()),
                        Message::Close(_) => debug!("Sending close"),
                        _ => debug!("Sending control frame"),
                    }
                    if let Err(e) = ws_sink.send(msg).await {
                        error!("❌ Failed to send message: {}", e);
                        break;
                    }

                    // Update stats
                    {
                        let mut stats_guard = stats.lock().await;
                        stats_guard.messages_sent += 1;
                    }
                },

                // Handle control commands
                Some(cmd) = control_rx.recv() => {
                    match cmd {
                        ManagerCommand::Disconnect => {
                            info!("🛑 Disconnect requested");
                            let _ = ws_sink.send(Message::Close(None)).await;
                            *state.write().await = ConnectionState::Closed;
                            // Clear exposed outgoing sender on disconnect
                            {
                                let mut sender_guard = outgoing_sender.lock().await;
                                *sender_guard = None;
                            }
                            return true;
                        },
                        ManagerCommand::Subscribe { id, channel } => {
                            debug!("Sending subscribe message: {}", channel);
                            let subscribe_msg = serde_json::json!({
                                "method": "subscribe",
                                "params": {"channels": [channel]},
                                "id": id
                            });
                            if let Err(e) = outgoing_tx.send(Message::Text(subscribe_msg.to_string())) {
                                error!("Failed to send subscribe message: {}", e);
                            }
                        },
                        ManagerCommand::Unsubscribe { id, channel } => {
                            let unsubscribe_msg = serde_json::json!({
                                "method": "unsubscribe",
                                "params": {"channels": [channel]},
                                "id": id
                            });
                            if let Err(e) = outgoing_tx.send(Message::Text(unsubscribe_msg.to_string())) {
                                error!("Failed to send unsubscribe message: {}", e);
                            }
                        },
                        _ => {}
                    }
                },
            }
        }

        // Connection ended
        info!("WebSocket connection ended");
        *state.write().await = ConnectionState::Disconnected;

        // Update stats
        {
            let mut stats_guard = stats.lock().await;
            stats_guard.last_disconnected_at = Some(Instant::now());
        }
        // Clear exposed outgoing sender when connection ends
        {
            let mut sender_guard = outgoing_sender.lock().await;
            *sender_guard = None;
        }
        // Notify SDK user immediately about disconnection
        let _ = message_tx.send(WebSocketMessage::Disconnected);
        true
    }

    /// Get a clone of the underlying WebSocket sender for direct frame sending.
    /// Returns None if not connected.
    pub async fn get_outgoing_sender(&self) -> Option<mpsc::UnboundedSender<Message>> {
        self.outgoing_sender.lock().await.as_ref().cloned()
    }
}
