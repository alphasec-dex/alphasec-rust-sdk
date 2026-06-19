//! Perp WebSocket event types and channel-based decoder.
//!
//! `WsManager` delivers perp messages as `WebSocketMessage::Generic(Value)` (the same
//! variant used for any unrecognised channel).  Callers pass the channel string and the
//! `result` payload to `decode_perp_event` to obtain a typed `PerpEvent`.
//!
//! Channel routing:
//!   perp_ticker[...] → PerpEvent::Ticker
//!   perp_markPrice@{id} → PerpEvent::MarkPrice
//!   perp_aggTrade@{id} → PerpEvent::AggTrade
//!   perp_aggDepth@{id} → PerpEvent::AggDepth
//!   perp_candle@{id}:{res} → PerpEvent::Candle
//!   userEvent[...] → second-level dispatch on "topic" field

use crate::error::{AlphaSecError, Result};
use crate::perp::types::{PerpTicker, PerpTrade};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Additional WS-specific payload structs
// ---------------------------------------------------------------------------

/// Mark-price stream payload (`perp_markPrice@{marketId}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpMarkPrice {
    /// Market ID
    pub market_id: String,
    /// Current mark price
    #[serde(with = "rust_decimal::serde::str")]
    pub mark_price: Decimal,
    /// Current index price
    #[serde(with = "rust_decimal::serde::str")]
    pub index_price: Decimal,
    /// Latest settled funding rate (signed)
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_rate: Decimal,
    /// Next funding settlement timestamp (ms)
    pub next_funding_time: i64,
    /// Milliseconds until next funding (0-clamped)
    pub funding_remaining_time: i64,
    /// Cumulative funding index (18 decimals, signed)
    #[serde(with = "rust_decimal::serde::str")]
    pub cumulative_funding_index: Decimal,
    /// Funding interval in seconds
    pub funding_interval_sec: u64,
    /// Predicted next-boundary funding rate (signed)
    #[serde(with = "rust_decimal::serde::str")]
    pub predicted_funding_rate: Decimal,
    /// Server timestamp (ms)
    pub timestamp: i64,
}

/// Order-book depth snapshot from the `perp_aggDepth@{marketId}` stream.
///
/// Distinct from the REST `PerpDepth` (`/market/depth`): the WS frame carries `marketId`
/// plus `firstId`/`finalId` aggregation-window bounds and has NO `lastUpdatedId`, so it
/// cannot reuse `PerpDepth` (whose required `lastUpdatedId` would fail to deserialize).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpAggDepth {
    /// Market ID
    pub market_id: String,
    /// Bid price/quantity string pairs (descending price)
    pub bids: Vec<[String; 2]>,
    /// Ask price/quantity string pairs (ascending price)
    pub asks: Vec<[String; 2]>,
    /// First sequence ID in this aggregation window
    pub first_id: i64,
    /// Last sequence ID in this aggregation window
    pub final_id: i64,
    /// Snapshot timestamp (ms epoch). Server restored this key to `time` (was `updatedAt`).
    pub time: i64,
}

/// Candle stream update payload (`perp_candle@{marketId}:{resolution}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandleUpdate {
    /// Market ID. Server restored this key to snake_case `market_id` (was `marketId`).
    #[serde(rename = "market_id")]
    pub market_id: String,
    /// Resolution code (e.g. "60", "1D")
    pub resolution: String,
    /// Candle open time (Unix seconds)
    pub time: i64,
    /// Open price
    pub open: f64,
    /// High price
    pub high: f64,
    /// Low price
    pub low: f64,
    /// Close price
    pub close: f64,
    /// Base volume
    pub volume: f64,
    /// True if the candle is finalised
    pub is_closed: bool,
}

// ---------------------------------------------------------------------------
// User-event payloads
// ---------------------------------------------------------------------------

/// Perp order user-event payload (topic = PERP_ORDER).
///
/// Reuses the generic UserEvent fields from the server's unified envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpOrderEvent {
    /// NEW / TRADE / CANCEL / REJECTED
    pub event_type: String,
    /// Event timestamp (ms)
    pub event_time: i64,
    /// Block number
    pub block_number: i64,
    /// Account address
    pub account_address: String,
    /// Transaction hash
    pub tx_hash: String,
    /// Order ID
    #[serde(default)]
    pub order_id: String,
    /// Market ID
    #[serde(default)]
    pub market_id: String,
    /// BUY or SELL
    #[serde(default)]
    pub side: String,
    /// Original order price
    #[serde(default)]
    pub orig_price: String,
    /// Original order quantity
    #[serde(default)]
    pub orig_qty: String,
    /// Order status
    #[serde(default)]
    pub status: String,
    /// Order creation timestamp (ms)
    #[serde(default)]
    pub created_at: i64,
    /// Cumulative executed quantity
    #[serde(default)]
    pub executed_qty: String,
    /// Cumulative executed quote amount
    #[serde(default)]
    pub executed_quote_qty: String,
    /// This-event execution price
    #[serde(default)]
    pub last_price: String,
    /// This-event execution quantity
    #[serde(default)]
    pub last_qty: String,
    /// Fee amount
    #[serde(default)]
    pub fee: String,
    /// Fee token (USDT for perp)
    #[serde(default)]
    pub fee_token_id: String,
    /// Trade ID (present on TRADE events)
    #[serde(default)]
    pub trade_id: String,
    /// Maker flag (present on TRADE events)
    #[serde(default)]
    pub is_maker: bool,
}

/// Perp position user-event payload (topic = PERP_POSITION).
///
/// Generic fields are reused with position-specific semantics documented in the
/// api-reference (size_before=origQty, size_after=executedQty, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpPositionEvent {
    /// POSITION_OPEN / POSITION_INCREASE / POSITION_REDUCE / POSITION_CLOSE /
    /// POSITION_FLIP / POSITION_LIQUIDATION / POSITION_ADL
    pub event_type: String,
    /// Event timestamp (ms)
    pub event_time: i64,
    /// Block number
    pub block_number: i64,
    /// Account address
    pub account_address: String,
    /// Transaction hash
    pub tx_hash: String,
    /// Market ID
    #[serde(default)]
    pub market_id: String,
    /// size_before (signed decimal string)
    #[serde(default)]
    pub orig_qty: String,
    /// size_after (signed decimal string)
    #[serde(default)]
    pub executed_qty: String,
    /// size_delta (signed decimal string)
    #[serde(default)]
    pub orig_quote_order_qty: String,
    /// Fill price for this event
    #[serde(default)]
    pub last_price: String,
    /// Fill quantity for this event
    #[serde(default)]
    pub last_qty: String,
    /// Realized PnL for this event (trading PnL only, funding excluded)
    #[serde(default)]
    pub fee: String,
    /// Source order ID
    #[serde(default)]
    pub order_id: String,
    /// Source trade ID
    #[serde(default)]
    pub trade_id: String,
}

/// Perp funding user-event payload (topic = PERP_FUNDING).
///
/// Uses the same generic fields as PERP_POSITION; included as a distinct type
/// for clarity at the call site.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpFundingEvent {
    /// FUNDING_PAYMENT
    pub event_type: String,
    /// Event timestamp (ms)
    pub event_time: i64,
    /// Block number
    pub block_number: i64,
    /// Account address
    pub account_address: String,
    /// Transaction hash
    pub tx_hash: String,
    /// Market ID
    #[serde(default)]
    pub market_id: String,
    /// Funding payment amount (signed)
    #[serde(default)]
    pub fee: String,
}

/// Account-level user-event payload (topic = ACCOUNT, perp eventTypes).
///
/// Covers PERP_DEPOSIT / PERP_WITHDRAW / PERP_SET_LEVERAGE etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpAccountEvent {
    /// PERP_DEPOSIT / PERP_WITHDRAW / PERP_SET_LEVERAGE / PERP_SET_MARGIN_TYPE /
    /// PERP_UPDATE_ISOLATED_MARGIN / SETTINGS_CHANGE_REJECTED
    pub event_type: String,
    /// Event timestamp (ms)
    pub event_time: i64,
    /// Block number
    pub block_number: i64,
    /// Account address
    pub account_address: String,
    /// Transaction hash
    pub tx_hash: String,
    /// Token ID (deposit/withdraw)
    #[serde(default)]
    pub token_id: String,
    /// Amount (deposit/withdraw)
    #[serde(default)]
    pub amount: String,
}

// ---------------------------------------------------------------------------
// PerpEvent enum
// ---------------------------------------------------------------------------

/// Typed perp WebSocket event produced by `decode_perp_event`.
#[derive(Debug, Clone)]
pub enum PerpEvent {
    /// perp_ticker / perp_ticker@{id} — array of ticker snapshots
    Ticker(Vec<PerpTicker>),
    /// perp_markPrice@{id} — single mark-price update
    MarkPrice(PerpMarkPrice),
    /// perp_aggTrade@{id} — batch of recent trades
    AggTrade(Vec<PerpTrade>),
    /// perp_aggDepth@{id} — order-book depth snapshot
    AggDepth(PerpAggDepth),
    /// perp_candle@{id}:{res} — candle update
    Candle(CandleUpdate),
    /// userEvent — PERP_ORDER sub-topic
    UserOrder(PerpOrderEvent),
    /// userEvent — PERP_POSITION sub-topic
    UserPosition(PerpPositionEvent),
    /// userEvent — PERP_FUNDING sub-topic
    UserFunding(PerpFundingEvent),
    /// userEvent — ACCOUNT sub-topic (perp eventTypes only)
    UserAccount(PerpAccountEvent),
}

// ---------------------------------------------------------------------------
// Decoder
// ---------------------------------------------------------------------------

/// Decode a perp WebSocket message given the channel name and the `result` payload.
///
/// The `value` argument is the `params.result` field from the WS envelope.
/// `WebSocketMessage::Generic(Value)` callers should extract `params.result`
/// before calling this function, or pass the full params object — the function
/// only inspects the structure described in the api-reference.
///
/// Channel routing (prefix match):
/// - `perp_ticker`  → `PerpEvent::Ticker`
/// - `perp_markPrice` → `PerpEvent::MarkPrice`
/// - `perp_aggTrade` → `PerpEvent::AggTrade`
/// - `perp_aggDepth` → `PerpEvent::AggDepth`
/// - `perp_candle`  → `PerpEvent::Candle`
/// - `userEvent`    → dispatch on `topic` field (PERP_ORDER / PERP_POSITION /
///                    PERP_FUNDING / ACCOUNT)
pub fn decode_perp_event(channel: &str, value: &Value) -> Result<PerpEvent> {
    if channel.starts_with("perp_ticker") {
        let tickers: Vec<PerpTicker> =
            serde_json::from_value(value.clone()).map_err(AlphaSecError::Json)?;
        return Ok(PerpEvent::Ticker(tickers));
    }

    if channel.starts_with("perp_markPrice") {
        let mp: PerpMarkPrice =
            serde_json::from_value(value.clone()).map_err(AlphaSecError::Json)?;
        return Ok(PerpEvent::MarkPrice(mp));
    }

    if channel.starts_with("perp_aggTrade") {
        let trades: Vec<PerpTrade> =
            serde_json::from_value(value.clone()).map_err(AlphaSecError::Json)?;
        return Ok(PerpEvent::AggTrade(trades));
    }

    if channel.starts_with("perp_aggDepth") {
        let depth: PerpAggDepth =
            serde_json::from_value(value.clone()).map_err(AlphaSecError::Json)?;
        return Ok(PerpEvent::AggDepth(depth));
    }

    if channel.starts_with("perp_candle") {
        let candle: CandleUpdate =
            serde_json::from_value(value.clone()).map_err(AlphaSecError::Json)?;
        return Ok(PerpEvent::Candle(candle));
    }

    if channel.starts_with("userEvent") {
        return decode_user_event(value);
    }

    Err(AlphaSecError::invalid_parameter(format!(
        "decode_perp_event: unrecognised channel '{}'",
        channel
    )))
}

/// Dispatch a `userEvent` payload by its `topic` field.
fn decode_user_event(value: &Value) -> Result<PerpEvent> {
    let topic = value.get("topic").and_then(|v| v.as_str()).ok_or_else(|| {
        AlphaSecError::invalid_parameter("userEvent payload missing 'topic' field")
    })?;

    match topic {
        "PERP_ORDER" => {
            let ev: PerpOrderEvent =
                serde_json::from_value(value.clone()).map_err(AlphaSecError::Json)?;
            Ok(PerpEvent::UserOrder(ev))
        }
        "PERP_POSITION" => {
            let ev: PerpPositionEvent =
                serde_json::from_value(value.clone()).map_err(AlphaSecError::Json)?;
            Ok(PerpEvent::UserPosition(ev))
        }
        "PERP_FUNDING" => {
            let ev: PerpFundingEvent =
                serde_json::from_value(value.clone()).map_err(AlphaSecError::Json)?;
            Ok(PerpEvent::UserFunding(ev))
        }
        "ACCOUNT" => {
            let ev: PerpAccountEvent =
                serde_json::from_value(value.clone()).map_err(AlphaSecError::Json)?;
            Ok(PerpEvent::UserAccount(ev))
        }
        other => Err(AlphaSecError::invalid_parameter(format!(
            "decode_perp_event: unknown userEvent topic '{}'",
            other
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_perp_markprice_by_channel_name() {
        let raw = serde_json::json!({
            "marketId": "1",
            "markPrice": "91005",
            "indexPrice": "91002",
            "fundingRate": "0.0001",
            "nextFundingTime": 1777395600000i64,
            "fundingRemainingTime": 3600000i64,
            "cumulativeFundingIndex": "125000000000000000",
            "fundingIntervalSec": 3600u64,
            "predictedFundingRate": "0.00015",
            "timestamp": 1777392000123i64
        });

        let ev = decode_perp_event("perp_markPrice@1", &raw)
            .expect("decode_perp_event must not fail for valid perp_markPrice payload");

        assert!(
            matches!(ev, PerpEvent::MarkPrice(_)),
            "expected PerpEvent::MarkPrice, got a different variant"
        );

        if let PerpEvent::MarkPrice(mp) = ev {
            assert_eq!(mp.market_id, "1");
            assert_eq!(mp.mark_price.to_string(), "91005");
        }
    }

    #[test]
    fn decode_rejects_unknown_channel_and_bad_topic() {
        assert!(decode_perp_event("spot_trade@1", &serde_json::json!({})).is_err());
        assert!(decode_perp_event("", &serde_json::json!({})).is_err());
        assert!(
            decode_perp_event("userEvent@a", &serde_json::json!({"eventType": "NEW"})).is_err(),
            "userEvent without a topic must error"
        );
        assert!(
            decode_perp_event("userEvent@a", &serde_json::json!({"topic": "NONSENSE"})).is_err(),
            "userEvent with an unknown topic must error"
        );
    }
}
