//! # AlphaSec Rust SDK
//! 
//! A comprehensive Rust SDK for interacting with the AlphaSec orderbook DEX.
//! 
//! ## Features
//! 
//! - **Complete Trading API**: All order operations with EIP-712 signing
//! - **Agent Architecture**: Unified interface combining API, WebSocket, and Signer
//! - **Session Management**: L1 wallet authentication with L2 session creation
//! - **WebSocket Streaming**: Real-time market data and user events
//! - **Type Safety**: Comprehensive type definitions with serde support
//! - **Error Handling**: Robust error types for all operations
//! 
//! ## Quick Start
//! 
//! ```rust,no_run
//! use alphasec_rust_sdk::{Agent, Config, OrderSide, OrderType, OrderMode};
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::new(
//!         "https://api-testnet.alphasec.trade",                // API URL
//!         "kairos",                                     // network
//!         "0x1234567890123456789012345678901234567890", // L1 address
//!         Some("your_l1_private_key"),                        // L1 private key
//!         None,
//!         false                                          // session enabled
//!     )?;
//!     
//!     let mut agent = Agent::new(config).await?;
//!     
//!     // Get market data
//!     let tickers = agent.get_tickers().await?;
//!     println!("Markets: {}", tickers.len());
//!     
//!     // Place an order
//!     let success = agent.order(
//!         "KAIA/USDT",           // market
//!         OrderSide::Buy,        // side
//!         1000000.0,             // price
//!         5000000.0,             // quantity
//!         OrderType::Limit,      // order_type
//!         OrderMode::Base,       // order_mode
//!         None,           // tp_limit
//!         None,           // sl_trigger
//!         None            // sl_limit
//!     ).await?;
//!     
//!     println!("Order placed: {}", success);
//!     
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod agent;
pub mod api;
pub mod error;
pub mod signer;
pub mod types;

#[cfg(feature = "websocket")]
pub mod websocket;

// Re-exports for convenience
pub use agent::Agent;
pub use error::{AlphaSecError, Result};
pub use signer::{AlphaSecSigner, Config};
pub use types::*;
