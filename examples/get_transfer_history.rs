//! Get transfer history example
//!
//! This example demonstrates how to fetch wallet transfer history on the L2 network

use alphasec_rs::{types::constants::chain_ids::ALPHASEC_TESTNET_CHAIN_ID, Agent, Config};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec transfer history example");

    // Create configuration with hardcoded values for testing
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0x0000000000000000000000000000000000000000", // Your L1 address
        Some("0000000000000000000000000000000000000000000000000000000000000000"), // Your private key (no 0x prefix)
        None,                            // L2 key, no session
        false,                           // L1 key, no session
        Some(ALPHASEC_TESTNET_CHAIN_ID), // Chain ID for testnet
    )?;

    // Create Agent
    let agent = Agent::new(config).await?;
    info!("✅ AlphaSec Agent initialized successfully");

    let address = agent.l1_address();
    info!("📱 Using address: {}", address);

    // Get all transfer history (default limit: 100)
    info!("=== Getting Transfer History ===");
    match agent
        .get_transfer_history(address, None, None, None, None)
        .await
    {
        Ok(transfers) => {
            info!("📋 Found {} transfers", transfers.len());
            for transfer in transfers.iter().take(5) {
                info!(
                    "  Transfer #{}: {} -> {} | {} {} | Status: {} | Type: {}",
                    transfer.id,
                    &transfer.from_address[..10],
                    &transfer.to_address[..10],
                    transfer.amount,
                    transfer.token_id,
                    transfer.status,
                    transfer.tx_type
                );
            }
        }
        Err(e) => error!("❌ Failed to get transfer history: {}", e),
    }

    // Example: Get transfers with token_id filter
    info!("\n=== Getting Transfers for token_id=2 ===");
    match agent
        .get_transfer_history(address, Some(2), None, None, None)
        .await
    {
        Ok(transfers) => {
            info!("📋 Found {} transfers for token_id=2", transfers.len());
        }
        Err(e) => error!("❌ Failed to get filtered transfer history: {}", e),
    }

    // Example: Get transfers with limit
    info!("\n=== Getting Transfers with limit=1 ===");
    match agent
        .get_transfer_history(address, None, None, None, Some(1))
        .await
    {
        Ok(transfers) => {
            info!("📋 Found {} transfers with limit=1", transfers.len());
        }
        Err(e) => error!("❌ Failed to get limited transfer history: {}", e),
    }

    info!("✨ Transfer history retrieved successfully!");
    Ok(())
}
