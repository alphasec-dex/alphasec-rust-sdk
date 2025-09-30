//! WebSocket client for AlphaSec real-time data

#[cfg(feature = "websocket")]
pub mod manager;

#[cfg(feature = "websocket")]
pub use manager::{WsManager, WsConfig, ConnectionState};
