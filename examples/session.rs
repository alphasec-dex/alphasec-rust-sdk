use alphasec_rs::{Agent, Config};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Initialize configuration
    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0x0000000000000000000000000000000000000000", // Your L1 address
        Some("0x0000000000000000000000000000000000000000000000000000000000000000"), // Your private key (no 0x prefix)
        Some("0xb7c58f04896daeaf67676d52ad8d5e33b295779eb4962b26b335e172285cec66"), // L2 key, no session
        false, // L1 key, no session
        None,  // Chain ID
    )
    .unwrap();

    // Initialize agent
    let agent = Agent::new(config).await.unwrap();

    // Create session
    let session_id = "test_session";
    let timestamp_ms = chrono::Utc::now().timestamp_millis() as u64;
    let expires_at = timestamp_ms + 1000 * 60 * 60;
    let metadata = b"SDK session";

    // If the session address is not provided, it will use the L2 wallet in the config
    let result = agent
        .create_session(session_id, None, timestamp_ms, expires_at, metadata)
        .await
        .unwrap();
    info!("Create session: {}", result);

    // Update session
    let timestamp_ms = chrono::Utc::now().timestamp_millis() as u64;
    let expires_at = timestamp_ms + 1000 * 60 * 60 * 24;
    let metadata = b"SDK session";
    let result = agent
        .update_session(session_id, None, timestamp_ms, expires_at, metadata)
        .await
        .unwrap();
    info!("Update session: {}", result);

    // Delete session
    let timestamp_ms = chrono::Utc::now().timestamp_millis() as u64;
    let result = agent.delete_session(None, timestamp_ms).await.unwrap();
    info!("Delete session: {}", result);

    // Get session
    let result = agent.get_sessions(agent.l1_address()).await.unwrap();
    info!("Get sessions: {:?}", result);

    Ok(())
}
