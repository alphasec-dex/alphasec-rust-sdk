# AlphaSec Rust SDK

A Rust SDK for trading spot and perpetuals on the AlphaSec orderbook DEX, built on the Kaia blockchain.

[![Crates.io](https://img.shields.io/crates/v/alphasec-rs.svg)](https://crates.io/crates/alphasec-rs)
[![docs.rs](https://img.shields.io/docsrs/alphasec-rs)](https://docs.rs/alphasec-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A single `Agent` is the entry point. It covers spot trading, transfers, sessions, queries, and
WebSocket subscriptions; perpetuals are reached through `agent.perp()`, which returns a `PerpAgent`.
Transaction signing is handled internally, so callers never deal with signatures directly.

## 🔗 Links

- [Official](https://alphasec.trade)
- [Telegram](https://t.me/alphasecofficial)
- [Discord](https://discord.gg/alphasec)
- [X](https://x.com/AlphaSec_Trade)

## 🌐 Network Information

### Kairos Testnet

- **API URL**: `https://api-testnet.alphasec.trade`
- **WebSocket URL**: `wss://api-testnet.alphasec.trade/ws`
- **Network**: `kairos`
- **L1 Chain ID**: 1001 (Kaia Kairos)
- **L2 Chain ID**: 41001 (AlphaSec L2)

### Mainnet

- **API URL**: `https://api.alphasec.trade`
- **WebSocket URL**: `wss://api.alphasec.trade/ws`
- **Network**: `mainnet`
- **L1 Chain ID**: 8217 (Kaia Mainnet)
- **L2 Chain ID**: 48217 (AlphaSec L2)

## 📦 Installation

```toml
[dependencies]
alphasec-rs = "0.1"
```

WebSocket support is enabled by default. To opt out:

```toml
[dependencies]
alphasec-rs = { version = "0.1", default-features = false }
```

## 🚀 Quickstart

Build a `Config`, pass it to `Agent::new`. Token metadata is fetched at construction, so a network
connection is required. Call `start()` only when you need WebSocket streaming.

```rust
use alphasec_rs::{Agent, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new(
        "https://api-testnet.alphasec.trade", // API base URL (the ws URL is derived from it)
        "kairos",                             // network: "kairos" | "mainnet"
        "0xYourL1Address",                    // L1 address (derived from the key if one is given)
        Some("l1_private_key_hex"),           // L1 private key (no 0x prefix), optional
        None,                                 // L2 private key (used in session mode)
        false,                                // session_enabled
        None,                                 // chain_id override (None = network default)
    )?;

    let mut agent = Agent::new(config).await?;
    agent.start().await?; // only required for WebSocket streaming
    Ok(())
}
```

- `Config::new` derives the WebSocket URL from the API URL; `network` is `"kairos"` or `"mainnet"`.
- Signing wallet: the L2 wallet when `session_enabled` is on, otherwise the L1 wallet. See [Sessions](#sessions).
- Perp trading and queries are REST-only, so `agent.start()` is not needed for them.
- Inspect state with `l1_address()` and `is_session_enabled()`.

## ❗ Error Handling

Every method returns `Result<T, AlphaSecError>`.

| Variant                                   | Meaning                                                                                   |
| ----------------------------------------- | ----------------------------------------------------------------------------------------- |
| `Api { code, message }`                   | Server rejected the request; the server's code/message are passed through verbatim.       |
| `Network`, `Http`, `WebSocket`            | Transport-layer failures (candidates for retry).                                          |
| `InvalidParameter`                        | Caught by the SDK before sending (negative price/qty, unknown symbol, bad market format). |
| `Config`, `NotFound`, `Auth`, `Signer`, … | See [`src/error.rs`](src/error.rs).                                                       |

## Spot

Markets are written `"BASE/QUOTE"`; prices and quantities are `Decimal`. `order`, `cancel`,
`cancel_all`, and `modify` use the trade WebSocket when it is connected and fall back to REST
otherwise; `stop_order` is always REST.

```rust
use alphasec_rs::{OrderSide, OrderType, OrderMode};
use rust_decimal::Decimal;

// Limit buy 5 KAIA @ 1 USDT. Returns the order_id (tx hash).
let order_id = agent.order(
    "KAIA/USDT", OrderSide::Buy,
    "1".parse::<Decimal>()?,  // price
    "5".parse::<Decimal>()?,  // quantity
    OrderType::Limit, OrderMode::Base,
    None, None, None,         // tp_limit, sl_trigger, sl_limit
    None,                     // timestamp_ms (None = now)
).await?;

agent.cancel(&order_id, None).await?;
```

### Trading

| Method       | Description                                                                                                                  |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| `order`      | Submit an order. `OrderType` = Limit/Market, `OrderMode` = Base qty / Quote amount; optional TP-limit, SL-trigger, SL-limit. |
| `cancel`     | Cancel one order by id.                                                                                                      |
| `cancel_all` | Cancel every open order (account-wide).                                                                                      |
| `modify`     | Amend the price/quantity of an open order.                                                                                   |
| `stop_order` | Stop order that fires at a trigger price (always REST).                                                                      |

### Transfers & Deposits

| Method            | Description                                                                                                   |
| ----------------- | ------------------------------------------------------------------------------------------------------------- |
| `native_transfer` | Send native KAIA to an address.                                                                               |
| `token_transfer`  | Send a token to an address.                                                                                   |
| `deposit_token`   | Deposit from L1 into the exchange. Sends an L1 tx and **waits for the receipt**, then returns the L1 tx hash. |
| `withdraw_token`  | Withdraw from the exchange to L1. Signs with the L1 wallet, submits via the exchange API.                     |

L1 deposit/withdraw always needs the L1 wallet, regardless of session mode.

### Sessions

A session registers an L2 key for trade signing without exposing the L1 key.

| Method           | Description                                                           |
| ---------------- | --------------------------------------------------------------------- |
| `create_session` | Register a session (uses the `Config` L2 wallet if none is supplied). |
| `update_session` | Renew a session.                                                      |
| `delete_session` | Remove a session.                                                     |
| `get_sessions`   | List the sessions for an address.                                     |

### Queries

| Group   | Methods                                                                                 |
| ------- | --------------------------------------------------------------------------------------- |
| Market  | `get_market_list`, `get_ticker`, `get_tickers`, `get_depth`, `get_trades`, `get_tokens` |
| Orders  | `get_open_orders`, `get_filled_canceled_orders`, `get_order_by_id`                      |
| Account | `get_balance`, `get_transfer_history`                                                   |

### WebSocket

Subscribe with `agent.subscribe(channel)` and consume via `take_message_receiver()`.

| Channel               | Content                                  |
| --------------------- | ---------------------------------------- |
| `ticker@{market}`     | Ticker                                   |
| `trade@{market}`      | Trades                                   |
| `depth@{market}`      | Order book                               |
| `userEvent@{address}` | Account events (shared by spot and perp) |

```rust
use alphasec_rs::types::WebSocketMessage;

agent.start().await?;
let mut rx = agent.take_message_receiver().await.expect("receiver taken once");

let sub_ticker = agent.subscribe("ticker@KAIA/USDT").await?;
let sub_trade = agent.subscribe("trade@KAIA/USDT").await?;

while let Some(msg) = rx.recv().await {
    match msg {
        WebSocketMessage::TickerMsg { params, .. } => println!("📈 {} entries", params.result.len()),
        WebSocketMessage::TradeMsg { params, .. } => {
            for t in &params.result {
                println!("💱 {} price={} qty={}", t.market_id, t.price, t.quantity);
            }
        }
        WebSocketMessage::Disconnected => break, // no auto-reconnect; recreate the Agent
        _ => {}
    }
}

agent.unsubscribe(sub_ticker).await?;
agent.unsubscribe(sub_trade).await?;
agent.stop().await;
```

## Perp

The entry point is `agent.perp()`. Trading and market methods take a `symbol` and resolve it to a
numeric `market_id` internally (markets are fetched once and cached). All perp trading is REST.

```rust
use alphasec_rs::OrderSide;
use alphasec_rs::perp::TimeInForce;
use rust_decimal::Decimal;

let agent = Agent::new(config).await?; // REST only, start() not required

// Limit buy 0.01 BTC @ 60000 USDT (below market, so it rests). Returns the order_id (tx hash).
let order_id = agent.perp().order(
    "BTCUSDT", OrderSide::Buy,
    "60000".parse::<Decimal>()?, // price
    "0.01".parse::<Decimal>()?,  // quantity
    TimeInForce::Gtc, false, None, // reduce_only, client_order_id
).await?;

agent.perp().cancel("BTCUSDT", &order_id).await?;
let positions = agent.perp().get_positions().await?;
```

### Trading

| Method       | Description                                                                                      |
| ------------ | ------------------------------------------------------------------------------------------------ |
| `order`      | Submit an order. `TimeInForce` = Gtc/Ioc/Post/Market; supports `reduce_only`, `client_order_id`. |
| `cancel`     | Cancel one order by id.                                                                          |
| `cancel_all` | Cancel all open orders for a symbol (market-scoped, unlike spot).                                |
| `modify`     | Amend an order (cancel-and-replace).                                                             |

### Funds & Leverage

| Method         | Description                                                                                                                  |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `transfer`     | Move margin between the spot and perp wallets (`TransferDirection`; USDT only). Internal to the exchange, does not touch L1. |
| `set_leverage` | Set leverage per symbol.                                                                                                     |

### Queries

| Group   | Methods                                                                                        |
| ------- | ---------------------------------------------------------------------------------------------- |
| Account | `get_account`, `get_positions`, `get_position_history`, `get_position_settings`, `get_funding` |
| Orders  | `get_open_orders`, `get_order_history`, `get_order`, `get_order_list`, `get_my_trades`         |
| Market  | `get_markets`, `get_ticker`, `get_tickers`, `get_depth`, `get_market_trades`, `get_candles`    |

### WebSocket

Perp streams share the same connection as spot; trade submission is always REST. Channels use the
numeric `market_id` (resolve it with `get_markets()`). Decode frames with
`alphasec_rs::perp::ws::decode_perp_event` into a `PerpEvent`.

| Channel                         | Content                                                        |
| ------------------------------- | -------------------------------------------------------------- |
| `perp_ticker@{market_id}`       | Ticker                                                         |
| `perp_markPrice@{market_id}`    | Mark price                                                     |
| `perp_aggTrade@{market_id}`     | Aggregated trades                                              |
| `perp_aggDepth@{market_id}`     | Order book                                                     |
| `perp_candle@{market_id}:{res}` | Candles                                                        |
| `userEvent@{address}`           | User events (orders, positions, funding, deposits/withdrawals) |

## 📋 Examples

### Spot

| Example                                                    | Description                                             |
| ---------------------------------------------------------- | ------------------------------------------------------- |
| [`basic_market_data`](examples/basic_market_data.rs)       | Markets, tickers, tokens, and recent trades in one pass |
| [`get_market_data`](examples/get_market_data.rs)           | Market list                                             |
| [`get_tickers`](examples/get_tickers.rs)                   | Tickers                                                 |
| [`get_tokens`](examples/get_tokens.rs)                     | Token list                                              |
| [`get_balance`](examples/get_balance.rs)                   | Account balance                                         |
| [`account_info`](examples/account_info.rs)                 | Balance, sessions, open orders                          |
| [`get_transfer_history`](examples/get_transfer_history.rs) | Transfer history                                        |
| [`place_buy_order`](examples/place_buy_order.rs)           | A single buy order                                      |
| [`place_sell_order`](examples/place_sell_order.rs)         | A single sell order                                     |
| [`cancel_all_orders`](examples/cancel_all_orders.rs)       | Cancel all open orders                                  |
| [`simple_trading`](examples/simple_trading.rs)             | Buy → sell → cancel flow                                |
| [`transfer_kaia`](examples/transfer_kaia.rs)               | Native KAIA transfer                                    |
| [`transfer_usdt`](examples/transfer_usdt.rs)               | USDT token transfer                                     |
| [`transfer_tokens`](examples/transfer_tokens.rs)           | Native and token transfer together                      |
| [`deposit_withdraw`](examples/deposit_withdraw.rs)         | L1 deposit, then withdraw                               |
| [`session`](examples/session.rs)                           | Session create / update / delete / list                 |
| [`websocket`](examples/websocket.rs)                       | Multi-channel subscription and receive loop             |

### Perp

| Example                                        | Description                                         |
| ---------------------------------------------- | --------------------------------------------------- |
| [`perp_order`](examples/perp_order.rs)         | Place a perp order, then cancel it                  |
| [`perp_query`](examples/perp_query.rs)         | Perp account, positions, funding history            |
| [`perp_websocket`](examples/perp_websocket.rs) | Subscribe to mark price and decode into `PerpEvent` |

## 🔐 Security

**Important Security Notes:**

- Never commit private keys to version control
- Use environment variables or secure key management
- Always test on the Kairos testnet before mainnet
- Verify transaction details before signing
- Blockchain transactions are irreversible

## ⚠️ Disclaimer

This SDK is provided as-is. Trading cryptocurrencies involves substantial risk and may result in significant losses. Always do your own research and never invest more than you can afford to lose. The developers are not responsible for any trading losses incurred while using this SDK.
