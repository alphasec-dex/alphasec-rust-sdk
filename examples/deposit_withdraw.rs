use alphasec_rust_sdk::{Agent, Config};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting AlphaSec deposit with withdraw example");

    let config = Config::new(
        "https://api-testnet.alphasec.trade",
        "kairos",
        "0x70dBb395AF2eDCC2833D803C03AbBe56ECe7c25c", // Your L1 address
        Some("0xca8c450e6775a185f2df9b41b97f03906343f0703bdeaa86200caae8605d0ff8"), // Your private key (no 0x prefix)
        Some("0x3a27159a9c2fc4f837a086f24bcf80f5f270e9d1224c6953859656f94c2fe2f3"), // L2 key, no session
        false, // L1 key, no session
        None,  // Chain ID
    )
    .unwrap();

    let agent = Agent::new(config).await.unwrap();
    info!("✅ AlphaSec Agent initialized successfully");

    info!("Depositing 1.0 KAIA to AlphaSec...");
    // match agent.deposit_token("KAIA", 1.0).await {
    match agent.deposit_token("KAIA", 1.0).await {
        Ok(result) => info!("✅ Deposit successful: 1.0 KAIA, result: {}", result),
        Err(e) => error!("❌ Failed to deposit KAIA: {}", e),
    };

    info!("Withdrawing 1.0 KAIA from AlphaSec...");
    // match agent.withdraw_token("KAIA", 1.0).await {
    match agent.withdraw_token("KAIA", 1.0).await {
        Ok(result) => info!("✅ Withdrawal successful: 1.0 KAIA, result: {}", result),
        Err(e) => error!("❌ Failed to withdraw KAIA: {}", e),
    };

    info!("✨ Deposit with withdraw example completed!");
    Ok(())
}
