//! Cancel all orders example
//!
//! This example demonstrates how to cancel all open orders only

use alphasec_rs::{Agent, Config};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec cancel all orders example");

    // Create configuration with hardcoded values for testing
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0x70dBb395AF2eDCC2833D803C03AbBe56ECe7c25c", // Your L1 address
        Some("ca8c450e6775a185f2df9b41b97f03906343f0703bdeaa86200caae8605d0ff8"), // Your private key (no 0x prefix)
        None,  // L2 key, no session
        false, // L1 key, no session
        None,  // Chain ID
    )?;

    // Create Agent
    let agent = Agent::new(config).await?;
    info!("✅ AlphaSec Agent initialized successfully");

    // Cancel all orders
    info!("🚫 Canceling all open orders...");
    match agent.cancel_all(None).await {
        Ok(result) => info!("✅ All orders canceled successfully, result: {}", result),
        Err(e) => {
            error!("❌ Failed to cancel all orders: {}", e);
            return Err(e.into());
        }
    }

    info!("✨ Cancel all operation completed!");
    info!("💡 This cancels ALL open orders across all markets");
    Ok(())
}
