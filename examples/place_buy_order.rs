//! Place buy order example
//! 
//! This example demonstrates how to place a single buy order only

use alphasec_rust_sdk::{Agent, Config, OrderSide, OrderType, OrderMode};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec buy order example");

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

    // Place a BUY limit order
    info!("📈 Placing a BUY limit order for KAIA/USDT...");
    match agent.order(
        "KAIA/USDT",           // market
        OrderSide::Buy,       // side
        1f64,                // price: $1.1
        1f64,             // quantity: 1 KAIA
        OrderType::Limit,     // order type
        OrderMode::Base,      // base token mode
        None,                 // tp_limit
        None,                 // sl_trigger
        None,                 // sl_limit
    ).await {
        Ok(result) => info!("✅ BUY order placed successfully for KAIA/USDT, result: {}", result),
        Err(e) => {
            error!("❌ Failed to place BUY order: {}", e);
            return Err(e.into());
        }
    }

    info!("✨ Buy order operation completed!");
    info!("⚠️  WARNING: This placed a real order! Use testnet and small amounts!");
    Ok(())
}
