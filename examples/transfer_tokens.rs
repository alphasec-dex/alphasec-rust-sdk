//! Token transfer example
//! 
//! This example demonstrates how to transfer tokens:
//! - Value transfer (native KAIA)
//! - Token transfer (ERC-20 tokens like USDT)

// To run this example:
//  1. Edit the hardcoded values in the source code
//  2. Replace recipient address with actual address
//  3. Make sure you have sufficient balance
//  4. Run: cargo run --example transfer_tokens
    
use alphasec_rust_sdk::{Agent, Config};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec token transfer example");

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

    // === Transfer Examples ===
    
    info!("💸 === TOKEN TRANSFERS ===");

    // Recipient address (replace with actual address)
    let recipient = "0xbAc1Aef897710759AAf5e6322A0c3EA5f58Df1C3"; // Example address
    
    // Example 1: Value transfer (native KAIA)
    // info!("💰 Transferring native KAIA...");
    // match agent.native_transfer(recipient, 1f64).await { // 1 KAIA
    //     Ok(result) => {
    //             info!("✅ Value transfer successful: 1 KAIA sent to {}", recipient);
    //             info!("  Result: {}", result);
    //         }
    //     Err(e) => error!("❌ Failed to transfer value: {}", e),
    // }

    // // Wait a bit between transactions
    // tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Example 2: Token transfer (USDT)
    info!("🪙 Transferring USDT tokens...");
    match agent.token_transfer(recipient, 1.55f64, "USDT").await { // 1.55 USDT
        Ok(result) => {
                info!("✅ Token transfer successful: 1.55 USDT sent to {}", recipient);
                info!("  Result: {}", result);
            }
        Err(e) => error!("❌ Failed to transfer USDT: {}", e),
    }
    info!("⚠️  WARNING: This transfers real tokens! Use testnet and small amounts!");
    info!("🔐 Never commit private keys to version control!");

    Ok(())
}
