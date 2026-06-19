//! Market data types for AlphaSec API

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Token information from /api/v1/market/tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]

pub struct Token {
    /// Token ID (internal identifier)
    pub token_id: String,
    /// Layer 1 symbol (e.g., "KAIA", "USDT")
    #[serde(rename = "l1Symbol")]
    pub symbol: String,
    /// Layer 2 symbol (e.g., "KAIA", "USDT")
    #[serde(rename = "l2Symbol")]
    pub l2_symbol: String,
    /// Layer 1 contract address
    #[serde(rename = "l1Address")]
    pub l1_address: String,
    /// Token decimals (all the l2 tokens have the same decimal:18)
    #[serde(rename = "l1Decimal")]
    pub decimals: u32,
    /// Whether the token is active
    #[serde(default)]
    pub is_active: bool,
}

/// Market information from /api/v1/market
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Market {
    /// Market ID (e.g., "1_2" for tokenId1/tokenId2)
    pub market_id: String,
    /// Base token ID
    pub base_token_id: String,
    /// Quote token ID
    pub quote_token_id: String,
    /// Market symbol (e.g., "KAIA/USDT")
    pub ticker: String,
    /// Market description
    pub description: String,
    /// Exchange name
    pub exchange: String,
    /// Market type (e.g., "spot")
    #[serde(rename = "type")]
    pub market_type: String,
    /// Whether the market is listed
    pub listed: bool,
    /// Taker fee
    pub taker_fee: String,
    /// Maker fee
    pub maker_fee: String,
}

/// Single level in orderbook depth (REST)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthLevel {
    /// Price as string
    pub price: String,
    /// Quantity as string
    pub quantity: String,
}

/// Depth payload from REST /api/v1/market/depth
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Depth {
    /// Bid levels (desc)
    pub bids: Vec<DepthLevel>,
    /// Ask levels (asc)
    pub asks: Vec<DepthLevel>,
    /// Last update timestamp (ms)
    #[serde(rename = "updatedAt")]
    pub updated_at: u64,
    /// Last updated ID
    #[serde(rename = "lastUpdatedId")]
    pub last_updated_id: i64,
}

/// Ticker information from /api/v1/market/ticker
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    /// Market ID (e.g., "1_2")
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
    /// Opening price 24 hours ago
    #[serde(rename = "open24h")]
    pub open_24h: String,
    /// Highest price in the last 24 hours
    #[serde(rename = "high24h")]
    pub high_24h: String,
    /// Lowest price in the last 24 hours
    #[serde(rename = "low24h")]
    pub low_24h: String,
    /// Volume in base asset in the last 24 hours
    #[serde(rename = "volume24h")]
    pub volume_24h: String,
    /// Volume in quote asset in the last 24 hours
    #[serde(rename = "quoteVolume24h")]
    pub quote_volume_24h: String,
}

/// Trade information from /api/v1/market/trades
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// Unique trade ID
    pub trade_id: String,
    /// Market ID
    pub market_id: String,
    /// Trade price (in wei or smallest unit)
    pub price: String,
    /// Trade quantity (in wei or smallest unit)
    pub quantity: String,
    /// Buy order ID
    pub buy_order_id: String,
    /// Sell order ID
    pub sell_order_id: String,
    /// Created at
    pub created_at: u64,
    /// Is buyer maker
    pub is_buyer_maker: bool,
}

/// Trade side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    /// Buy
    Buy,
    /// Sell
    Sell,
}

impl std::fmt::Display for TradeSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeSide::Buy => write!(f, "buy"),
            TradeSide::Sell => write!(f, "sell"),
        }
    }
}

/// Token metadata mapping helper
#[derive(Debug, Clone)]
pub struct TokenMetadata {
    /// Token ID to symbol mapping
    pub token_id_symbol_map: HashMap<String, String>,
    /// Symbol to token ID mapping
    pub symbol_token_id_map: HashMap<String, String>,
    /// Token ID to L1 address mapping
    pub token_id_address_map: HashMap<String, String>,
    /// Token ID to decimal mapping
    pub token_id_decimal_map: HashMap<String, String>,
}

impl TokenMetadata {
    /// Create new token metadata from tokens list
    pub fn from_tokens(tokens: &[Token]) -> Self {
        let mut token_id_symbol_map = HashMap::new();
        let mut symbol_token_id_map = HashMap::new();
        let mut token_id_address_map = HashMap::new();
        let mut token_id_decimal_map = HashMap::new();

        for token in tokens {
            token_id_symbol_map.insert(token.token_id.clone(), token.l2_symbol.clone());
            symbol_token_id_map.insert(token.l2_symbol.clone(), token.token_id.clone());
            token_id_address_map.insert(token.token_id.clone(), token.l1_address.clone());
            token_id_decimal_map.insert(token.token_id.clone(), token.decimals.to_string());
        }

        Self {
            token_id_symbol_map,
            symbol_token_id_map,
            token_id_address_map,
            token_id_decimal_map,
        }
    }

    /// Convert market symbol to market ID
    pub fn market_to_market_id(&self, market: &str) -> crate::Result<String> {
        let parts: Vec<&str> = market.split('/').collect();
        if parts.len() != 2 {
            return Err(crate::AlphaSecError::invalid_parameter(format!(
                "Invalid market format: {}. Expected format: BASE/QUOTE",
                market
            )));
        }

        let base_symbol = parts[0];
        let quote_symbol = parts[1];

        let base_token_id = self.symbol_token_id_map.get(base_symbol).ok_or_else(|| {
            crate::AlphaSecError::not_found(format!("Base token not found: {}", base_symbol))
        })?;

        let quote_token_id = self.symbol_token_id_map.get(quote_symbol).ok_or_else(|| {
            crate::AlphaSecError::not_found(format!("Quote token not found: {}", quote_symbol))
        })?;

        Ok(format!("{}_{}", base_token_id, quote_token_id))
    }

    /// Convert market ID to market symbol
    pub fn market_id_to_market(&self, market_id: &str) -> crate::Result<String> {
        let parts: Vec<&str> = market_id.split('_').collect();
        if parts.len() != 2 {
            return Err(crate::AlphaSecError::invalid_parameter(format!(
                "Invalid market ID format: {}",
                market_id
            )));
        }

        let base_token_id = parts[0];
        let quote_token_id = parts[1];

        let base_symbol = self.token_id_symbol_map.get(base_token_id).ok_or_else(|| {
            crate::AlphaSecError::not_found(format!("Base token ID not found: {}", base_token_id))
        })?;

        let quote_symbol = self
            .token_id_symbol_map
            .get(quote_token_id)
            .ok_or_else(|| {
                crate::AlphaSecError::not_found(format!(
                    "Quote token ID not found: {}",
                    quote_token_id
                ))
            })?;

        Ok(format!("{}/{}", base_symbol, quote_symbol))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AlphaSecError;

    /// Test-only Token builder. `symbol` (the L1 symbol) is deliberately distinct from
    /// `l2_symbol` so a from_tokens regression that keys the map on the L1 symbol makes
    /// every l2-symbol lookup fail instead of passing by coincidence.
    fn make_token(token_id: &str, l2_symbol: &str) -> Token {
        Token {
            token_id: token_id.to_string(),
            symbol: format!("L1{}", l2_symbol),
            l2_symbol: l2_symbol.to_string(),
            l1_address: format!("0x{:0>40}", token_id),
            decimals: 18,
            is_active: true,
        }
    }

    fn metadata() -> TokenMetadata {
        TokenMetadata::from_tokens(&[make_token("1", "KAIA"), make_token("2", "USDT")])
    }

    #[test]
    fn market_to_market_id_joins_base_then_quote_token_ids() {
        let md = metadata();
        assert_eq!(md.market_to_market_id("KAIA/USDT").unwrap(), "1_2");
        // Reversed market must produce the reversed id; if base/quote were swapped
        // internally, both assertions cannot hold at once.
        assert_eq!(md.market_to_market_id("USDT/KAIA").unwrap(), "2_1");
    }

    #[test]
    fn market_format_without_exactly_one_slash_is_invalid_parameter() {
        let md = metadata();
        for input in ["KAIAUSDT", "A/B/C", ""] {
            let err = md.market_to_market_id(input).unwrap_err();
            assert!(
                matches!(err, AlphaSecError::InvalidParameter(_)),
                "{:?} expected InvalidParameter, got {:?}",
                input,
                err
            );
        }
    }

    #[test]
    fn empty_quote_is_quote_not_found_distinct_from_base_missing() {
        let md = metadata();

        // "KAIA/" -> base resolves, empty quote symbol fails the quote lookup.
        let err = md.market_to_market_id("KAIA/").unwrap_err();
        assert!(matches!(err, AlphaSecError::NotFound(_)), "got {:?}", err);
        let msg = err.to_string();
        assert!(msg.contains("Quote token not found"), "msg: {}", msg);
        assert!(!msg.contains("Base token"), "msg: {}", msg);

        // Unknown base is attributed to the base side. (Lookup ORDER itself is pinned in
        // empty_token_slice_makes_wellformed_lookups_not_found, where both lookups fail.)
        let err = md.market_to_market_id("XXX/USDT").unwrap_err();
        assert!(matches!(err, AlphaSecError::NotFound(_)), "got {:?}", err);
        let msg = err.to_string();
        assert!(msg.contains("Base token not found: XXX"), "msg: {}", msg);
        assert!(!msg.contains("Quote token"), "msg: {}", msg);

        // Empty base ("/USDT") is also attributed to the base side.
        let err = md.market_to_market_id("/USDT").unwrap_err();
        assert!(matches!(err, AlphaSecError::NotFound(_)), "got {:?}", err);
        let msg = err.to_string();
        assert!(msg.contains("Base token not found"), "msg: {}", msg);
        assert!(!msg.contains("Quote token"), "msg: {}", msg);
    }

    #[test]
    fn market_id_to_market_inverts_market_to_market_id() {
        let md = metadata();
        for market in ["KAIA/USDT", "USDT/KAIA"] {
            let id = md.market_to_market_id(market).unwrap();
            assert_eq!(md.market_id_to_market(&id).unwrap(), market);
        }
    }

    #[test]
    fn market_id_with_wrong_separator_or_extra_parts_is_invalid_parameter() {
        let md = metadata();
        for input in ["1-2", "1_2_3"] {
            let err = md.market_id_to_market(input).unwrap_err();
            assert!(
                matches!(err, AlphaSecError::InvalidParameter(_)),
                "{:?} expected InvalidParameter, got {:?}",
                input,
                err
            );
        }
    }

    #[test]
    fn duplicate_l2_symbol_resolves_to_last_token_id() {
        let md = TokenMetadata::from_tokens(&[
            make_token("7", "DUP"),
            make_token("8", "DUP"),
            make_token("2", "USDT"),
        ]);
        // symbol -> id direction loses token "7": the later insert wins.
        assert_eq!(md.symbol_token_id_map.get("DUP"), Some(&"8".to_string()));
        assert_eq!(md.market_to_market_id("DUP/USDT").unwrap(), "8_2");
        // id -> symbol direction keeps both ids (keys are distinct), so the loss is
        // one-directional: "7_2" still maps back to "DUP/USDT".
        assert_eq!(md.market_id_to_market("7_2").unwrap(), "DUP/USDT");
    }

    #[test]
    fn empty_token_slice_makes_wellformed_lookups_not_found() {
        let md = TokenMetadata::from_tokens(&[]);
        let err = md.market_to_market_id("KAIA/USDT").unwrap_err();
        assert!(matches!(err, AlphaSecError::NotFound(_)), "got {:?}", err);
        // Both lookups fail on empty metadata, so the winning message is the only input
        // that truly pins lookup ORDER: a quote-first implementation would report the
        // quote side here.
        let msg = err.to_string();
        assert!(msg.contains("Base token not found: KAIA"), "msg: {}", msg);
        let err = md.market_id_to_market("1_2").unwrap_err();
        assert!(matches!(err, AlphaSecError::NotFound(_)), "got {:?}", err);
    }
}
