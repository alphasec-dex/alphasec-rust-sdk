//! Basic market data example
//! 
//! This example demonstrates how to fetch basic market information:
//! - Market list
//! - Tickers
//! - Tokens
//! - Recent trades
//! 
//! 
//! To run this example:
//! 1. Edit the hardcoded values in the source code
//! 2. Run: cargo run --example basic_market_data

use alphasec_rust_sdk::{Agent, Config};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AlphaSec basic market data example");

    // Create configuration with hardcoded values for testing
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",  // Your L1 address
        Some("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"), // Your private key (no 0x prefix)
        None, // L2 key, no session
        false, // L1 key, no session
        None // Chain ID
    )?;

    // Create Agent
    let agent = Agent::new(config).await?;
    info!("✅ AlphaSec Agent initialized successfully");

    info!("📊 === MARKET DATA ===");
    
    // Get all available tokens
    match agent.get_tokens().await {
        Ok(tokens) => {
            info!("✅ Available tokens: {}", tokens.len());
            for token in tokens.iter().take(5) {  // Show first 5
                info!("  - {} (ID: {})", token.symbol, token.token_id);
            }
        }
        Err(e) => error!("❌ Failed to get tokens: {}", e),
    }

    // Get market list
    match agent.get_market_list().await {
        Ok(markets) => {
            info!("✅ Available markets: {}", markets.len());
            for market in markets.iter().take(5) {  // Show first 5
                info!("  - {} (ID: {})", market.ticker, market.market_id);
            }
        }
        Err(e) => error!("❌ Failed to get market list: {}", e),
    }

    // Get all tickers
    match agent.get_tickers().await {
        Ok(tickers) => {
            info!("✅ Tickers retrieved: {}", tickers.len());
            for ticker in tickers.iter().take(3) {  // Show first 3
                info!("  - Market {}: Price=${}, Open=${}, High=${}, Low=${}, Volume={}", 
                      ticker.market_id, 
                      ticker.price, 
                      ticker.open_24h,
                      ticker.high_24h, 
                      ticker.low_24h,
                      ticker.volume_24h);
            }
        }
        Err(e) => error!("❌ Failed to get tickers: {}", e),
    }

    // Get specific ticker (example: BTC/USDT)
    match agent.get_ticker("BTC/USDT").await {
        Ok(ticker) => {
            let price = ticker.price.parse::<f64>().unwrap();
            let open_24h = ticker.open_24h.parse::<f64>().unwrap();
            let change = price - open_24h;
            let change_percent = change / open_24h * 100.0;
            info!("✅ BTC/USDT ticker:");
            info!("  Price: ${}", price);
            info!("  24h Change: {}%", change_percent);
            info!("  24h Change Amount: ${}", change);
            info!("  Open Price: ${}", open_24h);
        }
        Err(e) => error!("❌ Failed to get BTC/USDT ticker: {}", e),
    }

    // Get recent trades for a specific market
    match agent.get_trades("BTC/USDT", Some(10)).await {
        Ok(trades) => {
            info!("✅ Recent BTC/USDT trades: {}", trades.len());
            let price = trades[0].price.parse::<f64>().unwrap();
            let quantity = trades[0].quantity.parse::<f64>().unwrap();
            for trade in trades.iter().take(3) {  // Show first 3
                info!("  - {} BTC at ${} ({})", 
                      quantity, price, trade.is_buyer_maker);
            }
        }
        Err(e) => error!("❌ Failed to get trades: {}", e),
    }

    Ok(())
}
