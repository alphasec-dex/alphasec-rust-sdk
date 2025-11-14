//! Account information example
//!
//! This example demonstrates how to fetch account-related information:
//! - Balance information
//! - Session information  
//! - Open orders
//! - Order history

use alphasec_rust_sdk::{Agent, Config, OrderSide};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec account information example");

    // Create configuration with hardcoded values for testing
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0x70dBb395AF2eDCC2833D803C03AbBe56ECe7c25c", // Your address
        Some("ca8c450e6775a185f2df9b41b97f03906343f0703bdeaa86200caae8605d0ff8"), // Your private key (no 0x prefix)
        None,  // L2 key, no session
        false, // L1 key, no session
        None,  // Chain ID
    )?;

    // Create Agent
    let agent = Agent::new(config).await?;
    info!("✅ AlphaSec Agent initialized successfully");

    let address = agent.l1_address();
    info!("📱 Using address: {}", address);

    // === Account Information ===

    info!("💰 === ACCOUNT INFORMATION ===");

    // Get balance information
    match agent.get_balance(address).await {
        Ok(balances) => {
            info!("✅ Total tokens: {}", balances.balances.len());
            for balance in balances.balances.iter().take(10) {
                // Show first 10
                let token_id = balance.token_id.clone();
                let locked = balance.locked.as_deref().unwrap_or("0");
                let unlocked = balance.unlocked.as_deref().unwrap_or("0");
                info!(
                    "  - {} (ID: {}): Locked={}, Unlocked={}",
                    token_id, balance.token_id, locked, unlocked
                );
            }
        }
        Err(e) => error!("❌ Failed to get balance: {}", e),
    }

    // Get session information
    match agent.get_sessions(address).await {
        Ok(sessions) => {
            info!("✅ Active sessions: {}", sessions.len());
            for session in sessions.iter() {
                info!("  - Session ID: {}", session.name);
                info!("    Session Address: {}", session.session_address);
                info!("    Owner Address: {}", session.owner_address);
                info!("    Expires: {}", session.expiry);
                info!(
                    "    Status: {}",
                    if session.applied {
                        "Active"
                    } else {
                        "Inactive"
                    }
                );
            }
        }
        Err(e) => error!("❌ Failed to get sessions: {}", e),
    }

    // === Order Information ===

    info!("📋 === ORDER INFORMATION ===");

    // Get open orders for BTC/USDT
    match agent
        .get_open_orders(address, Some("GRND/USDT"), Some(50), None, None)
        .await
    {
        Ok(orders) => {
            if orders.is_empty() {
                info!("✅ No open orders for GRND/USDT");
            } else {
                info!("✅ Open orders for GRND/USDT: {}", orders.len());
                for order in orders.iter().take(5) {
                    // Show first 5
                    info!(
                        "  - Order {}: {} {} at {} ({})",
                        order.order_id,
                        order.side, // Already a string like "BUY" or "SELL"
                        order.orig_qty,
                        order.price,
                        order.status
                    );
                }
            }
        }
        Err(e) => error!("❌ Failed to get open orders: {}", e),
    }

    // Get filled/canceled orders for BTC/USDT
    match agent
        .get_filled_canceled_orders(address, Some("BTC/USDT"), Some(20), None, None)
        .await
    {
        Ok(orders) => {
            if orders.is_empty() {
                info!("✅ No recent filled/canceled orders for BTC/USDT");
            } else {
                info!(
                    "✅ Recent filled/canceled orders for BTC/USDT: {}",
                    orders.len()
                );
                for order in orders.iter().take(5) {
                    // Show first 5
                    info!(
                        "  - Order {}: {} {} at {} ({})",
                        order.order_id,
                        order.side, // Already a string like "BUY" or "SELL"
                        order.orig_qty,
                        order.price,
                        order.status
                    );
                }
            }
        }
        Err(e) => error!("❌ Failed to get order history: {}", e),
    }

    // Try to get a specific order by ID (example)
    let example_order_id = "example-order-id-123";
    match agent.get_order_by_id(example_order_id).await {
        Ok(Some(order)) => {
            info!("✅ Found order {}:", example_order_id);
            info!(
                "  - {} {} at {} ({})",
                order.side, // Already a string like "BUY" or "SELL"
                order.orig_qty,
                order.price,
                order.status
            );
        }
        Ok(None) => {
            warn!("⚠️  Order {} not found", example_order_id);
        }
        Err(e) => error!("❌ Failed to get order by ID: {}", e),
    }

    info!("✨ === EXAMPLE COMPLETED ===");
    info!("💡 To run this example:");
    info!("   1. Edit the hardcoded values in the source code");
    info!("   2. Run: cargo run --example account_info");
    info!("⚠️  Note: You need actual account data to see meaningful results");

    Ok(())
}
