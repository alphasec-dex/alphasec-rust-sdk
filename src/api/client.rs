//! API client for AlphaSec REST API

use crate::{
    error::{AlphaSecError, Result},
    signer::{AlphaSecSigner, Config},
    types::{market::*, orders::*, account::*, api::*},
};
use reqwest::Client as HttpClient;
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, info};

/// AlphaSec API client
#[derive(Debug, Clone)]
pub struct ApiClient {
    /// HTTP client
    http_client: HttpClient,
    /// Base API URL
    base_url: String,
    /// Signer for authenticated requests
    signer: Option<AlphaSecSigner>,
    /// Token metadata for conversions
    token_metadata: Option<TokenMetadata>,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(config: &Config, signer: Option<AlphaSecSigner>) -> Result<Self> {
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

        Ok(Self {
            http_client,
            base_url: config.api_url.to_string(),
            signer,
            token_metadata: None,
        })
    }

    /// Initialize token metadata
    pub async fn initialize_metadata(&mut self) -> Result<()> {
        let tokens = self.get_tokens().await?;
        self.token_metadata = Some(TokenMetadata::from_tokens(&tokens));
        info!("✅ Token metadata initialized with {} tokens", tokens.len());
        Ok(())
    }

    /// Get token metadata
    pub fn token_metadata(&self) -> Option<&TokenMetadata> {
        self.token_metadata.as_ref()
    }

    /// Make a GET request
    async fn get(&self, path: &str, params: Option<&[(&str, &str)]>) -> Result<Value> {
        let mut url = if self.base_url.ends_with('/') && path.starts_with('/') {
            format!("{}{}", self.base_url.trim_end_matches('/'), path)
        } else if !self.base_url.ends_with('/') && !path.starts_with('/') {
            format!("{}/{}", self.base_url, path)
        } else {
            format!("{}{}", self.base_url, path)
        };
        
        if let Some(params) = params {
            if !params.is_empty() {
                url.push('?');
                for (i, (key, value)) in params.iter().enumerate() {
                    if i > 0 {
                        url.push('&');
                    }
                    url.push_str(&format!("{}={}", 
                        urlencoding::encode(key), 
                        urlencoding::encode(value)
                    ));
                }
            }
        }

        debug!("GET {}", url);
        let response = self.http_client.get(&url).send().await?;
        
        if response.status().is_success() {
            let json: Value = response.json().await?;
            Ok(json)
        } else {
            let status_code = response.status().as_u16() as i32;
            let error_text = response.text().await.unwrap_or_default();
            Err(AlphaSecError::api(status_code, error_text))
        }
    }

    /// Make a POST request
    async fn post(&self, path: &str, params: Option<Value>) -> Result<Value> {
        let url = if self.base_url.ends_with('/') && path.starts_with('/') {
            format!("{}{}", self.base_url.trim_end_matches('/'), path)
        } else if !self.base_url.ends_with('/') && !path.starts_with('/') {
            format!("{}/{}", self.base_url, path)
        } else {
            format!("{}{}", self.base_url, path)
        };
        
        debug!("POST {} with params: {:?}", url, params);
        let mut request = self.http_client
            .post(&url)
            .header("Content-Type", "application/json");
        
        if let Some(params) = params {
            request = request.body(params.to_string());
        }
        
        info!("🔍 Request: {:?}", request);
        let response = request.send().await?;
        
        if response.status().is_success() {
            let json: Value = response.json().await?;
            Ok(json)
        } else {
            let status_code = response.status().as_u16() as i32;
            let error_text = response.text().await.unwrap_or_default();
            Err(AlphaSecError::api(status_code, error_text))
        }
    }

    // === Public Market Data API ===

    /// Get all markets
    pub async fn get_market_list(&self) -> Result<Vec<Market>> {
        let response = self.get("/api/v1/market", None).await?;
        let markets = response["result"]
            .as_array()
            .ok_or_else(|| AlphaSecError::api(500, "Invalid market list response format"))?
            .iter()
            .map(|market| serde_json::from_value(market.clone()).map_err(AlphaSecError::Json))
            .collect::<Result<Vec<Market>>>()?;
        Ok(markets)
    }

    /// Get all tickers
    pub async fn get_tickers(&self) -> Result<Vec<Ticker>> {
        let response = self.get("/api/v1/market/ticker", None).await?;
        let tickers = response["result"]
            .as_array()
            .ok_or_else(|| AlphaSecError::api(500, "Invalid tickers response format"))?
            .iter()
            .map(|ticker| serde_json::from_value(ticker.clone()).map_err(AlphaSecError::Json))
            .collect::<Result<Vec<Ticker>>>()?;
        Ok(tickers)
    }

    /// Get ticker for specific market
    pub async fn get_ticker(&self, market: &str) -> Result<Ticker> {
        let market_id = if let Some(metadata) = &self.token_metadata {
            metadata.market_to_market_id(market)?
        } else {
            market.to_string()
        };

        let params = [("marketId", market_id.as_str())];
        let response = self.get("/api/v1/market/ticker", Some(&params)).await?;
        
        let ticker_array = response["result"]
            .as_array()
            .ok_or_else(|| AlphaSecError::api(500, "Invalid ticker response format"))?;
        
        if ticker_array.is_empty() {
            return Err(AlphaSecError::not_found(format!("Ticker not found for market: {}", market)));
        }
        
        let ticker = serde_json::from_value(ticker_array[0].clone()).map_err(AlphaSecError::Json)?;
        Ok(ticker)
    }

    /// Get all tokens
    pub async fn get_tokens(&self) -> Result<Vec<Token>> {
        let response = self.get("/api/v1/market/tokens", None).await?;
        let tokens = response["result"]
            .as_array()
            .ok_or_else(|| AlphaSecError::api(500, "Invalid tokens response format"))?
            .iter()
            .map(|token| serde_json::from_value(token.clone()).map_err(AlphaSecError::Json))
            .collect::<Result<Vec<Token>>>()?;
        Ok(tokens)
    }

    /// Get recent trades
    pub async fn get_trades(&self, market: &str, limit: Option<u32>) -> Result<Vec<Trade>> {
        let market_id = if let Some(metadata) = &self.token_metadata {
            metadata.market_to_market_id(market)?
        } else {
            market.to_string()
        };

        let limit_str = limit.unwrap_or(100).to_string();
        let params = [
            ("marketId", market_id.as_str()),
            ("limit", limit_str.as_str()),
        ];
        let response = self.get("/api/v1/market/trades", Some(&params)).await?;
        
        let trades = response["result"]
            .as_array()
            .ok_or_else(|| AlphaSecError::api(500, "Invalid trades response format"))?
            .iter()
            .map(|trade| serde_json::from_value(trade.clone()).map_err(AlphaSecError::Json))
            .collect::<Result<Vec<Trade>>>()?;
        Ok(trades)
    }

    // === Account Information API ===

    /// Get account balance
    pub async fn get_balance(&self, address: &str) -> Result<Vec<Balance>> {
        let params = [("address", address)];
        let response = self.get("/api/v1/wallet/balance", Some(&params)).await?;
        
        let balances = response["result"]
            .as_array()
            .ok_or_else(|| AlphaSecError::api(500, "Invalid balance response format"))?
            .iter()
            .map(|balance| serde_json::from_value(balance.clone()).map_err(AlphaSecError::Json))
            .collect::<Result<Vec<Balance>>>()?;
        Ok(balances)
    }

    /// Get sessions
    pub async fn get_sessions(&self, address: &str) -> Result<Vec<Session>> {
        let params = [("address", address)];
        let response = self.get("/api/v1/wallet/session", Some(&params)).await?;
        
        let sessions = response["result"]
            .as_array()
            .ok_or_else(|| AlphaSecError::api(500, "Invalid sessions response format"))?
            .iter()
            .map(|session| serde_json::from_value(session.clone()).map_err(AlphaSecError::Json))
            .collect::<Result<Vec<Session>>>()?;
        Ok(sessions)
    }

    /// Get open orders
    pub async fn get_open_orders(&self, query: &OrdersQuery) -> Result<Vec<Order>> {
        let mut params = vec![("address", query.address.as_str())];
        
        let market_id;
        if let Some(ref market) = query.market {
            market_id = if let Some(metadata) = &self.token_metadata {
                metadata.market_to_market_id(market)?
            } else {
                market.clone()
            };
            params.push(("marketId", market_id.as_str()));
        }
        
        let limit_str;
        if let Some(limit) = query.limit {
            limit_str = limit.to_string();
            params.push(("limit", limit_str.as_str()));
        }
        
        let response = self.get("/api/v1/order/open", Some(&params)).await?;
        if response["result"].is_null() {
            return Ok(vec![]);
        }
        
        let orders = response["result"]
            .as_array()
            .ok_or_else(|| AlphaSecError::api(500, "Invalid open orders response format"))?
            .iter()
            .map(|order| serde_json::from_value(order.clone()).map_err(AlphaSecError::Json))
            .collect::<Result<Vec<Order>>>()?;
        Ok(orders)
    }

    /// Get order history
    pub async fn get_filled_canceled_orders(&self, query: &OrdersQuery) -> Result<Vec<Order>> {
        let mut params = vec![("address", query.address.as_str())];
        
        let market_id;
        if let Some(ref market) = query.market {
            market_id = if let Some(metadata) = &self.token_metadata {
                metadata.market_to_market_id(market)?
            } else {
                market.clone()
            };
            params.push(("marketId", market_id.as_str()));
        }
        
        let limit_str;
        if let Some(limit) = query.limit {
            limit_str = limit.to_string();
            params.push(("limit", limit_str.as_str()));
        }
        
        let response = self.get("/api/v1/order/", Some(&params)).await?;
        
        if response["result"].is_null() {
            return Ok(vec![]);
        }
        
        let orders = response["result"]
            .as_array()
            .ok_or_else(|| AlphaSecError::api(500, "Invalid orders response format"))?
            .iter()
            .map(|order| serde_json::from_value(order.clone()).map_err(AlphaSecError::Json))
            .collect::<Result<Vec<Order>>>()?;
        Ok(orders)
    }

    /// Get order by ID
    pub async fn get_order_by_id(&self, order_id: &str) -> Result<Option<Order>> {
        let path = format!("/api/v1/order/{}", order_id);
        match self.get(&path, None).await {
            Ok(response) => {
                let order = serde_json::from_value(response["result"].clone()).map_err(AlphaSecError::Json)?;
                Ok(Some(order))
            }
            Err(AlphaSecError::Api { code: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // === Trading API ===

    /// Submit an order
    pub async fn order(&self, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for trading operations"));
        }

        let params = serde_json::json!({
            "tx": signed_tx
        });

        let response = self.post("/api/v1/order", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    /// Cancel an order
    pub async fn cancel(&self, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for trading operations"));
        }

        let params = serde_json::json!({
            "tx": signed_tx
        });

        let response = self.post("/api/v1/wallet/order/cancel", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    /// Cancel all orders
    pub async fn cancel_all(&self, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for trading operations"));
        }

        let params = serde_json::json!({
            "tx": signed_tx
        });

        let response = self.post("/api/v1/order/cancel/all", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    /// Modify an order
    pub async fn modify(&self, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for trading operations"));
        }

        let params = serde_json::json!({
            "tx": signed_tx
        });

        let response = self.post("/api/v1/wallet/order/modify", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    /// Submit a stop order
    pub async fn stop_order(&self, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for trading operations"));
        }

        let params = serde_json::json!({
            "tx": signed_tx
        });

        let response = self.post("/api/v1/wallet/order/stop", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    /// Value transfer (native token)
    pub async fn native_transfer(&self, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for transfer operations"));
        }

        let params = serde_json::json!({
            "tx": signed_tx
        });

        let response = self.post("/api/v1/wallet/transfer", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    /// Token transfer
    pub async fn token_transfer(&self, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for transfer operations"));
        }

        let params = serde_json::json!({
            "tx": signed_tx
        });

        let response = self.post("/api/v1/wallet/transfer", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    /// Create a session
    pub async fn create_session(&self, session_id: &str, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for session operations"));
        }

        let params = serde_json::json!({
            "sessionId": session_id,
            "tx": signed_tx
        });

        let response = self.post("/api/v1/wallet/session", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    pub async fn update_session(&self, session_id: &str, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for session operations"));
        }

        let params = serde_json::json!({
            "sessionId": session_id,
            "tx": signed_tx
        });

        let response = self.post("/api/v1/wallet/session/update", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }
    
    pub async fn delete_session(&self, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for session operations"));
        }

        let params = serde_json::json!({
            "tx": signed_tx
        });

        let response = self.post("/api/v1/wallet/session/delete", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }
    /// Withdraw token
    pub async fn withdraw_token(&self, signed_tx: &str) -> Result<ApiResponse<Value>> {
        if self.signer.is_none() {
            return Err(AlphaSecError::auth("Signer required for withdraw operations"));
        }

        let params = serde_json::json!({
            "tx": signed_tx
        });

        let response = self.post("/api/v1/wallet/withdraw", Some(params)).await?;
        
        Ok(ApiResponse {
            success: response["code"] == 200,
            code: response.get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            result: response.get("result").cloned(),
            error: response.get("errMsg").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }
}
