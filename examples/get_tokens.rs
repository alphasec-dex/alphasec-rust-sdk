//! Get tokens example
//!
//! This example demonstrates how to fetch available tokens only

use alphasec_rust_sdk::{Agent, Config};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec tokens example");

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

    // Get all available tokens
    match agent.get_tokens().await {
        Ok(tokens) => {
            info!("✅ Available tokens: {}", tokens.len());
            for token in tokens.iter() {
                info!(
                    "  - {} (ID: {}, Address: {})",
                    token.symbol, token.token_id, token.l1_address
                );
            }
        }
        Err(e) => {
            error!("❌ Failed to get tokens: {}", e);
            return Err(e.into());
        }
    }

    info!("✨ Tokens retrieved successfully!");
    Ok(())
}
