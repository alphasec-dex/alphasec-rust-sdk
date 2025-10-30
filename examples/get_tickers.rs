//! Get tickers example
//! 
//! This example demonstrates how to fetch ticker information only

use alphasec_rust_sdk::{Agent, Config};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec tickers example");

    // Create configuration with hardcoded values for testing
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",  // Your L1 address
        Some("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"), // Your private key (no 0x prefix)
        None, // L2 key, no session
        false, // L1 key, no session
        None // Chain ID
    )?;

    // Create Agent
    let agent = Agent::new(config).await?;
    info!("✅ AlphaSec Agent initialized successfully");

    // Get all tickers
    match agent.get_tickers().await {
        Ok(tickers) => {
            info!("✅ Tickers retrieved: {}", tickers.len());
            for ticker in tickers.iter().take(10) {  // Show first 10
                info!("  - Market {}: Price=${}, Open=${}, High=${}, Low=${}, Volume={}", 
                      ticker.market_id, 
                      ticker.price, 
                      ticker.open_24h,
                      ticker.high_24h, 
                      ticker.low_24h,
                      ticker.volume_24h);
            }
        }
        Err(e) => {
            error!("❌ Failed to get tickers: {}", e);
            return Err(e.into());
        }
    }

    info!("✨ Tickers retrieved successfully!");
    Ok(())
}
