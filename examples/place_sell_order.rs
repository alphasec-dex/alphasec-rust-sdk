//! Place sell order example
//!
//! This example demonstrates how to place a single sell order only

use alphasec_rs::{Agent, Config, OrderMode, OrderSide, OrderType};
use rust_decimal::Decimal;
use std::str::FromStr;
use tracing::{error, info, warn};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec sell order example");

    // Create configuration with hardcoded values for testing
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0x0000000000000000000000000000000000000000", // Your L1 address
        Some("0000000000000000000000000000000000000000000000000000000000000000"), // Your private key (no 0x prefix)
        None,  // L2 key, no session
        false, // L1 key, no session
        None,  // Chain ID
    )?;

    // Create Agent
    let agent = Agent::new(config).await?;
    info!("✅ AlphaSec Agent initialized successfully");

    // Place a SELL limit order
    info!("📉 Placing a SELL limit order for BTC/USDT...");
    match agent
        .order(
            "GRND/USDT",                       // market
            OrderSide::Sell,                   // side
            Decimal::from_str("5.1").unwrap(), // price: $55,000
            Decimal::from_str("1").unwrap(),   // quantity: 1 BTC
            OrderType::Limit,                  // order type
            OrderMode::Base,                   // base token mode
            None,                              // tp_limit
            None,                              // sl_trigger
            None,                              // sl_limit
            None,                              // timestamp_ms
        )
        .await
    {
        Ok(result) => info!(
            "✅ SELL order placed successfully for BTC/USDT, result: {}",
            result
        ),
        Err(e) => {
            error!("❌ Failed to place SELL order: {}", e);
            return Err(e.into());
        }
    }

    info!("✨ Sell order operation completed!");
    info!("⚠️  WARNING: This placed a real order! Use testnet and small amounts!");
    Ok(())
}
