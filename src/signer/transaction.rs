//! Transaction models for AlphaSec operations

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Session context model for session management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContextModel {
    #[serde(rename = "type")]
    pub r#type: u8,
    pub publickey: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: u64,
    pub nonce: u64,
    pub l1owner: String,
    pub l1signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
}

impl SessionContextModel {
    pub fn to_wire(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

/// Value transfer model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueTransferModel {
    pub l1owner: String,
    pub to: String,
    pub value: String,
}

impl ValueTransferModel {
    pub fn to_wire(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

/// Token transfer model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransferModel {
    pub l1owner: String,
    pub to: String,
    pub value: String,
    pub token: String,
}

impl TokenTransferModel {
    pub fn to_wire(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

/// Take profit / Stop loss model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpslModel {
    #[serde(rename = "tpLimit")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tp_limit: Option<String>,
    #[serde(rename = "slTrigger")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sl_trigger: Option<String>,
    #[serde(rename = "slLimit")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sl_limit: Option<String>,
}

impl TpslModel {
    pub fn to_wire(&self) -> Value {
        let mut obj = serde_json::Map::new();
        if let Some(ref tp_limit) = self.tp_limit {
            obj.insert("tpLimit".to_string(), Value::String(tp_limit.clone()));
        }
        if let Some(ref sl_trigger) = self.sl_trigger {
            obj.insert("slTrigger".to_string(), Value::String(sl_trigger.clone()));
        }
        if let Some(ref sl_limit) = self.sl_limit {
            obj.insert("slLimit".to_string(), Value::String(sl_limit.clone()));
        }
        Value::Object(obj)
    }
}

/// Order model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderModel {
    pub l1owner: String,
    #[serde(rename = "baseToken")]
    pub base_token: String,
    #[serde(rename = "quoteToken")]
    pub quote_token: String,
    pub side: u32,
    pub price: String,
    pub quantity: String,
    #[serde(rename = "orderType")]
    pub order_type: u32,
    #[serde(rename = "orderMode")]
    pub order_mode: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tpsl: Option<TpslModel>,
}

impl OrderModel {
    /// Create alphasec-style transaction bytes (0x21 + JSON)
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![0x21]; // DEX_COMMAND_ORDER
        final_tx_bytes.extend_from_slice(&serde_json::to_vec(self)?);
        Ok(final_tx_bytes)
    }
}

/// Cancel model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelModel {
    pub l1owner: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
}

impl CancelModel {
    /// Create alphasec-style transaction bytes (0x22 + JSON)
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![0x22]; // DEX_COMMAND_CANCEL
        final_tx_bytes.extend_from_slice(&serde_json::to_vec(self)?);
        Ok(final_tx_bytes)
    }
}

/// Cancel all model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelAllModel {
    pub l1owner: String,
}

impl CancelAllModel {
    /// Create alphasec-style transaction bytes (0x23 + JSON)
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![0x23]; // DEX_COMMAND_CANCEL_ALL
        final_tx_bytes.extend_from_slice(&serde_json::to_vec(self)?);
        Ok(final_tx_bytes)
    }
}

/// Modify model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifyModel {
    pub l1owner: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "newPrice")]
    pub new_price: String,
    #[serde(rename = "newQty")]
    pub new_qty: String,
    #[serde(rename = "orderMode")]
    pub order_mode: u32,
}

impl ModifyModel {
    /// Create alphasec-style transaction bytes (0x24 + JSON)
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![0x24]; // DEX_COMMAND_MODIFY
        final_tx_bytes.extend_from_slice(&serde_json::to_vec(self)?);
        Ok(final_tx_bytes)
    }
}

/// Stop order model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopOrderModel {
    pub l1owner: String,
    #[serde(rename = "baseToken")]
    pub base_token: String,
    #[serde(rename = "quoteToken")]
    pub quote_token: String,
    #[serde(rename = "stopPrice")]
    pub stop_price: String,
    pub price: String,
    pub quantity: String,
    pub side: u32,
    #[serde(rename = "orderType")]
    pub order_type: u32,
    #[serde(rename = "orderMode")]
    pub order_mode: u32,
}

impl StopOrderModel {
    /// Create alphasec-style transaction bytes (0x25 + JSON)
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![0x25]; // DEX_COMMAND_STOP_ORDER
        final_tx_bytes.extend_from_slice(&serde_json::to_vec(self)?);
        Ok(final_tx_bytes)
    }
}
