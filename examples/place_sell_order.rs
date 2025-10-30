//! Place sell order example
//! 
//! This example demonstrates how to place a single sell order only

use alphasec_rust_sdk::{Agent, Config, OrderSide, OrderType, OrderMode};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec sell order example");

    // Create configuration with hardcoded values for testing
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0x70dBb395AF2eDCC2833D803C03AbBe56ECe7c25c",  // Your L1 address
        Some("ca8c450e6775a185f2df9b41b97f03906343f0703bdeaa86200caae8605d0ff8"), // Your private key (no 0x prefix)
        None, // L2 key, no session
        false, // L1 key, no session
        None // Chain ID
    )?;

    // Create Agent
    let agent = Agent::new(config).await?;
    info!("✅ AlphaSec Agent initialized successfully");

    // Place a SELL limit order
    info!("📉 Placing a SELL limit order for BTC/USDT...");
    match agent.order(
        "GRND/USDT",           // market
        OrderSide::Sell,      // side
        5.1f64,                // price: $55,000
        1f64,                    // quantity: 1 BTC
        OrderType::Limit,     // order type
        OrderMode::Base,      // base token mode
        None,                 // tp_limit
        None,                 // sl_trigger
        None,                 // sl_limit
    ).await {
        Ok(result) => info!("✅ SELL order placed successfully for BTC/USDT, result: {}", result),
        Err(e) => {
            error!("❌ Failed to place SELL order: {}", e);
            return Err(e.into());
        }
    }

    info!("✨ Sell order operation completed!");
    info!("⚠️  WARNING: This placed a real order! Use testnet and small amounts!");
    Ok(())
}
