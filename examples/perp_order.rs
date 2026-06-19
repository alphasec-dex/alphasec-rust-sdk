//! Place and cancel a perp order example.
//!
//! Mirrors the style of examples/place_buy_order.rs.
//! Set PERP_PRIVATE_KEY (hex, no 0x prefix) and optionally PERP_L1_ADDRESS
//! before running.  The endpoint defaults to the testnet constant; override with PERP_API_URL.
//!
//! ```sh
//! PERP_PRIVATE_KEY=<hex_key> cargo run --example perp_order
//! ```

use alphasec_rs::{
    perp::types::TimeInForce,
    types::{constants::endpoints::ALPHASEC_PERP_API_TESTNET_URL, orders::OrderSide},
    Agent, Config,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Starting perp order example");

    let _ = dotenvy::dotenv(); // load .env (repo root) if present

    // Load signing key from environment; fall back to a placeholder so the
    // binary compiles and the Config path is exercised even without a live key.
    let private_key = std::env::var("PERP_PRIVATE_KEY").ok();
    let l1_address = std::env::var("PERP_L1_ADDRESS")
        .unwrap_or_else(|_| "0x0000000000000000000000000000000000000001".to_string());

    let api_url =
        std::env::var("PERP_API_URL").unwrap_or_else(|_| ALPHASEC_PERP_API_TESTNET_URL.to_string());

    let config = Config::new(
        &api_url,
        "kairos",
        &l1_address,
        private_key.as_deref(),
        None,  // L2 key
        false, // session disabled
        None,  // chain ID
    )?;

    let agent = Agent::new(config).await?;
    info!("Agent initialized (endpoint: {})", api_url);

    if private_key.is_none() {
        info!("PERP_PRIVATE_KEY not set — skipping live order (key required)");
        return Ok(());
    }

    // Place a BUY limit order on the BTC/USDT perp market.
    info!("Placing BUY limit order on BTCUSDT perp...");
    let order_result = agent
        .perp()
        .order(
            "BTCUSDT",
            OrderSide::Buy,
            Decimal::from_str("90000").unwrap(), // price
            Decimal::from_str("0.001").unwrap(), // quantity
            TimeInForce::Gtc,
            false, // reduce_only
            None,  // client_order_id
        )
        .await;

    let order_tx = match order_result {
        Ok(tx) => {
            info!("Order placed. tx_hash={}", tx);
            tx
        }
        Err(e) => {
            error!("Failed to place order: {}", e);
            return Err(e.into());
        }
    };

    // Cancel the order we just placed (order_id = tx hash returned by the server).
    info!("Cancelling order tx={}", order_tx);
    match agent.perp().cancel("BTCUSDT", &order_tx).await {
        Ok(cancel_tx) => info!("Order cancelled. tx_hash={}", cancel_tx),
        Err(e) => error!("Cancel failed: {}", e),
    }

    info!("Perp order example completed");
    Ok(())
}
