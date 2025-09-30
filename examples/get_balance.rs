//! Get balance example
//! 
//! This example demonstrates how to fetch account balance only

use alphasec_rust_sdk::{Agent, Config};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec balance example");

    // Create configuration with hardcoded values for testing
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",  // Your L1 address
        Some("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"), // Your private key (no 0x prefix)
        None, // L2 key, no session
        false, // L1 key, no session
    )?;

    // Create Agent
    let agent = Agent::new(config).await?;
    info!("✅ AlphaSec Agent initialized successfully");

    let address = agent.l1_address();
    info!("📱 Using address: {}", address);

    // Get balance information
    match agent.get_balance(address).await {
        Ok(balances) => {
            info!("✅ Total tokens: {}", balances.len());
            for balance in balances.iter().take(10) {  // Show first 10
                    let token_id = balance.token_id.clone();
                    let locked = balance.locked.as_deref().unwrap_or("0");
                    let unlocked = balance.unlocked.as_deref().unwrap_or("0");
                    info!("  - {} (ID: {}): Locked={}, Unlocked={}", token_id, balance.token_id, locked, unlocked);
            }
        }
        Err(e) => error!("❌ Failed to get balance: {}", e),
    }

    info!("✨ Balance retrieved successfully!");
    Ok(())
}
