//! Simple trading example
//! 
//! This example demonstrates basic trading operations:
//! - Placing a buy order
//! - Placing a sell order
//! - Canceling an order
//! - Modifying an order

//! To run this example:
//! 1. Edit the hardcoded values in the source code
//! 2. Make sure you have sufficient balance
//! 3. Run: cargo run --example simple_trading

use alphasec_rust_sdk::{Agent, Config, OrderSide, OrderType, OrderMode};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec simple trading example");

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

    // === Trading Examples ===
    
    info!("💰 === TRADING OPERATIONS ===");

    // Example 1: Place a BUY limit order
    info!("📈 Placing a BUY limit order...");
    let order_id =  match agent.order(
        "KAIA/USDT",           // market
        OrderSide::Buy,       // side
        0.9f64,                // price: $0.9
        5f64,                    // quantity: 5 KAIA
        OrderType::Limit,     // order type
        OrderMode::Base,      // base token mode
        None,                 // tp_limit
        None,                 // sl_trigger
        None,                 // sl_limit
    ).await {
        Ok(result) => {
            info!("✅ BUY order placed successfully, order id: {}", result);
            result
        }
        Err(e) => {
            error!("❌ Failed to place BUY order: {}", e);
            return Err(e.into());
        }
    };

    // Wait a bit
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Example 5: Modify an order (you would need a real order ID)
    let example_order_id_modify = order_id.clone();
    info!("✏️  Attempting to modify order: {}", example_order_id_modify);
    match agent.modify(
        &example_order_id_modify,
        1.2f64,          // new_price: $1.2
        2f64,              // new_qty: 5 KAIA
        OrderMode::Base, // order_mode: Quote
    ).await {
        Ok(result) => info!("✅ Order modified successfully, result: {}", result),
        Err(e) => error!("❌ Failed to modify order: {}", e),
    }
    // Wait a bit between orders
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Example 2: Place a SELL limit order
    info!("📉 Placing a SELL limit order...");
    match agent.order(
        "KAIA/USDT",           // market
        OrderSide::Sell,      // side
        1.1f64,                // price: $55,000
        2f64,                    // quantity: 1 BTC
        OrderType::Limit,     // order type
        OrderMode::Base,      // base token mode
        None,                 // tp_limit
        None,                 // sl_trigger
        None,                 // sl_limit
    ).await {
        Ok(result) => info!("✅ SELL order placed successfully, result: {}", result),
        Err(e) => error!("❌ Failed to place SELL order: {}", e),
    }

    // Wait a bit
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Example 3: Cancel a specific order (you would need a real order ID)
    let example_order_id = order_id.clone();
    info!("🚫 Attempting to cancel order: {}", example_order_id);
    match agent.cancel(&example_order_id).await {
        Ok(result) => info!("✅ Order canceled successfully, result: {}", result),
        Err(e) => error!("❌ Failed to cancel order: {}", e),
    }

    // Wait a bit
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Example 4: Cancel all orders
    info!("🚫 Canceling all open orders...");
    match agent.cancel_all().await {
        Ok(result) => info!("✅ All orders canceled successfully, result: {}", result),
        Err(e) => error!("❌ Failed to cancel all orders: {}", e),
    }

    info!("✨ === EXAMPLE COMPLETED ===");
    info!("🎯 This example demonstrated:");
    info!("  ✅ Placing BUY limit orders");
    info!("  ✅ Modifying orders");
    info!("  ✅ Placing SELL limit orders");
    info!("  ✅ Canceling specific orders");
    info!("  ✅ Canceling all orders");
    
    info!("⚠️  WARNING: This places real orders! Use testnet and small amounts!");

    Ok(())
}
