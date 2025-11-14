//! Transfer USDT example
//!
//! This example demonstrates how to transfer USDT tokens only

use alphasec_rust_sdk::{Agent, Config};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec USDT transfer example");

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

    // Recipient address (replace with actual address)
    let recipient = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"; // Example address
    let amount = 100f64; // 100 USDT
    let token = "USDT";

    // Token transfer (USDT)
    info!("🪙 Transferring {} {} to {}...", amount, token, recipient);
    match agent.token_transfer(recipient, amount, token).await {
        Ok(result) => info!(
            "✅ Token transfer successful: {} {} to {}, result: {}",
            amount, token, recipient, result
        ),
        Err(e) => {
            error!("❌ Failed to transfer {}: {}", token, e);
            return Err(e.into());
        }
    }

    info!("✨ USDT transfer operation completed!");
    info!("💡 To run this example:");
    info!("   1. Replace recipient address with actual address");
    info!("   2. Make sure you have sufficient USDT balance");
    info!("⚠️  WARNING: This transfers real USDT! Use testnet and small amounts!");
    Ok(())
}
