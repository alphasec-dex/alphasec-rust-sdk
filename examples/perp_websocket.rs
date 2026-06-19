//! Perp WebSocket subscribe + decode example.
//!
//! Subscribes to a perp mark-price channel, receives messages as
//! `WebSocketMessage::Generic(Value)`, calls `decode_perp_event` on each,
//! and prints the typed `PerpEvent`.  Mirrors the style of examples/websocket.rs.
//!
//! ```sh
//! cargo run --example perp_websocket
//! ```

use alphasec_rs::{
    perp::ws::decode_perp_event,
    types::{constants::endpoints::ALPHASEC_PERP_API_TESTNET_URL, WebSocketMessage},
    Agent, Config,
};
use tokio::time::{sleep, Duration};
use tracing::{error, info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    info!("Starting perp websocket example");

    let _ = dotenvy::dotenv(); // load .env (repo root) if present

    let api_url =
        std::env::var("PERP_API_URL").unwrap_or_else(|_| ALPHASEC_PERP_API_TESTNET_URL.to_string());

    let l1_address = std::env::var("PERP_L1_ADDRESS")
        .unwrap_or_else(|_| "0x0000000000000000000000000000000000000001".to_string());

    let config = Config::new(
        &api_url,
        "kairos",
        &l1_address,
        None,  // no key needed for public market channels
        None,  // L2 key
        false, // session disabled
        None,  // chain ID
    )?;

    let mut agent = Agent::new(config).await?;

    // Start the WebSocket connection.
    agent.start().await?;
    info!("WebSocket connection started");

    // Take the receiver — can only be called once per agent.
    let mut message_receiver = agent
        .take_message_receiver()
        .await
        .expect("Failed to take message receiver");

    // Subscribe to the BTC/USDT perp mark-price channel.
    // Channel format: perp_markPrice@{marketId}
    let channel = "perp_markPrice@1";
    let sub_id = agent.subscribe(channel).await?;
    info!("Subscribed to '{}' (id={})", channel, sub_id);

    // Spawn a task that receives messages and decodes perp events.
    let channel_owned = channel.to_string();
    let message_processor = tokio::spawn(async move {
        let mut count = 0u32;

        while let Some(message) = message_receiver.recv().await {
            count += 1;

            match message {
                WebSocketMessage::Ack { id, result } => {
                    info!("Subscription ack #{}: id={}, result={}", count, id, result);
                }
                WebSocketMessage::Generic(value) => {
                    // Extract the `result` payload from the WS envelope if present,
                    // otherwise pass the whole value to the decoder.
                    let payload = value
                        .get("params")
                        .and_then(|p| p.get("result"))
                        .unwrap_or(&value);

                    match decode_perp_event(&channel_owned, payload) {
                        Ok(event) => info!("PerpEvent #{}: {:?}", count, event),
                        Err(e) => info!("decode_perp_event error #{}: {}", count, e),
                    }
                }
                WebSocketMessage::Disconnected => {
                    info!("Disconnected");
                    break;
                }
                WebSocketMessage::Pong(_) => {
                    info!("Pong #{}", count);
                }
                other => {
                    info!("Other message #{}: {:?}", count, other);
                }
            }

            if count >= 10 {
                info!("Received {} messages, stopping", count);
                break;
            }
        }

        info!("Message processing completed");
    });

    // Let the processor run for 15 seconds or until it finishes.
    sleep(Duration::from_secs(15)).await;

    // Unsubscribe and stop.
    agent.unsubscribe(sub_id).await?;
    agent.stop().await;

    if let Err(e) = message_processor.await {
        error!("Message processor error: {}", e);
    }

    info!("Perp websocket example completed");
    Ok(())
}
