//! Perp transaction models for AlphaSec perpetual futures operations

use serde::{Deserialize, Serialize};

use crate::types::dex_commands::*;

/// Perp order model (0x41)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerpOrderModel {
    pub l1owner: String,
    #[serde(rename = "marketId")]
    pub market_id: u64,
    pub side: u8,         // 0=Buy, 1=Sell
    pub price: String,    // human-readable decimal (quoted on wire; node scales 1e18)
    pub quantity: String, // human-readable decimal (quoted on wire; node scales 1e18)
    #[serde(rename = "isReduceOnly")]
    pub is_reduce_only: bool,
    #[serde(rename = "timeInForce")]
    pub time_in_force: u8, // 0=GTC, 1=IOC, 2=POST, 3=MARKET
    #[serde(rename = "clientOrderId", skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}

impl PerpOrderModel {
    /// Create alphasec-style transaction bytes (0x41 + JSON).
    ///
    /// price and quantity are `String` fields holding the human-readable decimal value, so
    /// standard serde emits them as quoted strings (the node scales 1e18 internally).
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![DEX_COMMAND_PERP_ORDER];
        final_tx_bytes.extend_from_slice(&serde_json::to_vec(self)?);
        Ok(final_tx_bytes)
    }
}

/// Perp cancel model (0x42)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerpCancelModel {
    pub l1owner: String,
    #[serde(rename = "marketId")]
    pub market_id: u64,
    #[serde(rename = "orderId")]
    pub order_id: String, // tx hash
}

impl PerpCancelModel {
    /// Create alphasec-style transaction bytes (0x42 + JSON)
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![DEX_COMMAND_PERP_CANCEL];
        final_tx_bytes.extend_from_slice(&serde_json::to_vec(self)?);
        Ok(final_tx_bytes)
    }
}

/// Perp cancel all model (0x43)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerpCancelAllModel {
    pub l1owner: String,
    #[serde(rename = "marketId")]
    pub market_id: u64, // 0 = all markets
}

impl PerpCancelAllModel {
    /// Create alphasec-style transaction bytes (0x43 + JSON)
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![DEX_COMMAND_PERP_CANCEL_ALL];
        final_tx_bytes.extend_from_slice(&serde_json::to_vec(self)?);
        Ok(final_tx_bytes)
    }
}

/// Perp deposit model (0x12) — Spot→Perp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerpDepositModel {
    pub l1owner: String,
    pub token: String,  // "2" = USDT
    pub amount: String, // raw integer as string (value × 10^18)
}

impl PerpDepositModel {
    /// Create alphasec-style transaction bytes (0x12 + JSON).
    ///
    /// `amount` is serialized as a JSON **string**: the server's `perpDepositContextJSON.amount`
    /// is a Go `string` field and rejects a raw number with -1103 ("cannot unmarshal number ...
    /// of type string"). All struct fields are `String`, so standard serde quotes them correctly.
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![DEX_COMMAND_PERP_DEPOSIT];
        let json = serde_json::to_string(self)?;
        final_tx_bytes.extend_from_slice(json.as_bytes());
        Ok(final_tx_bytes)
    }
}

/// Perp withdraw model (0x44) — Perp→Spot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerpWithdrawModel {
    pub l1owner: String,
    pub token: String,
    pub amount: String, // raw integer as string (value × 10^18)
}

impl PerpWithdrawModel {
    /// Create alphasec-style transaction bytes (0x44 + JSON).
    ///
    /// `amount` is serialized as a JSON **string**, same as deposit (the server's
    /// `perpWithdrawContextJSON.amount` is a Go `string` field — a raw number is rejected with
    /// -1103). All struct fields are `String`, so standard serde quotes them correctly.
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![DEX_COMMAND_PERP_WITHDRAW];
        let json = serde_json::to_string(self)?;
        final_tx_bytes.extend_from_slice(json.as_bytes());
        Ok(final_tx_bytes)
    }
}

/// Perp modify order model (0x4A) — cancel-and-replace
///
/// None fields are omitted from the JSON wire payload so the server inherits
/// the original order's value. An explicit 0 would be rejected by the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerpModifyModel {
    pub l1owner: String,
    #[serde(rename = "marketId")]
    pub market_id: u64,
    #[serde(rename = "orderId")]
    pub order_id: String,
    /// New price as human-readable decimal (quoted on wire; node scales 1e18). None omits the key (server inherits).
    #[serde(rename = "newPrice", skip_serializing_if = "Option::is_none")]
    pub new_price: Option<String>,
    /// New quantity as human-readable decimal (quoted on wire; node scales 1e18). None omits the key (server inherits).
    #[serde(rename = "newQuantity", skip_serializing_if = "Option::is_none")]
    pub new_quantity: Option<String>,
    #[serde(rename = "clientOrderId", skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}

impl PerpModifyModel {
    /// Create alphasec-style transaction bytes (0x4A + JSON).
    ///
    /// newPrice and newQuantity are `Option<String>` holding the human-readable decimal value
    /// (quoted on wire; the node scales 1e18). `skip_serializing_if = "Option::is_none"` omits
    /// a None field entirely so the server inherits the original order value.
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![DEX_COMMAND_PERP_MODIFY];
        final_tx_bytes.extend_from_slice(&serde_json::to_vec(self)?);
        Ok(final_tx_bytes)
    }
}

/// Perp set leverage model (0x45)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerpSetLeverageModel {
    pub l1owner: String,
    #[serde(rename = "marketId")]
    pub market_id: u64,
    pub leverage: u32, // 1~125
}

impl PerpSetLeverageModel {
    /// Create alphasec-style transaction bytes (0x45 + JSON)
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![DEX_COMMAND_PERP_SET_LEVERAGE];
        final_tx_bytes.extend_from_slice(&serde_json::to_vec(self)?);
        Ok(final_tx_bytes)
    }
}
