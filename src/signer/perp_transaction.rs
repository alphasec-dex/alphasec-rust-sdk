//! Perp transaction models for AlphaSec perpetual futures operations

use serde::{Deserialize, Serialize};

use crate::types::dex_commands::*;

/// Perp order model (0x41)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerpOrderModel {
    pub l1owner: String,
    #[serde(rename = "marketId")]
    pub market_id: u64,
    pub side: u8, // 0=Buy, 1=Sell
    pub price: String, // 18 decimal big.Int string
    pub quantity: String, // 18 decimal big.Int string
    #[serde(rename = "isReduceOnly")]
    pub is_reduce_only: bool,
    #[serde(rename = "timeInForce")]
    pub time_in_force: u8, // 0=GTC, 1=IOC, 2=POST, 3=MARKET
    #[serde(rename = "clientOrderId", skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}

impl PerpOrderModel {
    /// Create alphasec-style transaction bytes (0x41 + JSON)
    /// price and quantity are serialized as raw JSON numbers for Go big.Int compat
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![DEX_COMMAND_PERP_ORDER];
        // Manual JSON construction to emit price/quantity as raw numbers
        let json = self.to_json_raw_numbers();
        final_tx_bytes.extend_from_slice(json.as_bytes());
        Ok(final_tx_bytes)
    }

    /// Produce JSON with price and quantity as unquoted numbers
    fn to_json_raw_numbers(&self) -> String {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "l1owner".to_string(),
            serde_json::Value::String(self.l1owner.clone()),
        );
        obj.insert(
            "marketId".to_string(),
            serde_json::Value::Number(self.market_id.into()),
        );
        obj.insert(
            "side".to_string(),
            serde_json::Value::Number(self.side.into()),
        );
        obj.insert(
            "isReduceOnly".to_string(),
            serde_json::Value::Bool(self.is_reduce_only),
        );
        obj.insert(
            "timeInForce".to_string(),
            serde_json::Value::Number(self.time_in_force.into()),
        );
        if let Some(ref coid) = self.client_order_id {
            obj.insert(
                "clientOrderId".to_string(),
                serde_json::Value::String(coid.clone()),
            );
        }

        // Serialize the base object, then inject price/quantity as raw numbers
        let mut json_str = serde_json::to_string(&obj).unwrap();
        // Remove trailing '}'
        json_str.pop();
        // Append price and quantity as raw (unquoted) numbers
        json_str.push_str(&format!(
            ",\"price\":{},\"quantity\":{}}}",
            self.price, self.quantity
        ));
        json_str
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
    pub token: String, // "2" = USDT
    pub amount: String, // raw integer as string (value × 10^18)
}

impl PerpDepositModel {
    /// Create alphasec-style transaction bytes (0x12 + JSON)
    /// amount is serialized as raw JSON number for Go big.Int compat
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![DEX_COMMAND_PERP_DEPOSIT];
        let json = self.to_json_raw_amount();
        final_tx_bytes.extend_from_slice(json.as_bytes());
        Ok(final_tx_bytes)
    }

    fn to_json_raw_amount(&self) -> String {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "l1owner".to_string(),
            serde_json::Value::String(self.l1owner.clone()),
        );
        obj.insert(
            "token".to_string(),
            serde_json::Value::String(self.token.clone()),
        );
        let mut json_str = serde_json::to_string(&obj).unwrap();
        json_str.pop();
        json_str.push_str(&format!(",\"amount\":{}}}", self.amount));
        json_str
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
    /// Create alphasec-style transaction bytes (0x44 + JSON)
    /// amount is serialized as raw JSON number for Go big.Int compat
    pub fn to_wire(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut final_tx_bytes = vec![DEX_COMMAND_PERP_WITHDRAW];
        let json = self.to_json_raw_amount();
        final_tx_bytes.extend_from_slice(json.as_bytes());
        Ok(final_tx_bytes)
    }

    fn to_json_raw_amount(&self) -> String {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "l1owner".to_string(),
            serde_json::Value::String(self.l1owner.clone()),
        );
        obj.insert(
            "token".to_string(),
            serde_json::Value::String(self.token.clone()),
        );
        let mut json_str = serde_json::to_string(&obj).unwrap();
        json_str.pop();
        json_str.push_str(&format!(",\"amount\":{}}}", self.amount));
        json_str
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
