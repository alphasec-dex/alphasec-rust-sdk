# AlphaSec Rust SDK

A comprehensive Rust SDK for interacting with the AlphaSec orderbook DEX, built on the Kaia blockchain.

[![Crates.io](https://img.shields.io/crates/v/alphasec-rust-sdk.svg)](https://crates.io/crates/alphasec-rust-sdk)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## 🔗 Links

- [Official](https://alphasec.io)
- [Telegram](https://t.me/alphasecofficial)
- [Discord](https://discord.gg/alphasec)
- [X](https://x.com/AlphaSec_Trade)

## 🌐 Network Information

### Kairos Testnet
- **API URL**: `https://api.alphasec.trade`
- **Websocket URL**: `wss://api.alphasec.trade/ws`
- **Network**: `kairos`
- **L1 Chain ID**: 1001 (Kaia Kairos)
- **L2 Chain ID**: 412346 (AlphaSec L2)

### Mainnet
- **API URL**: `https://api-testnet.alphasec.trade`
- **Websocket URL**: `wss://api-testnet.alphasec.trade/ws`
- **Network**: `mainnet`
- **L1 Chain ID**: 8217 (Kaia Mainnet)
- **L2 Chain ID**: 412346 (AlphaSec L2)

## 📖 Configuration

### Environment-specific Configurations

```rust
use alphasec_rust_sdk::{Agent, Config};

// Note: private keys are optional; pass Some(key) when signing is needed.
let config = Config::new(
    "https://api-testnet.alphasec.trade", // API base URL
    "kairos",                      // network: "kairos" | "mainnet"
    "0x1234567890123456789012345678901234567890", // L1 address
    Some("<l1_private_key_hex_without_0x>"),       // L1 key (optional)
    Some("<l2_private_key_hex_without_0x>"),       // L2 key (optional)
    true                                            // session_enabled
)?;

let mut agent = Agent::new(config).await?;
```

## API Reference

### Market Data

```rust
// Get all available tokens
let tokens = agent.get_tokens().await?;
// Get all markets
let markets = agent.get_market_list().await?;
// Get all tickers
let tickers = agent.get_tickers().await?;
// Get specific ticker
let ticker = agent.get_ticker("KAIA/USDT").await?;
// Get recent trades
let trades = agent.get_trades("KAIA/USDT", Some(50)).await?;
```

### Account Information

```rust
// Get account balance
let balances = agent.get_balance("0x...").await?;
// Get active sessions
let sessions = agent.get_sessions("0x...").await?;
// Get open orders
let open_orders = agent.get_open_orders("0x...", None, Some(100), None, None).await?;
// Get order history
let order_history = agent.get_filled_canceled_orders("0x...", Some("KAIA/USDT"), Some(50), None, None).await?;
// Get specific order
let order = agent.get_order_by_id("order_id").await?;
```

### Trading Operations

```rust
use alphasec_rust_sdk::{OrderSide, OrderType, OrderMode};

// Place a limit buy order
// order_id : tx_hash
let order_id = agent.order(
    "KAIA/USDT",                // market
    OrderSide::Buy,             // side
    1,                          // price (1 USDT)
    5,                          // quantity (5 KAIA)
    OrderType::Limit,           // order type
    OrderMode::Base,            // order mode (base/quote)
    None,                       // take profit
    None,                       // stop loss trigger
    None                        // stop loss limit
).await?;
// Cancel an order
let success = agent.cancel("order_id").await?;
// Cancel all orders
let success = agent.cancel_all().await?;
// Modify an order
let success = agent.modify(
    "order_id",
    11,             // new price
    4,              // new quantity
    None            // new order mode
).await?;
```

### Session Operations

```rust
use ethers::signers::LocalWallet;

// Provide a session wallet (L2) explicitly or let SDK use Config.l2_wallet
let session_wallet: Option<LocalWallet> = None;
let session_id = "my_session";
let now_ms = chrono::Utc::now().timestamp_millis() as u64;
let expires_ms = now_ms + 60 * 60 * 1000; // +1h
let metadata = b"SDK session";

// Create / Update / Delete session (EIP-712 signing under the hood)
let result = agent.create_session(session_id, session_wallet.clone(), now_ms, expires_ms, metadata).await?;
let result = agent.update_session(session_id, session_wallet.clone(), now_ms, expires_ms, metadata).await?;
let result = agent.delete_session(session_id, session_wallet).await?;
```

### Asset Transfers

```rust
// Transfer native tokens (KAIA)
let success = agent.native_transfer("0x...", 1).await?; // 1 KAIA
// Transfer ERC20 tokens
let success = agent.token_transfer("0x...", 1, "USDT").await?; // 1 USDT
```

### WebSocket Streaming

```rust
// Enable websocket feature in Cargo.toml
// alphasec-rust-sdk = { version = "0.1.0", features = ["websocket"] }

// Start WebSocket connection
agent.start().await?;

// Get message receiver (can be taken once)
let mut rx = agent.take_message_receiver().await.expect("receiver taken once");

// Subscribe to channels
let sub_ticker = agent.subscribe("ticker@KAIA/USDT").await?;
let sub_trade  = agent.subscribe("trade@KAIA/USDT").await?;

// Consume messages (simplified)
while let Some(msg) = rx.recv().await {
    match msg {
        alphasec_rust_sdk::types::WebSocketMessage::TickerMsg { params, .. } => {
            println!("📈 {} entries", params.result.len());
        }
        alphasec_rust_sdk::types::WebSocketMessage::TradeMsg { params, .. } => {
            for t in &params.result {
                println!("💱 trade market={} price={} qty={}", t.market_id, t.price, t.quantity);
            }
        }
        _ => {}
    }
}

// Unsubscribe and stop
agent.unsubscribe(sub_ticker).await?;
agent.unsubscribe(sub_trade).await?;
agent.stop().await;
```

## 📋 Examples

The SDK includes several focused examples for different use cases:

### Basic Examples (No Dependencies)
- [`get_market_data.rs`](examples/get_market_data.rs) - Market list only
- [`get_tokens.rs`](examples/get_tokens.rs) - Available tokens only
- [`get_tickers.rs`](examples/get_tickers.rs) - Ticker information only
- [`get_balance.rs`](examples/get_balance.rs) - Account balance only

### Trading Examples (Independent)
- [`place_buy_order.rs`](examples/place_buy_order.rs) - Single buy order only
- [`place_sell_order.rs`](examples/place_sell_order.rs) - Single sell order only
- [`cancel_all_orders.rs`](examples/cancel_all_orders.rs) - Cancel all orders only

### Transfer Examples (Independent)
- [`transfer_kaia.rs`](examples/transfer_kaia.rs) - Native KAIA transfer only
- [`transfer_usdt.rs`](examples/transfer_usdt.rs) - USDT token transfer only

### WebSocket Examples (Independent)
- [`websocket.rs`](examples/websocket.rs) - Multi-channel subscription demo

### Running Examples
```bash
cargo run --example get_market_data
cargo run --example get_tokens
cargo run --example get_tickers
cargo run --example websocket
cargo run --example place_buy_order
cargo run --example place_sell_order
cargo run --example deposit_withdraw
```

## 🔐 Security

⚠️ **Important Security Notes:**

- Never commit private keys to version control
- Use environment variables or secure key management
- Always test on kairos testnet before mainnet
- Verify transaction details before signing
- Blockchain transactions are irreversible

## ⚠️ Disclaimer

This SDK is provided as-is. Trading cryptocurrencies involves substantial risk and may result in significant losses. Always do your own research and never invest more than you can afford to lose. The developers are not responsible for any trading losses incurred while using this SDK.
