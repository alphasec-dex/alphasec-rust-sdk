//! Perp domain types — response structs, enums, and query parameter structs.
//!
//! Decimal fields are deserialized from JSON strings (server sends quoted decimals).
//! Nullable/omitempty server fields are modeled as Option<_>.
//! All structs use #[serde(rename_all = "camelCase")].

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Deserialization helpers — tolerate server contract quirks observed on dev.
// (Server is the source of truth; these absorb string/null/empty variants the
// typed structs would otherwise reject.)
// ---------------------------------------------------------------------------

/// Deserialize a `u64` from either a JSON number or a quoted string.
/// `/market` sends `fundingInterval` as a string (e.g. `"28800"`).
fn de_u64_flex<'de, D>(d: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StrOrNum {
        N(u64),
        S(String),
    }
    match StrOrNum::deserialize(d)? {
        StrOrNum::N(n) => Ok(n),
        StrOrNum::S(s) => s.trim().parse().map_err(serde::de::Error::custom),
    }
}

/// Deserialize a `Vec<T>` treating JSON `null` (and absence) as an empty vec.
/// `/market` sends `tiers: null` when a market has no tier ladder.
fn de_vec_null<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(d)?.unwrap_or_default())
}

/// Decimal that accepts an empty string `""` as zero (keeps string serialization).
/// `/market/ticker` sends `cumulativeFundingIndex`/`predictedFundingRate` as `""`
/// for un-bootstrapped markets.
mod decimal_empty_zero {
    use super::Decimal;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Decimal, s: S) -> Result<S::Ok, S::Error> {
        rust_decimal::serde::str::serialize(v, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
        let s = String::deserialize(d)?;
        if s.trim().is_empty() {
            Ok(Decimal::ZERO)
        } else {
            s.parse().map_err(serde::de::Error::custom)
        }
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Transfer direction between Spot wallet and Perp wallet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDirection {
    /// Spot → Perp deposit (command 0x12)
    SpotToPerp,
    /// Perp → Spot withdrawal (command 0x44)
    PerpToSpot,
}

/// Perp order time-in-force policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good-till-cancelled (0)
    Gtc,
    /// Immediate-or-cancel (1)
    Ioc,
    /// Post-only (2)
    Post,
    /// Market order (3)
    Market,
}

/// Perpetual position side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    /// Long position
    Long,
    /// Short position
    Short,
}

/// Perp order status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerpOrderStatus {
    /// Accepted open order
    New,
    /// Partially filled (read-time derived)
    PartiallyFilled,
    /// Fully filled
    Filled,
    /// Cancelled
    Canceled,
    /// Expired (reserved)
    Expired,
    /// Rejected by system
    Rejected,
    /// Trigger tx accepted; transitioning to WaitingTrigger
    PendingTrigger,
    /// Waiting for trigger condition
    WaitingTrigger,
}

// ---------------------------------------------------------------------------
// Response structs
// ---------------------------------------------------------------------------

/// Maker/taker fee rate pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeRate {
    /// Maker fee rate
    #[serde(with = "rust_decimal::serde::str")]
    pub maker: Decimal,
    /// Taker fee rate
    #[serde(with = "rust_decimal::serde::str")]
    pub taker: Decimal,
}

/// Perp order — returned by /order, /order/open, /order/{id}, /order/list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpOrder {
    /// Internal sequence ID
    pub id: i64,
    /// Order ID (tx hash or derived hash for OTOCO children)
    pub order_id: String,
    /// Account address
    pub account_address: String,
    /// Market ID
    pub market_id: String,
    /// BUY or SELL
    pub side: String,
    /// Deprecated derived field (TRIGGER/MARKET/LIMIT)
    pub order_type: String,
    /// GTC / IOC / POST / MARKET
    pub time_in_force: String,
    /// Order price
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Trigger price (omitempty — only meaningful for trigger orders)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub trigger_price: Option<Decimal>,
    /// 0=SL, 1=TP (omitempty)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tpsl_type: Option<u8>,
    /// True if trigger fires as market order
    pub is_market_trigger: bool,
    /// True if trigger has already fired
    pub is_triggered: bool,
    /// True for 0x4B position-level TP/SL
    pub is_position_tpsl: bool,
    /// NONE / OTO / OCO
    pub contingency_type: String,
    /// NONE / WORKING_LEG / TP_LEG / SL_LEG
    pub oto_leg_type: String,
    /// Original order quantity
    #[serde(with = "rust_decimal::serde::str")]
    pub orig_qty: Decimal,
    /// Cumulative executed quantity
    #[serde(with = "rust_decimal::serde::str")]
    pub executed_qty: Decimal,
    /// Cumulative executed quote amount
    #[serde(with = "rust_decimal::serde::str")]
    pub executed_quote_qty: Decimal,
    /// Reduce-only flag
    pub is_reduce_only: bool,
    /// Post-only flag
    pub is_post_only: bool,
    /// Fill percentage (omitempty)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub filled_percentage: Option<Decimal>,
    /// Average execution price (omitempty — absent until at least one fill)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub average_price: Option<Decimal>,
    /// Client-supplied order ID (empty string if absent)
    pub client_order_id: String,
    /// Order status
    pub status: String,
    /// Submission tx hash
    pub tx_hash: String,
    /// Creation timestamp (ms epoch)
    pub created_at: i64,
    /// Last-updated timestamp (ms epoch)
    pub updated_at: i64,
}

/// Open perp position — returned by /position.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// Market ID
    pub market_id: String,
    /// LONG or SHORT
    pub side: String,
    /// OPEN / CLOSED / LIQUIDATED / ADL
    pub status: String,
    /// Position size (signed: LONG > 0, SHORT < 0)
    #[serde(with = "rust_decimal::serde::str")]
    pub size: Decimal,
    /// Entry price (VWAP of OPEN/INCREASE fills)
    #[serde(with = "rust_decimal::serde::str")]
    pub entry_price: Decimal,
    /// Liquidation price (raw core formula). `null` when over-collateralized, the formula is
    /// undefined (denominator <= 0), or the market's mark price is stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub liquidation_price: Option<Decimal>,
    /// User-set leverage (clamped by tier maxLeverage)
    pub leverage: u32,
    /// CROSS or ISOLATED
    pub margin_type: String,
    /// Position margin (ISOLATED: allocated margin; CROSS: notional × tierIMRate)
    #[serde(with = "rust_decimal::serde::str")]
    pub position_margin: Decimal,
    /// Per-position maintenance margin (`notional × tier MMR − tier maintenanceAmount`).
    /// `null` when the market's mark price is stale (notional collapses to 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub maint_margin: Option<Decimal>,
    /// Margin ratio. `null` when equity <= 0 or the risk inputs are stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub margin_ratio: Option<Decimal>,
    /// Return on equity (`unrealizedPnL × leverage / notional`, raw ratio).
    /// `null` when notional <= 0, leverage is unresolved, or the mark price is stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub roe: Option<Decimal>,
    /// Cumulative settled funding (signed: + = received, − = paid)
    #[serde(with = "rust_decimal::serde::str")]
    pub cumulative_funding: Decimal,
    /// Pending (unsettled) funding since last settlement (signed)
    #[serde(with = "rust_decimal::serde::str")]
    pub pending_funding: Decimal,
    /// ADL queue quantile (currently always 0)
    pub adl_quantile: u32,
    /// Position open timestamp (ms epoch)
    pub opened_at: i64,
    /// Last-updated timestamp (ms epoch)
    pub updated_at: i64,
}

/// Perp account balances and risk aggregates — returned by /wallet/account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpAccount {
    /// Sequencer-level perp wallet balance (signed; can be negative under insolvency)
    #[serde(with = "rust_decimal::serde::str")]
    pub wallet_balance: Decimal,
    /// walletBalance + crossUnrealizedProfit (CROSS equity)
    #[serde(with = "rust_decimal::serde::str")]
    pub margin_balance: Decimal,
    /// Available for new orders / withdrawal (signed)
    #[serde(with = "rust_decimal::serde::str")]
    pub available_balance: Decimal,
    /// CROSS positions unrealized PnL (trade PnL only, excludes funding)
    #[serde(with = "rust_decimal::serde::str")]
    pub cross_unrealized_profit: Decimal,
    /// CROSS positions effective IM sum
    #[serde(with = "rust_decimal::serde::str")]
    pub total_position_margin: Decimal,
    /// Margin locked by open orders
    #[serde(with = "rust_decimal::serde::str")]
    pub open_order_margin: Decimal,
    /// Spot wallet total (USDT-equivalent)
    #[serde(with = "rust_decimal::serde::str")]
    pub spot_total: Decimal,
    /// CROSS position notional value sum
    #[serde(with = "rust_decimal::serde::str")]
    pub position_value: Decimal,
    /// marginRatio = maintenanceMargin / marginBalance; null when marginBalance ≤ 0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub margin_ratio: Option<Decimal>,
    /// Account-level maintenance margin (CROSS)
    #[serde(with = "rust_decimal::serde::str")]
    pub maintenance_margin: Decimal,
    /// positionValue / marginBalance; null when marginBalance ≤ 0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub account_leverage: Option<Decimal>,
    /// Maker/taker fee rates (Tier-0 fixed for now)
    pub fee_rate: FeeRate,
}

/// Position lifecycle history row — returned by /position/history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionHistory {
    /// Market ID
    pub market_id: String,
    /// LONG or SHORT
    pub side: String,
    /// OPEN / CLOSED / LIQUIDATED / ADL
    pub status: String,
    /// Max absolute position size during lifecycle
    #[serde(with = "rust_decimal::serde::str")]
    pub max_size: Decimal,
    /// Entry VWAP
    #[serde(with = "rust_decimal::serde::str")]
    pub entry_price: Decimal,
    /// Cumulative close-side fill quantity
    #[serde(with = "rust_decimal::serde::str")]
    pub closed_size: Decimal,
    /// Average close price (omitempty — absent if never closed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub avg_close_price: Option<Decimal>,
    /// Lifetime realized PnL (trade PnL only, signed)
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    /// Lifetime cumulative funding (signed)
    #[serde(with = "rust_decimal::serde::str")]
    pub cumulative_funding: Decimal,
    /// Lifecycle open timestamp (ms epoch)
    pub opened_at: i64,
    /// Lifecycle close timestamp (ms epoch; omitempty — absent if still open)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<i64>,
    /// Last-updated timestamp (ms epoch)
    pub updated_at: i64,
}

/// Per-market leverage/margin-mode setting — returned by /position/settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionSetting {
    /// Market ID
    pub market_id: String,
    /// Configured leverage (0 = never explicitly set; FE shows market max)
    pub leverage: u64,
    /// CROSS or ISOLATED
    pub margin_mode: String,
}

/// Funding history row — returned by /wallet/funding.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingItem {
    /// Packed keyset cursor (boundary_time_sec × 100000 + marketId)
    pub id: i64,
    /// Account address
    pub address: String,
    /// Market ID
    pub market_id: String,
    /// Market symbol (empty string if metadata unavailable)
    pub symbol: String,
    /// Funding rate for this boundary (signed)
    #[serde(with = "rust_decimal::serde::str")]
    pub rate: Decimal,
    /// Net payment for this boundary (signed: + received, − paid)
    #[serde(with = "rust_decimal::serde::str")]
    pub payment: Decimal,
    /// Position size used for this funding calculation (absolute value)
    #[serde(with = "rust_decimal::serde::str")]
    pub size_at_event: Decimal,
    /// True = already reflected in cumulativeFunding; false = pending tail
    pub realized: bool,
    /// Boundary timestamp (ms epoch)
    pub time: i64,
}

/// Fill / trade row — returned by /order/trade.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpFill {
    /// Internal fill ID (use as lastID for pagination)
    pub id: i64,
    /// Order ID
    pub order_id: String,
    /// Account address
    pub account_address: String,
    /// Market ID
    pub market_id: String,
    /// BUY or SELL
    pub side: String,
    /// Deprecated derived field
    pub order_type: String,
    /// Original order price
    #[serde(with = "rust_decimal::serde::str")]
    pub orig_price: Decimal,
    /// Original order quantity
    #[serde(with = "rust_decimal::serde::str")]
    pub orig_qty: Decimal,
    /// Actual execution price
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Actual execution quantity
    #[serde(with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Fee token (currently USDT)
    pub fee_token_id: String,
    /// Fee amount
    #[serde(with = "rust_decimal::serde::str")]
    pub fee: Decimal,
    /// True if this side was the maker
    pub is_maker: bool,
    /// Trade ID
    pub trade_id: String,
    /// Realized PnL for closing fills (omitempty — absent for non-closing fills)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub realized_pnl: Option<Decimal>,
    /// Fill timestamp (ms epoch)
    pub created_at: i64,
}

/// Single margin tier entry within a market.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpMarginTier {
    /// Tier index (ascending notional)
    pub tier: u32,
    /// Max notional for this tier ("0" = open-ended catch-all)
    #[serde(with = "rust_decimal::serde::str")]
    pub notional_cap: Decimal,
    /// Initial margin rate (fraction)
    #[serde(with = "rust_decimal::serde::str")]
    pub initial_margin_rate: Decimal,
    /// Maintenance margin rate (fraction)
    #[serde(with = "rust_decimal::serde::str")]
    pub maintenance_margin_rate: Decimal,
    /// Max leverage for this tier
    pub max_leverage: u32,
}

/// Perp market metadata — returned by /market (within the "symbols" array).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpMarket {
    /// Numeric market ID
    pub market_id: String,
    /// Symbol (e.g. BTCUSDT)
    pub symbol: String,
    /// Display description
    pub description: String,
    /// Exchange code
    pub exchange: String,
    /// Market type (perpetual)
    #[serde(rename = "type")]
    pub market_type: String,
    /// DB status
    pub status: String,
    /// Max leverage (fallback when tier ladder is empty)
    pub max_leverage: String,
    /// Base initial margin rate
    #[serde(with = "rust_decimal::serde::str")]
    pub initial_margin_rate: Decimal,
    /// Base maintenance margin rate
    #[serde(with = "rust_decimal::serde::str")]
    pub maintenance_margin_rate: Decimal,
    /// "0"=both, "1"=ISOLATED only, "2"=CROSS only
    pub margin_mode_restriction: String,
    /// Maker fee
    #[serde(with = "rust_decimal::serde::str")]
    pub maker_fee: Decimal,
    /// Taker fee
    #[serde(with = "rust_decimal::serde::str")]
    pub taker_fee: Decimal,
    /// Funding interval in seconds
    #[serde(deserialize_with = "de_u64_flex")]
    pub funding_interval: u64,
    /// Max funding rate
    #[serde(with = "rust_decimal::serde::str")]
    pub max_funding_rate: Decimal,
    /// Price tick size
    #[serde(with = "rust_decimal::serde::str")]
    pub tick_size: Decimal,
    /// Quantity lot size
    #[serde(with = "rust_decimal::serde::str")]
    pub lot_size: Decimal,
    /// Minimum notional value
    #[serde(with = "rust_decimal::serde::str")]
    pub min_notional: Decimal,
    /// Max open interest
    #[serde(with = "rust_decimal::serde::str")]
    pub max_open_interest: Decimal,
    /// TradingView UDF compat
    pub minmov: u32,
    /// TradingView UDF compat
    pub pricescale: u32,
    /// TradingView session (e.g. "24x7")
    pub session: String,
    /// TradingView UDF compat
    pub has_intraday: bool,
    /// TradingView UDF compat
    pub has_empty_bars: bool,
    /// Logo image URL (empty string if unset; field omitted by server when unset)
    #[serde(default)]
    pub logo_urls: String,
    /// Market image URL (empty string if unset; field omitted by server when unset)
    #[serde(default)]
    pub image: String,
    /// Margin tier ladder (empty = use base IMR/MMR/maxLeverage)
    #[serde(default, deserialize_with = "de_vec_null")]
    pub tiers: Vec<PerpMarginTier>,
    /// PUBLIC_TRADING entry timestamp (ms; omitempty)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_trading_time: Option<i64>,
    /// DELISTED timestamp (ms; omitempty)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delisted_time: Option<i64>,
    /// Announcement link URL (omitempty)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub announcement_link_url: Option<String>,
    /// Row creation timestamp (ms epoch)
    pub created_at: i64,
    /// Row last-updated timestamp (ms epoch)
    pub updated_at: i64,
}

/// Top-level response wrapper for /market.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketsResponse {
    /// Market list
    pub symbols: Vec<PerpMarket>,
    /// Default maker fee (first market's makerFee; omitted if no markets)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub default_maker_fee: Option<Decimal>,
    /// Default taker fee (first market's takerFee; omitted if no markets)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub default_taker_fee: Option<Decimal>,
}

/// Ticker snapshot — returned by /market/ticker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpTicker {
    /// Market ID
    pub market_id: String,
    /// Symbol
    pub symbol: String,
    /// Last trade price
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Mark price
    #[serde(with = "rust_decimal::serde::str")]
    pub mark_price: Decimal,
    /// Index price
    #[serde(with = "rust_decimal::serde::str")]
    pub index_price: Decimal,
    /// 24h open price
    #[serde(with = "rust_decimal::serde::str")]
    pub open24h: Decimal,
    /// 24h high price
    #[serde(with = "rust_decimal::serde::str")]
    pub high24h: Decimal,
    /// 24h low price
    #[serde(with = "rust_decimal::serde::str")]
    pub low24h: Decimal,
    /// 24h base volume
    #[serde(with = "rust_decimal::serde::str")]
    pub volume24h: Decimal,
    /// 24h quote volume
    #[serde(with = "rust_decimal::serde::str")]
    pub quote_volume24h: Decimal,
    /// Latest settled funding rate (signed)
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_rate: Decimal,
    /// Next funding settlement timestamp (ms)
    pub next_funding_time: i64,
    /// Milliseconds until next funding (0-clamped)
    pub funding_remaining_time: i64,
    /// Cumulative funding index (18 decimals, signed)
    #[serde(with = "decimal_empty_zero")]
    pub cumulative_funding_index: Decimal,
    /// Funding interval in seconds
    pub funding_interval_sec: u64,
    /// In-flight predicted funding rate (signed)
    #[serde(with = "decimal_empty_zero")]
    pub predicted_funding_rate: Decimal,
    /// Open interest (currently always "0")
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest: Decimal,
}

/// Order book depth snapshot — returned by /market/depth.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpDepth {
    /// Bid price/quantity pairs (descending price)
    pub bids: Vec<[String; 2]>,
    /// Ask price/quantity pairs (ascending price)
    pub asks: Vec<[String; 2]>,
    /// Snapshot timestamp (ms epoch)
    pub updated_at: i64,
    /// Depth update sequence ID
    pub last_updated_id: u64,
}

/// Public trade — returned by /market/trades.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpTrade {
    /// Trade ID
    pub trade_id: String,
    /// Market ID
    pub market_id: String,
    /// Trade price
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Trade quantity
    #[serde(with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// True if the buy side was the maker
    pub is_buyer_maker: bool,
    /// True if this was a liquidation trade.
    /// The `perp_aggTrade` WS stream omits this on normal trades (omitempty) → defaults to false;
    /// REST `/market/trades` always includes it.
    #[serde(default)]
    pub is_liquidation: bool,
    /// True if this was an ADL trade. Omitted by `perp_aggTrade` on normal trades → false.
    #[serde(default)]
    pub is_adl: bool,
    /// Trade timestamp (ms epoch)
    pub created_at: i64,
    /// Buy order ID
    pub buy_order_id: String,
    /// Sell order ID
    pub sell_order_id: String,
}

/// One OHLCV bar — an element of the `/market/candles` response array.
/// The server returns `result` as a JSON array of these objects (not the columnar
/// TradingView UDF `{s,t,o,h,l,c,v}` shape); no-trade buckets carry the prior close
/// across open/high/low/close with `volume = 0`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpCandle {
    /// Numeric market ID (server sends it as a string)
    pub market_id: String,
    /// Resolution label echoed back by the server (e.g. "1", "60")
    pub resolution: String,
    /// Bar open time (Unix seconds)
    pub timestamp: i64,
    /// Open price
    pub open: f64,
    /// High price
    pub high: f64,
    /// Low price
    pub low: f64,
    /// Close price
    pub close: f64,
    /// Base asset volume
    pub volume: f64,
    /// Quote asset volume
    pub quote_volume: f64,
    /// Number of trades aggregated into the bar
    pub trade_count: u64,
}

// ---------------------------------------------------------------------------
// Query parameter structs
// ---------------------------------------------------------------------------

/// Pagination/filter query for order endpoints (/order/open, /order, /order/trade).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpOrderQuery {
    /// Filter by market ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    /// Start timestamp (ms epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<i64>,
    /// End timestamp (ms epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<i64>,
    /// Keyset cursor (id from previous page)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<i64>,
    /// Max rows per page (default 100, max 500)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Pagination/filter query for position history (/position/history).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpHistoryQuery {
    /// Filter by market ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    /// Start openedAt timestamp (ms epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<i64>,
    /// End openedAt timestamp (ms epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<i64>,
    /// Max rows per page (default 100, max 500)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Pagination/filter query for funding history (/wallet/funding).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpFundingQuery {
    /// Filter by market ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    /// Start timestamp (ms epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<i64>,
    /// End timestamp (ms epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<i64>,
    /// Keyset cursor from previous page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<i64>,
    /// Max rows per page (default 100, max 500)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}
