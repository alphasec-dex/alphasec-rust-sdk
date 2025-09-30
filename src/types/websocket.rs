//! WebSocket message types for AlphaSec (JSON-RPC 2.0 format)

use serde::{Deserialize, Serialize};

/// WebSocket message from AlphaSec (JSON-RPC 2.0 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WebSocketMessage {
    /// Subscription acknowledgment
    Ack {
        /// Request ID
        id: i32,
        /// Result (usually "success")
        result: String,
    },
    /// Trade message
    TradeMsg {
        /// Method (always "subscription")
        method: String,
        /// Trade parameters
        params: TradeParams,
    },
    /// Depth (orderbook) message
    DepthMsg {
        /// Method (always "subscription")
        method: String,
        /// Depth parameters
        params: DepthParams,
    },
    /// Ticker message
    TickerMsg {
        /// Method (always "subscription")
        method: String,
        /// Ticker parameters
        params: TickerParams,
    },
    /// User event message
    UserEventMsg {
        /// Method (always "subscription")
        method: String,
        /// User event parameters
        params: UserEventParams,
    },
    /// Generic message (fallback for any other format)
    Generic(serde_json::Value),
}

/// Trade parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeParams {
    /// Channel name
    pub channel: String,
    /// Trade result
    pub result: Vec<TradeResult>,
}

/// Individual trade result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResult {
    /// Unique trade ID
    #[serde(rename = "tradeId")]
    pub trade_id: String,
    /// Market ID
    #[serde(rename = "marketId")]
    pub market_id: String,
    /// Trade price
    pub price: String,
    /// Trade quantity
    #[serde(rename = "quantity")]
    pub quantity: String,
    /// Buy order ID
    #[serde(rename = "buyOrderId")]
    pub buy_order_id: String,
    /// Sell order ID
    #[serde(rename = "sellOrderId")]
    pub sell_order_id: String,
    /// Created at timestamp
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    /// Is buyer maker
    #[serde(rename = "isBuyerMaker")]
    pub is_buyer_maker: bool,
}

/// Depth parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthParams {
    /// Channel name
    pub channel: String,
    /// Depth result
    pub result: DepthResult,
}

/// Orderbook depth result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthResult {
    /// Market ID
    #[serde(rename = "marketId")]
    pub market_id: String,
    /// Bids [[price, size], ...]
    pub bids: Option<Vec<Vec<String>>>,
    /// Asks [[price, size], ...]
    pub asks: Option<Vec<Vec<String>>>,
    /// First ID
    #[serde(rename = "firstId")]
    pub first_id: i64,
    /// Final ID
    #[serde(rename = "finalId")]
    pub final_id: i64,
    /// Timestamp
    pub time: i64,
}

/// Ticker parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerParams {
    /// Channel name
    pub channel: String,
    /// Ticker result (array of entries)
    pub result: Vec<TickerEntry>,
}

/// Individual ticker entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerEntry {
    /// Market ID
    #[serde(rename = "marketId")]
    pub market_id: String,
    /// Base token ID
    #[serde(rename = "baseTokenId")]
    pub base_token_id: String,
    /// Quote token ID
    #[serde(rename = "quoteTokenId")]
    pub quote_token_id: String,
    /// Current price
    pub price: String,
    /// 24h open price
    #[serde(rename = "open24h")]
    pub open_24h: String,
    /// 24h high price
    #[serde(rename = "high24h")]
    pub high_24h: String,
    /// 24h low price
    #[serde(rename = "low24h")]
    pub low_24h: String,
    /// 24h volume
    #[serde(rename = "volume24h")]
    pub volume_24h: String,
    /// 24h quote volume
    #[serde(rename = "quoteVolume24h")]
    pub quote_volume_24h: String,
}

/// User event parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEventParams {
    /// Channel name
    pub channel: String,
    /// User event result
    pub result: UserEventResult,
}

/// User event result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEventResult {
    /// Event type
    #[serde(rename = "eventType")]
    pub event_type: String, // "NEW", "TRADE", "CANCELED", etc.
    /// Event time
    #[serde(rename = "eventTime")]
    pub event_time: i64,
    /// Account address
    #[serde(rename = "accountAddress")]
    pub account_address: String,
    /// Account ID (optional, for backward compatibility)
    #[serde(rename = "accountId")]
    pub account_id: Option<i64>,
    /// Order ID
    #[serde(rename = "orderId")]
    pub order_id: String,
    /// Transaction hash
    #[serde(rename = "txHash")]
    pub tx_hash: String,
    /// Market ID
    #[serde(rename = "marketId")]
    pub market_id: String,
    /// Order side
    pub side: String, // "BUY", "SELL"
    /// Order type
    #[serde(rename = "orderType")]
    pub order_type: String, // "LIMIT", "MARKET"
    /// Order mode
    #[serde(rename = "orderMode")]
    pub order_mode: i32,
    /// Original price
    #[serde(rename = "origPrice")]
    pub orig_price: String,
    /// Original quantity
    #[serde(rename = "origQty")]
    pub orig_qty: String,
    /// Original quote order quantity
    #[serde(rename = "origQuoteOrderQty")]
    pub orig_quote_order_qty: String,
    /// Order status
    pub status: String, // "NEW", "PARTIALLY_FILLED", "FILLED", etc.
    /// Created at timestamp
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    /// Executed quantity
    #[serde(rename = "executedQty")]
    pub executed_qty: String,
    /// Executed quote quantity
    #[serde(rename = "executedQuoteQty")]
    pub executed_quote_qty: String,
    /// Last price
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    /// Last quantity
    #[serde(rename = "lastQty")]
    pub last_qty: String,
    /// Fee
    pub fee: String,
    /// Fee token ID
    #[serde(rename = "feeTokenId")]
    pub fee_token_id: Option<String>,
    /// Trade ID
    #[serde(rename = "tradeId")]
    pub trade_id: String,
    /// Is maker
    #[serde(rename = "isMaker")]
    pub is_maker: bool,
}

/// WebSocket subscription request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionRequest {
    /// Channels to subscribe to
    pub channels: Vec<String>,
}