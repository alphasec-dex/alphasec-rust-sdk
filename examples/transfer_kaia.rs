//! Transfer KAIA example
//! 
//! This example demonstrates how to transfer native KAIA tokens only

use alphasec_rust_sdk::{Agent, Config};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec KAIA transfer example");

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

    // Recipient address (replace with actual address)
    let recipient = "0xbAc1Aef897710759AAf5e6322A0c3EA5f58Df1C3"; // Example address
    let amount = 0.1f64; // 1 KAIA

    // Value transfer (native KAIA)
    info!("💰 Transferring {} KAIA to {}...", amount, recipient);
    match agent.native_transfer(recipient, amount).await {
        Ok(result) => info!("✅ Value transfer successful: {} KAIA to {}, result: {}", amount, recipient, result),
        Err(e) => {
            error!("❌ Failed to transfer KAIA: {}", e);
            return Err(e.into());
        }
    }

    info!("✨ KAIA transfer operation completed!");
    info!("💡 To run this example:");
    info!("   1. Replace recipient address with actual address");
    info!("   2. Make sure you have sufficient KAIA balance");
    info!("⚠️  WARNING: This transfers real KAIA! Use testnet and small amounts!");
    Ok(())
}
