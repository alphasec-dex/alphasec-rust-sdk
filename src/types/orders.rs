//! Order-related types for AlphaSec API

use serde::{Deserialize, Serialize};

/// Order side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    /// Buy order (side = 0 in API)
    Buy = 0,
    /// Sell order (side = 1 in API)
    Sell = 1,
}

impl std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "buy"),
            OrderSide::Sell => write!(f, "sell"),
        }
    }
}

impl From<u32> for OrderSide {
    fn from(value: u32) -> Self {
        match value {
            0 => OrderSide::Buy,
            1 => OrderSide::Sell,
            _ => OrderSide::Buy, // Default to buy
        }
    }
}

impl From<OrderSide> for u32 {
    fn from(side: OrderSide) -> Self {
        side as u32
    }
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    /// Limit order (order_type = 0 in API)
    Limit = 0,
    /// Market order (order_type = 1 in API)
    Market = 1,
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::Limit => write!(f, "limit"),
            OrderType::Market => write!(f, "market"),
        }
    }
}

impl From<u32> for OrderType {
    fn from(value: u32) -> Self {
        match value {
            0 => OrderType::Limit,
            1 => OrderType::Market,
            _ => OrderType::Limit, // Default to limit
        }
    }
}

impl From<OrderType> for u32 {
    fn from(order_type: OrderType) -> Self {
        order_type as u32
    }
}

/// Order mode (base=0, quote=1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderMode {
    /// Base token mode (order_mode = 0 in API)
    Base = 0,
    /// Quote token mode (order_mode = 1 in API)
    Quote = 1,
}

impl std::fmt::Display for OrderMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderMode::Base => write!(f, "base"),
            OrderMode::Quote => write!(f, "quote"),
        }
    }
}

impl From<u32> for OrderMode {
    fn from(value: u32) -> Self {
        match value {
            0 => OrderMode::Base,
            1 => OrderMode::Quote,
            _ => OrderMode::Base, // Default to base
        }
    }
}

impl From<OrderMode> for u32 {
    fn from(order_mode: OrderMode) -> Self {
        order_mode as u32
    }
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderStatus {
    /// Order is new and waiting to be processed
    New,
    /// Order is partially filled
    PartiallyFilled,
    /// Order is completely filled
    Filled,
    /// Order was canceled
    Canceled,
    /// Order was rejected
    Rejected,
    /// Order expired
    Expired,
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::New => write!(f, "NEW"),
            OrderStatus::PartiallyFilled => write!(f, "PARTIALLY_FILLED"),
            OrderStatus::Filled => write!(f, "FILLED"),
            OrderStatus::Canceled => write!(f, "CANCELED"),
            OrderStatus::Rejected => write!(f, "REJECTED"),
            OrderStatus::Expired => write!(f, "EXPIRED"),
        }
    }
}

/// Order information from API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    /// Database ID
    pub id: u64,
    /// Order ID (transaction hash)
    pub order_id: String,
    /// Account address
    pub account_address: String,
    /// Market ID (e.g., "5_2")
    pub market_id: String,
    /// Order side (BUY/SELL)
    pub side: String,
    /// Order type (LIMIT/MARKET)
    pub order_type: String,
    /// Order price as string
    pub price: String,
    /// Original quantity as string
    pub orig_qty: String,
    /// Original quote order quantity as string
    pub orig_quote_order_qty: String,
    /// Is trigger order
    pub is_trigger: bool,
    /// Is triggered
    pub is_triggered: bool,
    /// Trigger price as string
    pub trigger_price: String,
    /// Order status (NEW/FILLED/CANCELED/etc.)
    pub status: String,
    /// Contingency type (NONE/OCO/etc.)
    pub contingency_type: String,
    /// OTO leg type (NONE/etc.)
    pub oto_leg_type: String,
    /// Transaction hash
    pub tx_hash: String,
    /// Creation timestamp (milliseconds)
    pub created_at: u64,
    /// Update timestamp (milliseconds)
    pub updated_at: u64,
    /// Executed quantity as string
    pub executed_qty: String,
    /// Executed quote quantity as string
    pub executed_quote_qty: String,
}

impl Order {
    /// Get order side as OrderSide enum
    pub fn side_enum(&self) -> Result<OrderSide, String> {
        match self.side.as_str() {
            "BUY" => Ok(OrderSide::Buy),
            "SELL" => Ok(OrderSide::Sell),
            _ => Err(format!("Unknown order side: {}", self.side))
        }
    }
    
    /// Get order type as OrderType enum
    pub fn order_type_enum(&self) -> Result<OrderType, String> {
        match self.order_type.as_str() {
            "LIMIT" => Ok(OrderType::Limit),
            "MARKET" => Ok(OrderType::Market),
            _ => Err(format!("Unknown order type: {}", self.order_type))
        }
    }
    
    /// Parse price as Decimal
    pub fn price_decimal(&self) -> Result<rust_decimal::Decimal, rust_decimal::Error> {
        use std::str::FromStr;
        rust_decimal::Decimal::from_str(&self.price)
    }
    
    /// Parse original quantity as Decimal
    pub fn orig_qty_decimal(&self) -> Result<rust_decimal::Decimal, rust_decimal::Error> {
        use std::str::FromStr;
        rust_decimal::Decimal::from_str(&self.orig_qty)
    }
    
    /// Parse executed quantity as Decimal
    pub fn executed_qty_decimal(&self) -> Result<rust_decimal::Decimal, rust_decimal::Error> {
        use std::str::FromStr;
        rust_decimal::Decimal::from_str(&self.executed_qty)
    }
    
    /// Convert creation timestamp to DateTime<Utc>
    pub fn created_at_datetime(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        use chrono::{TimeZone, Utc};
        Utc.timestamp_millis_opt(self.created_at as i64).single()
    }
    
    /// Convert update timestamp to DateTime<Utc>
    pub fn updated_at_datetime(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        use chrono::{TimeZone, Utc};
        Utc.timestamp_millis_opt(self.updated_at as i64).single()
    }
    
    /// Check if order is filled
    pub fn is_filled(&self) -> bool {
        self.status == "FILLED"
    }
    
    /// Check if order is canceled
    pub fn is_canceled(&self) -> bool {
        self.status == "CANCELED"
    }
    
    /// Check if order is active (NEW or PARTIALLY_FILLED)
    pub fn is_active(&self) -> bool {
        matches!(self.status.as_str(), "NEW" | "PARTIALLY_FILLED")
    }
}

/// Query parameters for listing orders
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrdersQuery {
    /// User address
    pub address: String,
    /// Market ID or symbol (optional)
    pub market: Option<String>,
    /// Maximum number of results
    pub limit: Option<u32>,
    /// Start time (milliseconds since epoch)
    pub from_msec: Option<i64>,
    /// End time (milliseconds since epoch)
    pub end_msec: Option<i64>,
}

impl OrdersQuery {
    /// Create a new orders query
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            market: None,
            limit: None,
            from_msec: None,
            end_msec: None,
        }
    }

    /// Set market filter
    pub fn market(mut self, market: impl Into<String>) -> Self {
        self.market = Some(market.into());
        self
    }

    /// Set limit
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set time range using millisecond timestamps
    pub fn time_range(mut self, from_msec: i64, to_msec: i64) -> Self {
        self.from_msec = Some(from_msec);
        self.end_msec = Some(to_msec);
        self
    }
    
    /// Set time range using DateTime (convenience method)
    pub fn time_range_datetime(mut self, from: chrono::DateTime<chrono::Utc>, to: chrono::DateTime<chrono::Utc>) -> Self {
        self.from_msec = Some(from.timestamp_millis());
        self.end_msec = Some(to.timestamp_millis());
        self
    }
}
