//! Query perp account state: account, positions, and funding history.
//!
//! Set PERP_L1_ADDRESS and optionally PERP_API_URL before running.
//!
//! ```sh
//! PERP_L1_ADDRESS=0x... cargo run --example perp_query
//! ```

use alphasec_rs::{
    perp::types::{PerpFundingQuery, PerpHistoryQuery},
    types::constants::endpoints::ALPHASEC_PERP_API_TESTNET_URL,
    Agent, Config,
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Starting perp query example");

    let _ = dotenvy::dotenv(); // load .env (repo root) if present

    let api_url =
        std::env::var("PERP_API_URL").unwrap_or_else(|_| ALPHASEC_PERP_API_TESTNET_URL.to_string());

    let l1_address = std::env::var("PERP_L1_ADDRESS")
        .unwrap_or_else(|_| "0x0000000000000000000000000000000000000001".to_string());

    let config = Config::new(
        &api_url,
        "kairos",
        &l1_address,
        None,  // read-only: no private key needed for GET queries
        None,  // L2 key
        false, // session disabled
        None,  // chain ID
    )?;

    let agent = Agent::new(config).await?;
    info!("Agent initialized (endpoint: {})", api_url);

    // Get perp account summary.
    match agent.perp().get_account().await {
        Ok(account) => info!("Account: {:?}", account),
        Err(e) => info!("get_account error (expected without live key): {}", e),
    }

    // Get open positions.
    match agent.perp().get_positions().await {
        Ok(positions) => info!("Positions: {} open", positions.len()),
        Err(e) => info!("get_positions error: {}", e),
    }

    // Get funding payment history (last 20 entries).
    let funding_query = PerpFundingQuery {
        market_id: None,
        from: None,
        to: None,
        last_id: None,
        limit: Some(20),
    };
    match agent.perp().get_funding(funding_query).await {
        Ok(items) => info!("Funding payments: {} entries", items.len()),
        Err(e) => info!("get_funding error: {}", e),
    }

    // Get position history (last 10 entries).
    let history_query = PerpHistoryQuery {
        market_id: None,
        from: None,
        to: None,
        limit: Some(10),
    };
    match agent.perp().get_position_history(history_query).await {
        Ok(history) => info!("Position history: {} entries", history.len()),
        Err(e) => info!("get_position_history error: {}", e),
    }

    info!("Perp query example completed");
    Ok(())
}
