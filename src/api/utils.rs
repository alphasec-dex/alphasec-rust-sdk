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

/// Convert market ID to market symbol
pub fn market_id_to_market(market_id: &str, token_id_symbol_map: &HashMap<u32, String>) -> Result<String> {
    let parts: Vec<&str> = market_id.split('_').collect();
    if parts.len() != 2 {
        return Err(AlphaSecError::invalid_parameter(
            format!("Invalid market ID format: {}. Expected format: BASE_QUOTE", market_id)
        ));
    }
    
    let base_token_id = parts[0].parse::<u32>()
        .map_err(|e| AlphaSecError::invalid_parameter(format!("Invalid base token ID: {}: {}", parts[0], e)))?;
    let quote_token_id = parts[1].parse::<u32>()
        .map_err(|e| AlphaSecError::invalid_parameter(format!("Invalid quote token ID: {}: {}", parts[1], e)))?;

    let base_symbol = token_id_symbol_map.get(&base_token_id)
        .ok_or_else(|| AlphaSecError::not_found(format!("Base token not found: {}", base_token_id)))?;

    let quote_symbol = token_id_symbol_map.get(&quote_token_id)
        .ok_or_else(|| AlphaSecError::not_found(format!("Quote token not found: {}", quote_token_id)))?;

    Ok(format!("{}/{}", base_symbol, quote_symbol))
}

/// Convert symbol to token ID
pub fn symbol_to_token_id(symbol: &str, symbol_token_id_map: &HashMap<String, u32>) -> Result<u32> {
    let token_id = symbol_token_id_map.get(symbol).copied().ok_or_else(|| AlphaSecError::not_found(format!("Token not found: {}", symbol)))?;
    Ok(token_id)
}

/// Convert token ID to symbol
pub fn token_id_to_symbol(token_id: u32, token_id_symbol_map: &HashMap<u32, String>) -> Result<String> {
    let symbol = token_id_symbol_map.get(&token_id).ok_or_else(|| AlphaSecError::not_found(format!("Token not found: {}", token_id)))?;
    Ok(symbol.clone())
}