//! Utility functions for API operations

use crate::error::{AlphaSecError, Result};
use std::collections::HashMap;

/// Convert market symbol to market ID
pub fn market_to_market_id(
    market: &str,
    symbol_token_id_map: &HashMap<String, u32>,
) -> Result<String> {
    let parts: Vec<&str> = market.split('/').collect();
    if parts.len() != 2 {
        return Err(AlphaSecError::invalid_parameter(format!(
            "Invalid market format: {}. Expected format: BASE/QUOTE",
            market
        )));
    }

    let base_symbol = parts[0];
    let quote_symbol = parts[1];

    let base_token_id = symbol_token_id_map.get(base_symbol).ok_or_else(|| {
        AlphaSecError::not_found(format!("Base token not found: {}", base_symbol))
    })?;

    let quote_token_id = symbol_token_id_map.get(quote_symbol).ok_or_else(|| {
        AlphaSecError::not_found(format!("Quote token not found: {}", quote_symbol))
    })?;

    Ok(format!("{}_{}", base_token_id, quote_token_id))
}

/// Convert market ID to market symbol
pub fn market_id_to_market(
    market_id: &str,
    token_id_symbol_map: &HashMap<u32, String>,
) -> Result<String> {
    let parts: Vec<&str> = market_id.split('_').collect();
    if parts.len() != 2 {
        return Err(AlphaSecError::invalid_parameter(format!(
            "Invalid market ID format: {}. Expected format: BASE_QUOTE",
            market_id
        )));
    }

    let base_token_id = parts[0].parse::<u32>().map_err(|e| {
        AlphaSecError::invalid_parameter(format!("Invalid base token ID: {}: {}", parts[0], e))
    })?;
    let quote_token_id = parts[1].parse::<u32>().map_err(|e| {
        AlphaSecError::invalid_parameter(format!("Invalid quote token ID: {}: {}", parts[1], e))
    })?;

    let base_symbol = token_id_symbol_map.get(&base_token_id).ok_or_else(|| {
        AlphaSecError::not_found(format!("Base token not found: {}", base_token_id))
    })?;

    let quote_symbol = token_id_symbol_map.get(&quote_token_id).ok_or_else(|| {
        AlphaSecError::not_found(format!("Quote token not found: {}", quote_token_id))
    })?;

    Ok(format!("{}/{}", base_symbol, quote_symbol))
}

/// Convert symbol to token ID
pub fn symbol_to_token_id(symbol: &str, symbol_token_id_map: &HashMap<String, u32>) -> Result<u32> {
    let token_id = symbol_token_id_map
        .get(symbol)
        .copied()
        .ok_or_else(|| AlphaSecError::not_found(format!("Token not found: {}", symbol)))?;
    Ok(token_id)
}

/// Convert token ID to symbol
pub fn token_id_to_symbol(
    token_id: u32,
    token_id_symbol_map: &HashMap<u32, String>,
) -> Result<String> {
    let symbol = token_id_symbol_map
        .get(&token_id)
        .ok_or_else(|| AlphaSecError::not_found(format!("Token not found: {}", token_id)))?;
    Ok(symbol.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol_map() -> HashMap<String, u32> {
        let mut map = HashMap::new();
        map.insert("BTC".to_string(), 1);
        map.insert("USDT".to_string(), 2);
        map
    }

    fn id_map() -> HashMap<u32, String> {
        let mut map = HashMap::new();
        map.insert(1, "BTC".to_string());
        map.insert(2, "USDT".to_string());
        map
    }

    #[test]
    fn market_to_market_id_keeps_base_before_quote() {
        let id = market_to_market_id("BTC/USDT", &symbol_map()).unwrap();
        assert_eq!(id, "1_2", "flipped base/quote would produce 2_1");
    }

    #[test]
    fn market_to_market_id_rejects_wrong_slash_count() {
        for input in ["BTCUSDT", "A/B/C", ""] {
            let err = market_to_market_id(input, &symbol_map()).unwrap_err();
            assert!(
                matches!(err, AlphaSecError::InvalidParameter(_)),
                "input {:?}: expected InvalidParameter, got {:?}",
                input,
                err
            );
        }
    }

    #[test]
    fn market_to_market_id_empty_base_reports_base_not_found() {
        let err = market_to_market_id("/USDT", &symbol_map()).unwrap_err();
        match err {
            AlphaSecError::NotFound(msg) => {
                assert!(msg.contains("Base token not found"), "got: {}", msg);
                assert!(
                    !msg.contains("Quote"),
                    "base failure must not blame quote: {}",
                    msg
                );
            }
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn market_to_market_id_empty_quote_reports_quote_not_found() {
        let err = market_to_market_id("BTC/", &symbol_map()).unwrap_err();
        match err {
            AlphaSecError::NotFound(msg) => {
                assert!(msg.contains("Quote token not found"), "got: {}", msg);
                assert!(
                    !msg.contains("Base"),
                    "quote failure must not blame base: {}",
                    msg
                );
            }
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn market_id_to_market_rejects_non_numeric_and_negative_ids() {
        for input in ["a_2", "-1_2"] {
            let err = market_id_to_market(input, &id_map()).unwrap_err();
            assert!(
                matches!(err, AlphaSecError::InvalidParameter(_)),
                "input {:?}: expected InvalidParameter, got {:?}",
                input,
                err
            );
        }
    }

    #[test]
    fn market_id_to_market_rejects_wrong_underscore_count() {
        for input in ["1-2", "1_2_3"] {
            let err = market_id_to_market(input, &id_map()).unwrap_err();
            assert!(
                matches!(err, AlphaSecError::InvalidParameter(_)),
                "input {:?}: expected InvalidParameter, got {:?}",
                input,
                err
            );
        }
    }

    #[test]
    fn market_conversion_roundtrips_in_both_directions() {
        let sm = symbol_map();
        let im = id_map();

        let id = market_to_market_id("BTC/USDT", &sm).unwrap();
        assert_eq!(market_id_to_market(&id, &im).unwrap(), "BTC/USDT");

        // Reversed pair is an independent valid input; ordering must survive too.
        let market = market_id_to_market("2_1", &im).unwrap();
        assert_eq!(market, "USDT/BTC");
        assert_eq!(market_to_market_id(&market, &sm).unwrap(), "2_1");
    }

    #[test]
    fn token_id_to_symbol_treats_zero_as_regular_key() {
        let mut map = HashMap::new();
        map.insert(0u32, "NATIVE".to_string());
        assert_eq!(token_id_to_symbol(0, &map).unwrap(), "NATIVE");

        let empty: HashMap<u32, String> = HashMap::new();
        let err = token_id_to_symbol(0, &empty).unwrap_err();
        assert!(
            matches!(err, AlphaSecError::NotFound(_)),
            "missing id 0 must be NotFound, got {:?}",
            err
        );
    }
}
