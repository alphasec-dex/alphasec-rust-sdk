//! This example demonstrates how to use the WebSocket client in a channel-based

use alphasec_rust_sdk::{Agent, Config};
use tokio::time::{sleep, Duration, Instant};
use tracing::{info, error, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🚀 Starting WebSocket channel example");

    // Create configuration for Kairos testnet
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0x70dBb395AF2eDCC2833D803C03AbBe56ECe7c25c",
        Some("0xca8c450e6775a185f2df9b41b97f03906343f0703bdeaa86200caae8605d0ff8"),
        None, // L2 key, no session
        false, // L1 key, no session
    )?;

    // Create agent
    let mut agent = Agent::new(config).await?;

    // Start WebSocket connection
    agent.start().await?;
    info!("✅ WebSocket connection started");

    // Get the message receiver (can only be called once)
    let mut message_receiver = agent.take_message_receiver().await
        .expect("Failed to get message receiver");

    // Subscribe to multiple channels
    let sub1 = agent.subscribe("ticker@KAIA/USDT").await?;
    let sub2 = agent.subscribe("trade@KAIA/USDT").await?;
    let sub3 = agent.subscribe("depth@KAIA/USDT").await?;
    let sub4 = agent.subscribe("userEvent@0x70dBb395AF2eDCC2833D803C03AbBe56ECe7c25c").await?;

    // info!("📡 Subscribed to channels: ticker={}, trades={}, depth={}, userEvent={}", sub1, sub2, sub3, sub4);
    info!("📡 Subscribed to channels userEvent = {}", sub4);

    // Spawn a task to process messages
    let message_processor = tokio::spawn(async move {
        let mut message_count = 0;
        let start = Instant::now();
        
        while let Some(message) = message_receiver.recv().await {
            message_count += 1;
            
            match message {
                alphasec_rust_sdk::types::WebSocketMessage::Ack { id, result } => {
                    info!("📡 Subscription ack #{}: id={}, result={}", 
                          message_count, id, result);
                }
                alphasec_rust_sdk::types::WebSocketMessage::TradeMsg { params, .. } => {
                    for trade in &params.result {
                    info!("💱 Trade update #{}: channel={}, trade_id={}, market_id={}, price={}, quantity={}, buy_order_id={}, sell_order_id={}, created_at={}, is_buyer_maker={}", 
                          message_count, params.channel, trade.trade_id, trade.market_id, 
                          trade.price, trade.quantity, trade.buy_order_id, trade.sell_order_id, trade.created_at, trade.is_buyer_maker);
                    }
                }
                alphasec_rust_sdk::types::WebSocketMessage::DepthMsg { params, .. } => {
                    info!("📊 Depth update #{}: channel={}, market={}, bids={}, asks={}", 
                          message_count, params.channel, params.result.market_id, 
                          params.result.bids.as_ref().map(|bids| bids.len()).unwrap_or(0), params.result.asks.as_ref().map(|asks| asks.len()).unwrap_or(0));
                }
                alphasec_rust_sdk::types::WebSocketMessage::TickerMsg { params, .. } => {
                    info!("📈 Ticker update #{}: channel={}, entries={}", 
                          message_count, params.channel, params.result.len());
                    for ticker in &params.result {
                        info!("  - Market: {}, Price: {}, Volume: {}", 
                              ticker.market_id, ticker.price, ticker.volume_24h);
                    }
                }
                alphasec_rust_sdk::types::WebSocketMessage::UserEventMsg { params, .. } => {
                    info!("👤 User event #{}: channel={}, type={}, order={}, status={}", 
                          message_count, params.channel, params.result.event_type, 
                          params.result.order_id, params.result.status);
                }
                alphasec_rust_sdk::types::WebSocketMessage::Generic(value) => {
                    info!("🔧 Generic message #{}: {:?}", message_count, value);
                }
            }

            // Stop after 30 seconds
            if start.elapsed().as_secs() > 30 {
                info!("📊 Received {} messages, stopping...", message_count);
                break;
            }
        }
        
        info!("🔚 Message processing completed");
    });

    // Let it run for a while
    sleep(Duration::from_secs(30)).await;

    // Unsubscribe from some channels
    info!("📡 Unsubscribing from ticker...");
    agent.unsubscribe(sub1).await?;
    info!("📡 Unsubscribing from trade...");
    agent.unsubscribe(sub2).await?;
    info!("📡 Unsubscribing from depth...");
    agent.unsubscribe(sub3).await?;
    info!("📡 Unsubscribing from userEvent...");
    agent.unsubscribe(sub4).await?;

    sleep(Duration::from_secs(10)).await;

    // Stop the WebSocket connection
    info!("🛑 Stopping WebSocket connection...");
    agent.stop().await;

    // Wait for message processor to complete
    if let Err(e) = message_processor.await {
        error!("Message processor error: {}", e);
    }

    info!("✅ WebSocket channel example completed");
    Ok(())
}
