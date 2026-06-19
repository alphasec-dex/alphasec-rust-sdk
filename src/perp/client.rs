//! REST client for the AlphaSec perpetual futures API (/fapi/v1/*)
//!
//! All signed-transaction endpoints share a single `submit` helper (POST body = {"tx": ...},
//! response result = tx hash string).  All read endpoints share a single `get_json` helper
//! (GET with query params, response result deserialized via serde).
//!
//! Transport (reqwest client + base URL) is constructed from `Config` exactly as `ApiClient`
//! does in `src/api/client.rs`.

use crate::{
    error::{AlphaSecError, Result},
    perp::types::{
        FundingItem, MarketsResponse, PerpAccount, PerpCandle, PerpDepth, PerpFill,
        PerpFundingQuery, PerpHistoryQuery, PerpMarket, PerpOrder, PerpOrderQuery, PerpTicker,
        PerpTrade, Position, PositionHistory, PositionSetting,
    },
    signer::Config,
};
use reqwest::Client as HttpClient;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;
use tracing::debug;

/// HTTP client for all `/fapi/v1` REST endpoints.
///
/// Constructed from a `Config` reference — reuses the same HTTP transport settings
/// (timeout, Content-Type header) that `ApiClient` uses for spot.
#[derive(Debug, Clone)]
pub struct PerpApiClient {
    /// Shared reqwest HTTP client
    http_client: HttpClient,
    /// Base URL (scheme + host, no trailing slash)
    base_url: String,
}

impl PerpApiClient {
    /// Create a new `PerpApiClient` from a `Config`.
    ///
    /// Mirrors `ApiClient::new` — same timeout, same Content-Type default header.
    pub fn new(config: &Config) -> Result<Self> {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::CONTENT_TYPE,
                    reqwest::header::HeaderValue::from_static("application/json"),
                );
                headers
            })
            .build()
            .map_err(|e| AlphaSecError::config(format!("Failed to create HTTP client: {}", e)))?;

        // Strip trailing slash so all path concatenations are uniform.
        let base_url = config.api_url.to_string().trim_end_matches('/').to_string();

        Ok(Self {
            http_client,
            base_url,
        })
    }

    // -------------------------------------------------------------------------
    // Private transport helpers
    // -------------------------------------------------------------------------

    /// Build a full URL from a `/fapi/v1/...` path.
    fn url(&self, path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}/{}", self.base_url, path)
        }
    }

    /// POST `{"tx": signed_tx}` to `path`; return the `result` field as a String
    /// (tx hash on success).  Uses the `{code, errMsg, result}` envelope.
    async fn submit(&self, path: &str, signed_tx: &str) -> Result<String> {
        let url = self.url(path);
        debug!("POST {}", url);

        let body = serde_json::json!({ "tx": signed_tx });
        let response = self
            .http_client
            .post(&url)
            .body(body.to_string())
            .send()
            .await?;

        if !response.status().is_success() {
            let code = response.status().as_u16() as i32;
            let text = response.text().await.unwrap_or_default();
            return Err(AlphaSecError::api(code, text));
        }

        let json: Value = response.json().await?;
        let code = json["code"].as_i64().unwrap_or(0);
        if code != 200 {
            let msg = json["errMsg"]
                .as_str()
                .unwrap_or("unknown error")
                .to_string();
            return Err(AlphaSecError::api(code as i32, msg));
        }

        // result is a tx-hash string
        let hash = match &json["result"] {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        Ok(hash)
    }

    /// GET `path` with `params` query pairs; deserialize `result` into `T`.
    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T> {
        // Build URL with query string (same encoding logic as ApiClient::get)
        let mut url = self.url(path);
        if !params.is_empty() {
            url.push('?');
            for (i, (key, value)) in params.iter().enumerate() {
                if i > 0 {
                    url.push('&');
                }
                url.push_str(&format!(
                    "{}={}",
                    urlencoding::encode(key),
                    urlencoding::encode(value)
                ));
            }
        }

        debug!("GET {}", url);
        let response = self.http_client.get(&url).send().await?;

        if !response.status().is_success() {
            let code = response.status().as_u16() as i32;
            let text = response.text().await.unwrap_or_default();
            return Err(AlphaSecError::api(code, text));
        }

        let json: Value = response.json().await?;
        decode_envelope(json)
    }

    // -------------------------------------------------------------------------
    // POST endpoints (signed transactions)
    // -------------------------------------------------------------------------

    /// Submit a new perp order.  POST /fapi/v1/order → tx hash.
    pub async fn order(&self, signed_tx: &str) -> Result<String> {
        self.submit("/fapi/v1/order", signed_tx).await
    }

    /// Cancel a perp order.  POST /fapi/v1/order/cancel → tx hash.
    pub async fn cancel(&self, signed_tx: &str) -> Result<String> {
        self.submit("/fapi/v1/order/cancel", signed_tx).await
    }

    /// Cancel all perp orders for a market.  POST /fapi/v1/order/cancel/all → tx hash.
    pub async fn cancel_all(&self, signed_tx: &str) -> Result<String> {
        self.submit("/fapi/v1/order/cancel/all", signed_tx).await
    }

    /// Modify (amend) an open perp order.  POST /fapi/v1/order/modify → tx hash.
    pub async fn modify(&self, signed_tx: &str) -> Result<String> {
        self.submit("/fapi/v1/order/modify", signed_tx).await
    }

    /// Deposit from Spot wallet to Perp wallet (0x12).  POST /fapi/v1/wallet/deposit → tx hash.
    pub async fn deposit(&self, signed_tx: &str) -> Result<String> {
        self.submit("/fapi/v1/wallet/deposit", signed_tx).await
    }

    /// Withdraw from Perp wallet to Spot wallet (0x44).  POST /fapi/v1/wallet/withdraw → tx hash.
    pub async fn withdraw(&self, signed_tx: &str) -> Result<String> {
        self.submit("/fapi/v1/wallet/withdraw", signed_tx).await
    }

    /// Set leverage for a market (0x45).  POST /fapi/v1/position/leverage → tx hash.
    pub async fn set_leverage(&self, signed_tx: &str) -> Result<String> {
        self.submit("/fapi/v1/position/leverage", signed_tx).await
    }

    // -------------------------------------------------------------------------
    // GET endpoints (account / position)
    // -------------------------------------------------------------------------

    /// Get perp account balances and risk aggregates.
    /// GET /fapi/v1/wallet/account?address=
    pub async fn get_account(&self, address: &str) -> Result<PerpAccount> {
        self.get_json("/fapi/v1/wallet/account", &[("address", address)])
            .await
    }

    /// Get all open positions.
    /// GET /fapi/v1/position?address=
    ///
    /// The server wraps positions in `{ "positions": [...], "degraded": ... }`. We extract the
    /// inner list (tolerating a bare array for backward / rollback compatibility) and drop
    /// `degraded` — the per-position nullable risk fields already carry per-market degradation.
    pub async fn get_positions(&self, address: &str) -> Result<Vec<Position>> {
        let result: Value = self
            .get_json("/fapi/v1/position", &[("address", address)])
            .await?;
        parse_positions(result)
    }

    /// Get position lifecycle history with optional pagination/filter.
    /// GET /fapi/v1/position/history
    pub async fn get_position_history(
        &self,
        address: &str,
        q: &PerpHistoryQuery,
    ) -> Result<Vec<PositionHistory>> {
        let params = build_history_params(address, q);
        let kv: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_json("/fapi/v1/position/history", &kv).await
    }

    /// Get per-market leverage / margin-mode settings.
    /// GET /fapi/v1/position/settings?address=
    pub async fn get_position_settings(&self, address: &str) -> Result<Vec<PositionSetting>> {
        self.get_json("/fapi/v1/position/settings", &[("address", address)])
            .await
    }

    /// Get funding payment history with optional pagination/filter.
    /// GET /fapi/v1/wallet/funding
    pub async fn get_funding(
        &self,
        address: &str,
        q: &PerpFundingQuery,
    ) -> Result<Vec<FundingItem>> {
        let params = build_funding_params(address, q);
        let kv: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_json("/fapi/v1/wallet/funding", &kv).await
    }

    // -------------------------------------------------------------------------
    // GET endpoints (orders)
    // -------------------------------------------------------------------------

    /// Get open orders with optional pagination/filter.
    /// GET /fapi/v1/order/open
    pub async fn get_open_orders(
        &self,
        address: &str,
        q: &PerpOrderQuery,
    ) -> Result<Vec<PerpOrder>> {
        let kv = build_order_params(address, q);
        let borrowed: Vec<(&str, &str)> = kv.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_json("/fapi/v1/order/open", &borrowed).await
    }

    /// Get order history (filled / cancelled) with optional pagination/filter.
    /// GET /fapi/v1/order
    pub async fn get_order_history(
        &self,
        address: &str,
        q: &PerpOrderQuery,
    ) -> Result<Vec<PerpOrder>> {
        let kv = build_order_params(address, q);
        let borrowed: Vec<(&str, &str)> = kv.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_json("/fapi/v1/order", &borrowed).await
    }

    /// Get a single order by ID.
    /// GET /fapi/v1/order/{id}
    pub async fn get_order(&self, order_id: &str) -> Result<PerpOrder> {
        let path = format!("/fapi/v1/order/{}", order_id);
        self.get_json(&path, &[]).await
    }

    /// Get orders submitted in a given transaction (by tx hash).
    /// GET /fapi/v1/order/list?txHash=
    pub async fn get_order_list(&self, tx_hash: &str) -> Result<Vec<PerpOrder>> {
        self.get_json("/fapi/v1/order/list", &[("txHash", tx_hash)])
            .await
    }

    /// Get personal trade history (fills) with optional pagination/filter.
    /// GET /fapi/v1/order/trade
    pub async fn get_my_trades(&self, address: &str, q: &PerpOrderQuery) -> Result<Vec<PerpFill>> {
        let kv = build_order_params(address, q);
        let borrowed: Vec<(&str, &str)> = kv.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_json("/fapi/v1/order/trade", &borrowed).await
    }

    // -------------------------------------------------------------------------
    // GET endpoints (market data)
    // -------------------------------------------------------------------------

    /// Get all perp markets.  Unwraps `MarketsResponse.symbols`.
    /// GET /fapi/v1/market
    pub async fn get_markets(&self) -> Result<Vec<PerpMarket>> {
        let resp: MarketsResponse = self.get_json("/fapi/v1/market", &[]).await?;
        Ok(resp.symbols)
    }

    /// Get tickers for all markets.
    /// GET /fapi/v1/market/ticker
    pub async fn get_tickers(&self) -> Result<Vec<PerpTicker>> {
        self.get_json("/fapi/v1/market/ticker", &[]).await
    }

    /// Get ticker(s) filtered by market ID.  Returns the full array (server returns 1 element).
    /// GET /fapi/v1/market/ticker?marketId=
    pub async fn get_ticker(&self, market_id: &str) -> Result<Vec<PerpTicker>> {
        self.get_json("/fapi/v1/market/ticker", &[("marketId", market_id)])
            .await
    }

    /// Get order book depth snapshot.
    /// GET /fapi/v1/market/depth?marketId=&limit=
    pub async fn get_depth(&self, market_id: &str, limit: Option<u32>) -> Result<PerpDepth> {
        let params = build_default_limit_params(market_id, limit);
        let kv: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_json("/fapi/v1/market/depth", &kv).await
    }

    /// Get recent public trades.
    /// GET /fapi/v1/market/trades?marketId=&limit=
    pub async fn get_market_trades(
        &self,
        market_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PerpTrade>> {
        let params = build_default_limit_params(market_id, limit);
        let kv: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_json("/fapi/v1/market/trades", &kv).await
    }

    /// Get OHLCV candles. `result` is a JSON array of bar objects (see [`PerpCandle`]).
    /// GET /fapi/v1/market/candles?marketId=&resolution=&from=&to=
    pub async fn get_candles(
        &self,
        market_id: &str,
        resolution: &str,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<Vec<PerpCandle>> {
        let params = build_candle_params(market_id, resolution, from, to);
        let kv: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_json("/fapi/v1/market/candles", &kv).await
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Build the shared `address + PerpOrderQuery` parameter list.
fn build_order_params<'a>(address: &'a str, q: &'a PerpOrderQuery) -> Vec<(&'a str, String)> {
    let mut params: Vec<(&str, String)> = vec![("address", address.to_string())];
    if let Some(ref mid) = q.market_id {
        params.push(("marketId", mid.clone()));
    }
    if let Some(f) = q.from {
        params.push(("from", f.to_string()));
    }
    if let Some(t) = q.to {
        params.push(("to", t.to_string()));
    }
    if let Some(id) = q.last_id {
        params.push(("lastID", id.to_string()));
    }
    if let Some(l) = q.limit {
        params.push(("limit", l.to_string()));
    }
    params
}

/// Build the `/position/history` parameter list (no `lastID` — offset paging unsupported here).
fn build_history_params<'a>(address: &'a str, q: &'a PerpHistoryQuery) -> Vec<(&'a str, String)> {
    let mut params: Vec<(&str, String)> = vec![("address", address.to_string())];
    if let Some(ref mid) = q.market_id {
        params.push(("marketId", mid.clone()));
    }
    if let Some(f) = q.from {
        params.push(("from", f.to_string()));
    }
    if let Some(t) = q.to {
        params.push(("to", t.to_string()));
    }
    if let Some(l) = q.limit {
        params.push(("limit", l.to_string()));
    }
    params
}

/// Build the `/wallet/funding` parameter list (includes uppercase `lastID` cursor).
fn build_funding_params<'a>(address: &'a str, q: &'a PerpFundingQuery) -> Vec<(&'a str, String)> {
    let mut params: Vec<(&str, String)> = vec![("address", address.to_string())];
    if let Some(ref mid) = q.market_id {
        params.push(("marketId", mid.clone()));
    }
    if let Some(f) = q.from {
        params.push(("from", f.to_string()));
    }
    if let Some(t) = q.to {
        params.push(("to", t.to_string()));
    }
    if let Some(id) = q.last_id {
        params.push(("lastID", id.to_string()));
    }
    if let Some(l) = q.limit {
        params.push(("limit", l.to_string()));
    }
    params
}

/// Server-side fallback page size applied when the caller passes `limit = None`.
const DEFAULT_LIMIT: u32 = 100;

/// Build the `marketId` + `limit` parameter list for `/market/depth` and `/market/trades`.
/// `limit` is ALWAYS present: `None` falls back to [`DEFAULT_LIMIT`] (unlike the optional
/// paging params, which are omitted when absent).
fn build_default_limit_params<'a>(
    market_id: &'a str,
    limit: Option<u32>,
) -> Vec<(&'a str, String)> {
    vec![
        ("marketId", market_id.to_string()),
        ("limit", limit.unwrap_or(DEFAULT_LIMIT).to_string()),
    ]
}

/// Build the `/market/candles` parameter list. `marketId` + `resolution` are always present;
/// `from`/`to` are omitted when `None`.
fn build_candle_params<'a>(
    market_id: &'a str,
    resolution: &'a str,
    from: Option<i64>,
    to: Option<i64>,
) -> Vec<(&'a str, String)> {
    let mut params: Vec<(&str, String)> = vec![
        ("marketId", market_id.to_string()),
        ("resolution", resolution.to_string()),
    ];
    if let Some(f) = from {
        params.push(("from", f.to_string()));
    }
    if let Some(t) = to {
        params.push(("to", t.to_string()));
    }
    params
}

/// Decode the `{code, errMsg, result}` envelope into `T`.
///
/// `code != 200` maps to `AlphaSecError::Api`; otherwise `result` is deserialized into `T`.
/// Pure (no transport) so the envelope contract can be unit-tested offline.
fn decode_envelope<T: DeserializeOwned>(json: Value) -> Result<T> {
    let code = json["code"].as_i64().unwrap_or(0);
    if code != 200 {
        let msg = json["errMsg"]
            .as_str()
            .unwrap_or("unknown error")
            .to_string();
        return Err(AlphaSecError::api(code as i32, msg));
    }
    let result = serde_json::from_value(json["result"].clone()).map_err(AlphaSecError::Json)?;
    Ok(result)
}

/// Parse the `/fapi/v1/position` result into the position list.
///
/// The server returns `{ "positions": [...], "degraded": ... }`; we extract the inner list and
/// tolerate a bare `[...]` array (old shape / rollback). `degraded` is intentionally dropped —
/// the per-position nullable risk fields already carry per-market degradation. Pure (no
/// transport) so the envelope contract can be unit-tested offline.
fn parse_positions(result: Value) -> Result<Vec<Position>> {
    // Wrapped object → take `positions`; bare array → use as-is. Branching on the JSON shape
    // (rather than a `#[serde(untagged)]` enum) keeps `rust_decimal::serde::str` field decoding
    // working — untagged buffering can't drive that decoder — and surfaces real per-field errors
    // instead of a generic "no variant matched". Mirrors Python's `isinstance(dict)` unwrap.
    let list = if result.is_object() {
        result
            .get("positions")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()))
    } else {
        result
    };
    serde_json::from_value(list).map_err(AlphaSecError::Json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perp::types::{PerpAccount, PerpFundingQuery, PerpHistoryQuery, PerpOrderQuery};

    /// Look up the value for `key` in a built param list, or None if the key is absent.
    fn get(params: &[(&str, String)], key: &str) -> Option<String> {
        params
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.clone())
    }

    fn test_client() -> PerpApiClient {
        // Real Config + real reqwest client; only the pure `url()` join logic is exercised.
        let config = Config::new(
            "https://api-testnet.alphasec.trade",
            "kairos",
            "0x0000000000000000000000000000000000000001",
            None,
            None,
            false,
            None,
        )
        .expect("config");
        PerpApiClient::new(&config).expect("client")
    }

    // -- Query builders --------------------------------------------------------

    #[test]
    fn order_params_exact_key_spelling_and_value_passthrough() {
        let q = PerpOrderQuery {
            market_id: Some("7".to_string()),
            from: Some(100),
            to: Some(200),
            last_id: Some(42),
            limit: Some(25),
        };
        let p = build_order_params("0xabc", &q);
        assert_eq!(get(&p, "address").as_deref(), Some("0xabc"));
        assert_eq!(get(&p, "marketId").as_deref(), Some("7"));
        assert_eq!(get(&p, "from").as_deref(), Some("100"));
        assert_eq!(get(&p, "to").as_deref(), Some("200"));
        // Uppercase D is the contract — `lastId` would not match.
        assert_eq!(get(&p, "lastID").as_deref(), Some("42"));
        assert!(
            get(&p, "lastId").is_none(),
            "must not emit camelCase lastId"
        );
        assert_eq!(get(&p, "limit").as_deref(), Some("25"));
    }

    #[test]
    fn order_params_omit_all_none_optionals_address_only() {
        let q = PerpOrderQuery::default();
        let p = build_order_params("0xabc", &q);
        assert_eq!(
            p.len(),
            1,
            "empty query must yield address only, got {:?}",
            p
        );
        assert_eq!(p[0].0, "address");
    }

    #[test]
    fn funding_params_use_uppercase_lastID() {
        let q = PerpFundingQuery {
            last_id: Some(99),
            ..Default::default()
        };
        let p = build_funding_params("0xabc", &q);
        assert_eq!(get(&p, "lastID").as_deref(), Some("99"));
        assert!(get(&p, "lastId").is_none());
    }

    #[test]
    fn funding_params_omit_none_optionals_address_only() {
        let q = PerpFundingQuery::default();
        let p = build_funding_params("0xabc", &q);
        assert_eq!(p.len(), 1, "got {:?}", p);
        assert_eq!(p[0], ("address", "0xabc".to_string()));
    }

    #[test]
    fn history_params_never_emit_lastID() {
        let q = PerpHistoryQuery {
            market_id: Some("3".to_string()),
            from: Some(1),
            to: Some(2),
            limit: Some(10),
        };
        let p = build_history_params("0xabc", &q);
        assert!(
            get(&p, "lastID").is_none() && get(&p, "lastId").is_none(),
            "history must not emit any lastID cursor: {:?}",
            p
        );
        // sanity: the keys it *does* carry are spelled correctly
        assert_eq!(get(&p, "marketId").as_deref(), Some("3"));
        assert_eq!(get(&p, "limit").as_deref(), Some("10"));
    }

    #[test]
    fn candle_params_required_keys_present_and_range_omitted_when_none() {
        let none_range = build_candle_params("7", "60", None, None);
        assert_eq!(get(&none_range, "marketId").as_deref(), Some("7"));
        assert_eq!(get(&none_range, "resolution").as_deref(), Some("60"));
        assert!(get(&none_range, "from").is_none(), "from must be omitted");
        assert!(get(&none_range, "to").is_none(), "to must be omitted");

        let with_range = build_candle_params("7", "60", Some(1000), Some(2000));
        assert_eq!(get(&with_range, "from").as_deref(), Some("1000"));
        assert_eq!(get(&with_range, "to").as_deref(), Some("2000"));
    }

    #[test]
    fn default_limit_params_always_present_and_default_100() {
        let defaulted = build_default_limit_params("7", None);
        assert_eq!(
            get(&defaulted, "limit").as_deref(),
            Some("100"),
            "None limit must fall back to 100"
        );
        let explicit = build_default_limit_params("7", Some(7));
        assert_eq!(get(&explicit, "limit").as_deref(), Some("7"));
        assert_eq!(get(&explicit, "marketId").as_deref(), Some("7"));
    }

    // -- URL join --------------------------------------------------------------

    #[test]
    fn url_join_no_double_slash_after_host() {
        let c = test_client();
        let u = c.url("/fapi/v1/market");
        assert_eq!(u, "https://api-testnet.alphasec.trade/fapi/v1/market");
        // exactly the scheme's `//` and no other `//`
        assert_eq!(
            u.matches("//").count(),
            1,
            "unexpected doubled slash in {}",
            u
        );
    }

    #[test]
    fn url_join_inserts_slash_for_pathless_input() {
        let c = test_client();
        let u = c.url("fapi/v1/market");
        assert_eq!(u, "https://api-testnet.alphasec.trade/fapi/v1/market");
    }

    // -- Envelope decode -------------------------------------------------------

    #[test]
    fn envelope_non_200_maps_to_api_error_with_code_and_msg() {
        let json = serde_json::json!({ "code": 400, "errMsg": "bad request", "result": null });
        let r: Result<PerpAccount> = decode_envelope(json);
        match r {
            Err(AlphaSecError::Api { code, message }) => {
                assert_eq!(code, 400);
                assert_eq!(message, "bad request");
            }
            other => panic!("expected Api error, got {:?}", other),
        }
    }

    #[test]
    fn envelope_missing_code_is_treated_as_error_not_success() {
        let json = serde_json::json!({ "result": { "anything": 1 } });
        let r: Result<PerpAccount> = decode_envelope(json);
        assert!(
            matches!(r, Err(AlphaSecError::Api { code: 0, .. })),
            "missing code must not be treated as 200"
        );
    }
}
