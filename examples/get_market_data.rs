//! Get market data example
//!
//! This example demonstrates how to fetch market information only

use alphasec_rust_sdk::{Agent, Config};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec market data example");

    // Create configuration with hardcoded values for testing
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", // Your L1 address
        Some("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"), // Your private key (no 0x prefix)
        None,  // L2 key, no session
        false, // L1 key, no session
        None,  // Chain ID
    )?;

    // Create Agent
    let agent = Agent::new(config).await?;
    info!("✅ AlphaSec Agent initialized successfully");

    // Get market list only
    match agent.get_market_list().await {
        Ok(markets) => {
            info!("✅ Available markets: {}", markets.len());
            for market in markets.iter().take(10) {
                // Show first 10
                info!("  - {} (ID: {})", market.ticker, market.market_id);
            }
        }
        Err(e) => {
            error!("❌ Failed to get market list: {}", e);
            return Err(e.into());
        }
    }

    info!("✨ Market data retrieved successfully!");
    Ok(())
}
