//! Utility functions for API operations

use crate::error::{AlphaSecError, Result};
use std::collections::HashMap;

/// Convert market symbol to market ID
pub fn market_to_market_id(market: &str, symbol_token_id_map: &HashMap<String, u32>) -> Result<String> {
    let parts: Vec<&str> = market.split('/').collect();
    if parts.len() != 2 {
        return Err(AlphaSecError::invalid_parameter(
            format!("Invalid market format: {}. Expected format: BASE/QUOTE", market)
        ));
    }

    let base_symbol = parts[0];
    let quote_symbol = parts[1];

    let base_token_id = symbol_token_id_map.get(base_symbol)
        .ok_or_else(|| AlphaSecError::not_found(format!("Base token not found: {}", base_symbol)))?;

    let quote_token_id = symbol_token_id_map.get(quote_symbol)
        .ok_or_else(|| AlphaSecError::not_found(format!("Quote token not found: {}", quote_symbol)))?;

    Ok(format!("{}_{}", base_token_id, quote_token_id))
}
