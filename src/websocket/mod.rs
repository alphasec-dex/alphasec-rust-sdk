//! WebSocket client for AlphaSec real-time data

#[cfg(feature = "websocket")]
pub mod manager;

#[cfg(feature = "websocket")]
pub mod trade;

#[cfg(feature = "websocket")]
pub use manager::{ConnectionState, WsConfig, WsManager};

#[cfg(feature = "websocket")]
pub use trade::{TradeWebSocket, TradeWsError, TradeWsResponse};
