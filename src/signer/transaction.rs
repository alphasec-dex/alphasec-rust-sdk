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

#[cfg(test)]
mod tests {
    use super::*;

    // ---- test helpers ------------------------------------------------------

    fn sample_order(price: &str, quantity: &str, tpsl: Option<TpslModel>) -> OrderModel {
        OrderModel {
            l1owner: "0x1111111111111111111111111111111111111111".to_string(),
            base_token: "BTC".to_string(),
            quote_token: "USDT".to_string(),
            side: 0,
            price: price.to_string(),
            quantity: quantity.to_string(),
            order_type: 1,
            order_mode: 2,
            tpsl,
        }
    }

    fn sample_stop_order() -> StopOrderModel {
        StopOrderModel {
            l1owner: "0x1111111111111111111111111111111111111111".to_string(),
            base_token: "BTC".to_string(),
            quote_token: "USDT".to_string(),
            stop_price: "95000".to_string(),
            price: "94000".to_string(),
            quantity: "0.5".to_string(),
            side: 1,
            order_type: 0,
            order_mode: 0,
        }
    }

    // Parses wire[1..] and returns the JSON object. serde_json::from_slice fails on
    // trailing NON-whitespace bytes only, so the terminal-byte check below additionally
    // rejects appended whitespace - together they pin "exactly one JSON value, nothing
    // appended".
    fn payload_object(wire: &[u8]) -> serde_json::Map<String, Value> {
        assert_eq!(
            *wire.last().expect("wire must not be empty"),
            b'}',
            "wire must end exactly at the JSON object (no appended bytes)"
        );
        let value: Value = serde_json::from_slice(&wire[1..])
            .expect("bytes after the command byte must parse as exactly one JSON value");
        match value {
            Value::Object(map) => map,
            other => panic!("expected JSON object payload, got: {other}"),
        }
    }

    fn sorted_keys(map: &serde_json::Map<String, Value>) -> Vec<String> {
        let mut keys: Vec<String> = map.keys().cloned().collect();
        keys.sort();
        keys
    }

    // ---- command byte / framing (one test per model so a swap points at it) ----

    #[test]
    fn order_wire_starts_with_0x21_then_single_json_object() {
        let wire = sample_order("90000", "0.5", None).to_wire().unwrap();
        assert_eq!(
            wire[0], 0x21,
            "Order command byte must be protocol literal 0x21"
        );
        assert_eq!(
            wire[1], b'{',
            "JSON must start immediately after the command byte"
        );
        let _ = payload_object(&wire);
    }

    #[test]
    fn cancel_wire_starts_with_0x22_then_single_json_object() {
        let model = CancelModel {
            l1owner: "0xowner".to_string(),
            order_id: "oid-1".to_string(),
        };
        let wire = model.to_wire().unwrap();
        assert_eq!(
            wire[0], 0x22,
            "Cancel command byte must be protocol literal 0x22"
        );
        assert_eq!(
            wire[1], b'{',
            "JSON must start immediately after the command byte"
        );
        let _ = payload_object(&wire);
    }

    #[test]
    fn cancel_all_wire_starts_with_0x23_then_single_json_object() {
        let model = CancelAllModel {
            l1owner: "0xowner".to_string(),
        };
        let wire = model.to_wire().unwrap();
        assert_eq!(
            wire[0], 0x23,
            "CancelAll command byte must be protocol literal 0x23"
        );
        assert_eq!(
            wire[1], b'{',
            "JSON must start immediately after the command byte"
        );
        let _ = payload_object(&wire);
    }

    #[test]
    fn modify_wire_starts_with_0x24_then_single_json_object() {
        let model = ModifyModel {
            l1owner: "0xowner".to_string(),
            order_id: "oid-1".to_string(),
            new_price: "51000".to_string(),
            new_qty: "2".to_string(),
            order_mode: 1,
        };
        let wire = model.to_wire().unwrap();
        assert_eq!(
            wire[0], 0x24,
            "Modify command byte must be protocol literal 0x24"
        );
        assert_eq!(
            wire[1], b'{',
            "JSON must start immediately after the command byte"
        );
        let _ = payload_object(&wire);
    }

    #[test]
    fn stop_order_wire_starts_with_0x25_then_single_json_object() {
        let wire = sample_stop_order().to_wire().unwrap();
        assert_eq!(
            wire[0], 0x25,
            "StopOrder command byte must be protocol literal 0x25"
        );
        assert_eq!(
            wire[1], b'{',
            "JSON must start immediately after the command byte"
        );
        let _ = payload_object(&wire);
    }

    // ---- OrderModel: string-vs-number (the core spot/perp wire difference) ----

    #[test]
    fn order_price_quantity_are_json_strings_and_enums_are_numbers() {
        let body = payload_object(&sample_order("90000", "0.5", None).to_wire().unwrap());
        assert_eq!(body["price"], Value::String("90000".to_string()));
        assert_eq!(body["quantity"], Value::String("0.5".to_string()));
        assert!(
            body["side"].is_u64(),
            "side must be an unquoted JSON number"
        );
        assert_eq!(body["side"], serde_json::json!(0));
        assert!(
            body["orderType"].is_u64(),
            "orderType must be an unquoted JSON number"
        );
        assert_eq!(body["orderType"], serde_json::json!(1));
        assert!(
            body["orderMode"].is_u64(),
            "orderMode must be an unquoted JSON number"
        );
        assert_eq!(body["orderMode"], serde_json::json!(2));
    }

    #[test]
    fn order_model_passes_price_strings_through_verbatim_without_validation() {
        let body = payload_object(&sample_order("1.000", "0.500", None).to_wire().unwrap());
        assert_eq!(
            body["price"],
            Value::String("1.000".to_string()),
            "trailing zeros must survive untouched"
        );
        assert_eq!(body["quantity"], Value::String("0.500".to_string()));

        let wire = sample_order("", "0.5", None)
            .to_wire()
            .expect("empty price must not be rejected at the model layer");
        let body = payload_object(&wire);
        assert_eq!(body["price"], Value::String(String::new()));
    }

    // ---- OrderModel: camelCase keys + tpsl inclusion/omission ----

    #[test]
    fn order_wire_keys_are_exact_camel_case_set_and_tpsl_none_is_omitted() {
        let body = payload_object(&sample_order("90000", "0.5", None).to_wire().unwrap());
        assert_eq!(
            sorted_keys(&body),
            vec![
                "baseToken",
                "l1owner",
                "orderMode",
                "orderType",
                "price",
                "quantity",
                "quoteToken",
                "side"
            ]
        );
        // Both directions: snake_case variants must be absent.
        for snake in ["base_token", "quote_token", "order_type", "order_mode"] {
            assert!(
                !body.contains_key(snake),
                "snake_case key {snake} leaked to the wire"
            );
        }
        assert!(
            !body.contains_key("tpsl"),
            "tpsl: None must omit the key entirely (a null would be rejected by the server)"
        );
    }

    #[test]
    fn order_tpsl_with_only_tp_limit_serializes_single_tp_limit_key() {
        let tpsl = TpslModel {
            tp_limit: Some("91000".to_string()),
            sl_trigger: None,
            sl_limit: None,
        };
        let body = payload_object(&sample_order("90000", "0.5", Some(tpsl)).to_wire().unwrap());
        let tpsl_obj = body["tpsl"]
            .as_object()
            .expect("tpsl must serialize as a JSON object");
        assert_eq!(sorted_keys(tpsl_obj), vec!["tpLimit"]);
        assert_eq!(tpsl_obj["tpLimit"], Value::String("91000".to_string()));
    }

    // ---- TpslModel::to_wire (hand-built map, not derived) ----

    #[test]
    fn tpsl_to_wire_all_none_is_empty_object() {
        let wire = TpslModel {
            tp_limit: None,
            sl_trigger: None,
            sl_limit: None,
        }
        .to_wire();
        assert_eq!(serde_json::to_string(&wire).unwrap(), "{}");
    }

    #[test]
    fn tpsl_to_wire_all_some_maps_each_field_to_its_own_camel_case_key() {
        let wire = TpslModel {
            tp_limit: Some("111".to_string()),
            sl_trigger: Some("222".to_string()),
            sl_limit: Some("333".to_string()),
        }
        .to_wire();
        let obj = wire.as_object().expect("tpsl wire must be a JSON object");
        assert_eq!(sorted_keys(obj), vec!["slLimit", "slTrigger", "tpLimit"]);
        // Distinct values prove there is no field/key cross-wiring.
        assert_eq!(obj["tpLimit"], Value::String("111".to_string()));
        assert_eq!(obj["slTrigger"], Value::String("222".to_string()));
        assert_eq!(obj["slLimit"], Value::String("333".to_string()));
    }

    #[test]
    fn tpsl_to_wire_some_empty_string_keeps_key_unlike_none() {
        let wire = TpslModel {
            tp_limit: Some(String::new()),
            sl_trigger: None,
            sl_limit: None,
        }
        .to_wire();
        let obj = wire.as_object().expect("tpsl wire must be a JSON object");
        assert_eq!(sorted_keys(obj), vec!["tpLimit"]);
        assert_eq!(obj["tpLimit"], Value::String(String::new()));
    }

    // ---- Modify / Cancel / CancelAll exact key sets ----

    #[test]
    fn modify_wire_uses_spot_new_qty_spelling_with_string_price_and_qty() {
        let model = ModifyModel {
            l1owner: "0xowner".to_string(),
            order_id: "oid-1".to_string(),
            new_price: "51000".to_string(),
            new_qty: "2".to_string(),
            order_mode: 1,
        };
        let body = payload_object(&model.to_wire().unwrap());
        assert_eq!(
            sorted_keys(&body),
            vec!["l1owner", "newPrice", "newQty", "orderId", "orderMode"]
        );
        assert!(
            !body.contains_key("newQuantity"),
            "spot must use newQty; newQuantity is the perp spelling"
        );
        assert_eq!(body["newPrice"], Value::String("51000".to_string()));
        assert_eq!(body["newQty"], Value::String("2".to_string()));
    }

    #[test]
    fn cancel_wire_has_exactly_l1owner_and_order_id_keys() {
        let model = CancelModel {
            l1owner: "0xowner".to_string(),
            order_id: "oid-1".to_string(),
        };
        let body = payload_object(&model.to_wire().unwrap());
        assert_eq!(sorted_keys(&body), vec!["l1owner", "orderId"]);
        assert!(
            !body.contains_key("marketId"),
            "marketId is perp-only and must not leak into spot cancel"
        );
    }

    #[test]
    fn cancel_all_wire_has_exactly_one_l1owner_key() {
        let model = CancelAllModel {
            l1owner: "0xowner".to_string(),
        };
        let body = payload_object(&model.to_wire().unwrap());
        assert_eq!(sorted_keys(&body), vec!["l1owner"]);
    }

    // ---- StopOrder structural independence from OrderModel ----

    #[test]
    fn stop_order_and_order_key_sets_differ_exactly_by_stop_price_vs_tpsl() {
        let stop_body = payload_object(&sample_stop_order().to_wire().unwrap());
        assert_eq!(
            sorted_keys(&stop_body),
            vec![
                "baseToken",
                "l1owner",
                "orderMode",
                "orderType",
                "price",
                "quantity",
                "quoteToken",
                "side",
                "stopPrice"
            ]
        );
        assert_eq!(stop_body["stopPrice"], Value::String("95000".to_string()));
        assert!(
            !stop_body.contains_key("stop_price"),
            "snake_case stop_price must not leak"
        );
        assert!(
            !stop_body.contains_key("tpsl"),
            "StopOrder must never carry a tpsl key"
        );

        let tpsl = TpslModel {
            tp_limit: Some("91000".to_string()),
            sl_trigger: None,
            sl_limit: None,
        };
        let order_body =
            payload_object(&sample_order("90000", "0.5", Some(tpsl)).to_wire().unwrap());
        assert!(
            order_body.contains_key("tpsl"),
            "Order carries tpsl when provided"
        );
        assert!(
            !order_body.contains_key("stopPrice"),
            "Order must never carry stopPrice"
        );
    }

    // ---- Transfer / Session models (serde_json::Value returns) ----

    #[test]
    fn transfer_wires_have_exact_key_sets_and_string_value() {
        let value_wire = ValueTransferModel {
            l1owner: "0xowner".to_string(),
            to: "0xrecipient".to_string(),
            value: "1.5".to_string(),
        }
        .to_wire();
        let value_obj = value_wire
            .as_object()
            .expect("value transfer wire must be an object");
        assert_eq!(sorted_keys(value_obj), vec!["l1owner", "to", "value"]);
        assert_eq!(value_obj["value"], Value::String("1.5".to_string()));

        let token_wire = TokenTransferModel {
            l1owner: "0xowner".to_string(),
            to: "0xrecipient".to_string(),
            value: "100".to_string(),
            token: "USDT".to_string(),
        }
        .to_wire();
        let token_obj = token_wire
            .as_object()
            .expect("token transfer wire must be an object");
        assert_eq!(
            sorted_keys(token_obj),
            vec!["l1owner", "to", "token", "value"]
        );
        assert_eq!(token_obj["value"], Value::String("100".to_string()));
    }

    #[test]
    fn session_context_wire_renames_type_and_expires_at_and_omits_none_metadata() {
        let model = SessionContextModel {
            r#type: 1,
            publickey: "0xpub".to_string(),
            expires_at: 1_700_000_000,
            nonce: 7,
            l1owner: "0xowner".to_string(),
            l1signature: "0xsig".to_string(),
            metadata: None,
        };
        let wire = model.to_wire();
        let obj = wire
            .as_object()
            .expect("session wire must be a JSON object");
        assert_eq!(
            sorted_keys(obj),
            vec![
                "expiresAt",
                "l1owner",
                "l1signature",
                "nonce",
                "publickey",
                "type"
            ]
        );
        assert!(
            !obj.contains_key("expires_at"),
            "snake_case expires_at must not leak"
        );
        assert!(
            !obj.contains_key("metadata"),
            "metadata: None must omit the key entirely"
        );
    }

    #[test]
    fn session_context_metadata_some_empty_string_keeps_key() {
        let model = SessionContextModel {
            r#type: 1,
            publickey: "0xpub".to_string(),
            expires_at: 1_700_000_000,
            nonce: 7,
            l1owner: "0xowner".to_string(),
            l1signature: "0xsig".to_string(),
            metadata: Some(String::new()),
        };
        let wire = model.to_wire();
        let obj = wire
            .as_object()
            .expect("session wire must be a JSON object");
        assert!(
            obj.contains_key("metadata"),
            "Some(\"\") must keep the metadata key"
        );
        assert_eq!(obj["metadata"], Value::String(String::new()));
    }
}
