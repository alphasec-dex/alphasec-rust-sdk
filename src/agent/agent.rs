//! Main Agent class for AlphaSec SDK
//!
//! Provides a unified interface for all AlphaSec operations including
//! market data, trading, and WebSocket.

use crate::{
    api::ApiClient,
    endpoints,
    error::{AlphaSecError, Result},
    session_commands::{SESSION_COMMAND_DELETE, SESSION_COMMAND_UPDATE},
    signer::{AlphaSecSigner, Config},
    types::{account::*, market::*, orders::*, session_commands::SESSION_COMMAND_CREATE},
};
pub use crate::types::account::{Transfer, TransferHistoryQuery};

#[cfg(feature = "websocket")]
use crate::websocket::{WsConfig, WsManager};

use rust_decimal::Decimal;
use ethers::{providers::Middleware, signers::LocalWallet, types::U64};
#[cfg(feature = "websocket")]
use tokio::sync::mpsc;

use tokio::time::{sleep, Duration};
use tracing::info;

/// Main Agent for AlphaSec operations
///
/// This is the primary interface for interacting with AlphaSec, combining
/// API client, signer, and WebSocket functionality in a single struct.
#[derive(Debug, Clone)]
pub struct Agent {
    /// API client for REST operations
    api: ApiClient,
    /// Transaction signer
    signer: AlphaSecSigner,
    /// WebSocket manager for real-time data
    #[cfg(feature = "websocket")]
    ws: Option<WsManager>,
    /// Configuration
    config: Config,
}

impl Agent {
    /// Create a new Agent
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration including API URL, network, and wallet keys
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use alphasec_rust_sdk::{Agent, Config};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = Config::new(
    ///         "https://api-testnet.alphasec.trade",
    ///         "kairos",
    ///         "0x1234567890123456789012345678901234567890",
    ///         Some("l1_private_key"),
    ///         None,
    ///         true
    ///     )?;
    ///     
    ///     let agent = Agent::new(config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(config: Config) -> Result<Self> {
        let signer = AlphaSecSigner::new(config.clone());
        let mut api = ApiClient::new(&config, Some(signer.clone()))?;

        // Initialize token metadata
        api.initialize_metadata().await?;

        #[cfg(feature = "websocket")]
        let ws = {
            let ws_config = WsConfig {
                url: config.ws_url.to_string(),
                max_reconnect_attempts: 0, // Infinite retries
                reconnect_delay: std::time::Duration::from_secs(1),
                max_reconnect_delay: std::time::Duration::from_secs(30),
                ping_interval: std::time::Duration::from_secs(10),
                pong_timeout: std::time::Duration::from_secs(10),
                message_queue_size: 1000,
            };
            Some(WsManager::new(ws_config))
        };

        info!(
            "✅ AlphaSec Agent initialized for network: {}",
            config.network
        );

        Ok(Self {
            api,
            signer,
            #[cfg(feature = "websocket")]
            ws,
            config,
        })
    }

    // === WebSocket Lifecycle ===

    /// Start WebSocket connection
    #[cfg(feature = "websocket")]
    pub async fn start(&mut self) -> Result<()> {
        if let Some(ref mut ws) = self.ws {
            ws.start().await?;
            info!("🚀 WebSocket manager started");
        }
        Ok(())
    }

    /// Stop WebSocket connection
    #[cfg(feature = "websocket")]
    pub async fn stop(&mut self) {
        if let Some(ref mut ws) = self.ws {
            ws.stop().await;
            info!("🛑 WebSocket manager stopped");
        }
    }

    /// # Arguments
    ///
    /// * `channel` - Channel in format 'type@target':
    ///   - 'trade@KAIA/USDT' for trade data
    ///   - 'ticker@KAIA/USDT' for ticker data
    ///   - 'depth@KAIA/USDT' for order book
    ///   - 'userEvent@0x123...' for user events
    ///
    /// # Returns
    ///
    /// Subscription ID for later unsubscribing
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use alphasec_rust_sdk::{Agent, Config};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = Config::new(
    ///         "https://api-testnet.alphasec.trade",
    ///         "kairos",
    ///         "0x1234567890123456789012345678901234567890",
    ///         Some("l1_private_key"),
    ///         None,
    ///         true
    ///     )?;
    ///     let mut agent = Agent::new(config).await?;
    ///     agent.start().await?;
    ///     
    ///     // Subscribe to channel
    ///     let sub_id = agent.subscribe("trade@KAIA/USDT").await?;
    ///     
    ///     // Get message receiver and process messages
    ///     if let Some(mut rx) = agent.take_message_receiver().await {
    ///         while let Some(msg) = rx.recv().await {
    ///             println!("Received: {:?}", msg);
    ///         }
    ///     }
    ///     
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "websocket")]
    pub async fn subscribe(&self, channel: &str) -> Result<i32> {
        if !channel.contains('@') {
            return Err(AlphaSecError::invalid_parameter(format!(
                "Channel format should be 'type@target', got: {}",
                channel
            )));
        }

        let parts: Vec<&str> = channel.split('@').collect();
        let channel_type = parts[0];
        let target = parts[1];

        let actual_channel = match channel_type {
            "trade" | "ticker" | "depth" => {
                // Convert market name to market_id
                let market_id = if let Some(metadata) = self.api.token_metadata() {
                    metadata.market_to_market_id(target)?
                } else {
                    target.to_string()
                };
                format!("{}@{}", channel_type, market_id)
            }
            "userEvent" => {
                // Use address directly
                format!("userEvent@{}", target)
            }
            _ => {
                return Err(AlphaSecError::invalid_parameter(format!(
                    "Unsupported channel type: {}. Use 'trade', 'ticker', 'depth', or 'userEvent'",
                    channel_type
                )));
            }
        };

        // Wait for WebSocket connection to be established
        info!("Waiting for WebSocket connection to be established");
        while let Some(ref ws) = self.ws {
            if ws.is_connected().await {
                break;
            }
            sleep(Duration::from_secs(1)).await;
        }

        if let Some(ref ws) = self.ws {
            let id = ws.subscribe(actual_channel).await?;
            info!("📡 Subscribed to channel: {} (ID: {})", channel, id);
            Ok(id)
        } else {
            Err(AlphaSecError::network("WebSocket not initialized"))
        }
    }

    /// Get the message receiver for processing WebSocket messages
    /// This can only be called once. After calling this, all WebSocket messages
    /// will be sent to the returned receiver.
    #[cfg(feature = "websocket")]
    pub async fn take_message_receiver(
        &self,
    ) -> Option<mpsc::UnboundedReceiver<crate::types::WebSocketMessage>> {
        if let Some(ref ws) = self.ws {
            ws.take_message_receiver().await
        } else {
            None
        }
    }

    /// Get a clone of the underlying WebSocket sender for direct frame sending.
    #[cfg(feature = "websocket")]
    pub async fn get_ws_sender(
        &self,
    ) -> Option<mpsc::UnboundedSender<tokio_tungstenite::tungstenite::Message>> {
        if let Some(ref ws) = self.ws {
            ws.get_outgoing_sender().await
        } else {
            None
        }
    }

    /// Unsubscribe from WebSocket channel
    #[cfg(feature = "websocket")]
    pub async fn unsubscribe(&self, subscription_id: i32) -> Result<bool> {
        if let Some(ref ws) = self.ws {
            let success = ws.unsubscribe(subscription_id).await?;
            if success {
                info!("📡 Unsubscribed from subscription ID: {}", subscription_id);
            }
            Ok(success)
        } else {
            Ok(false)
        }
    }

    // === Trading API Helpers ===

    /// Place an order
    ///
    /// # Arguments
    ///
    /// * `market` - Market symbol (e.g., "KAIA/USDT")
    /// * `side` - Order side (Buy or Sell)
    /// * `price` - Price in wei
    /// * `quantity` - Quantity in wei
    /// * `order_type` - Order type (Limit or Market)
    /// * `order_mode` - Order mode (Base or Quote)
    /// * `tp_limit` - Take profit limit price (optional)
    /// * `sl_trigger` - Stop loss trigger price (optional)
    /// * `sl_limit` - Stop loss limit price (optional)
    pub async fn order(
        &self,
        market: &str,
        side: OrderSide,
        price: Decimal,
        quantity: Decimal,
        order_type: OrderType,
        order_mode: OrderMode,
        tp_limit: Option<Decimal>,
        sl_trigger: Option<Decimal>,
        sl_limit: Option<Decimal>,
        timestamp_ms: Option<u64>,
    ) -> Result<String> {
        // Convert market to base/quote tokens
        let market_parts: Vec<&str> = market.split('/').collect();
        if market_parts.len() != 2 {
            return Err(AlphaSecError::invalid_parameter("Invalid market format"));
        }

        let base_symbol = market_parts[0];
        let quote_symbol = market_parts[1];

        // Convert symbols to token_ids using the metadata
        let token_metadata = self
            .api
            .token_metadata()
            .ok_or_else(|| AlphaSecError::config("Token metadata not initialized"))?;
        let base_token_id = token_metadata
            .symbol_token_id_map
            .get(base_symbol)
            .ok_or_else(|| {
                AlphaSecError::config(format!("Unknown base token symbol: {}", base_symbol))
            })?;
        let quote_token_id = token_metadata
            .symbol_token_id_map
            .get(quote_symbol)
            .ok_or_else(|| {
                AlphaSecError::config(format!("Unknown quote token symbol: {}", quote_symbol))
            })?;

        // Create order data with token_ids
        let order_data = self.signer.create_order_data(
            base_token_id,
            quote_token_id,
            side as u32,
            price,
            quantity,
            order_type as u32,
            order_mode as u32,
            tp_limit,
            sl_trigger,
            sl_limit,
        )?;

        // Generate and sign transaction
        let signed_tx = self
            .signer
            .generate_alphasec_transaction(timestamp_ms, &order_data, None)
            .await?;

        // Submit order
        let response = self.api.order(&signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    /// Cancel an order
    pub async fn cancel(&self, order_id: &str, timestamp_ms: Option<u64>) -> Result<String> {
        let cancel_data = self.signer.create_cancel_data(order_id)?;
        let signed_tx = self
            .signer
            .generate_alphasec_transaction(timestamp_ms, &cancel_data, None)
            .await?;
        let response = self.api.cancel(&signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    /// Cancel all orders
    pub async fn cancel_all(&self, timestamp_ms: Option<u64>) -> Result<String> {
        let cancel_all_data = self.signer.create_cancel_all_data()?;
        let signed_tx = self
            .signer
            .generate_alphasec_transaction(timestamp_ms, &cancel_all_data, None)
            .await?;
        let response = self.api.cancel_all(&signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    /// Modify an order
    pub async fn modify(
        &self,
        order_id: &str,
        new_price: Decimal,
        new_qty: Decimal,
        order_mode: OrderMode,
        timestamp_ms: Option<u64>,
    ) -> Result<String> {
        let modify_data =
            self.signer
                .create_modify_data(order_id, new_price, new_qty, order_mode as u32)?;
        let signed_tx = self
            .signer
            .generate_alphasec_transaction(timestamp_ms, &modify_data, None)
            .await?;
        let response = self.api.modify(&signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    /// Transfer value (native token)
    pub async fn native_transfer(&self, to: &str, value: Decimal, timestamp_ms: Option<u64>) -> Result<String> {
        let transfer_data = self.signer.create_value_transfer_data(to, value)?;
        let signed_tx = self
            .signer
            .generate_alphasec_transaction(timestamp_ms, &transfer_data, None)
            .await?;
        let response = self.api.native_transfer(&signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    /// Transfer tokens
    pub async fn token_transfer(&self, to: &str, value: f64, token: &str, timestamp_ms: Option<u64>) -> Result<String> {
        let token_id = self
            .api
            .token_metadata()
            .ok_or_else(|| AlphaSecError::config("Token metadata not initialized"))?
            .symbol_token_id_map
            .get(token)
            .ok_or_else(|| AlphaSecError::config(format!("Unknown token symbol: {}", token)))?;
        let transfer_data = self
            .signer
            .create_token_transfer_data(to, value, token_id)?;
        let signed_tx = self
            .signer
            .generate_alphasec_transaction(timestamp_ms, &transfer_data, None)
            .await?;
        let response = self.api.token_transfer(&signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    /// Place a stop order
    pub async fn stop_order(
        &self,
        base_token: &str,
        quote_token: &str,
        stop_price: Decimal,
        price: Decimal,
        quantity: Decimal,
        side: OrderSide,
        order_type: OrderType,
        order_mode: OrderMode,
        timestamp_ms: Option<u64>,
    ) -> Result<String> {
        // Convert symbols to token_ids using the metadata
        let token_metadata = self
            .api
            .token_metadata()
            .ok_or_else(|| AlphaSecError::config("Token metadata not initialized"))?;
        let base_token_id = token_metadata
            .symbol_token_id_map
            .get(base_token)
            .ok_or_else(|| {
                AlphaSecError::config(format!("Unknown base token symbol: {}", base_token))
            })?;
        let quote_token_id = token_metadata
            .symbol_token_id_map
            .get(quote_token)
            .ok_or_else(|| {
                AlphaSecError::config(format!("Unknown quote token symbol: {}", quote_token))
            })?;

        let stop_data = self.signer.create_stop_order_data(
            base_token_id,
            quote_token_id,
            stop_price,
            price,
            quantity,
            side as u32,
            order_type as u32,
            order_mode as u32,
        )?;
        let signed_tx = self
            .signer
            .generate_alphasec_transaction(timestamp_ms, &stop_data, None)
            .await?;
        let response = self.api.stop_order(&signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    /// Create session
    pub async fn create_session(
        &self,
        session_id: &str,
        session_wallet: Option<LocalWallet>,
        timestamp_ms: u64,
        expires_at: u64,
        metadata: &[u8],
    ) -> Result<String> {
        let new_session_wallet = match session_wallet {
            Some(wallet) => wallet,
            None => self.config.l2_wallet.as_ref().cloned().ok_or_else(|| {
                AlphaSecError::invalid_parameter("L2 wallet is required for session operations")
            })?,
        };

        let session_data = self
            .signer
            .create_session_data(
                SESSION_COMMAND_CREATE,
                new_session_wallet.clone(),
                timestamp_ms,
                expires_at,
                metadata,
            )
            .await?;

        let signed_tx = self
            .signer
            .generate_alphasec_transaction(
                Some(timestamp_ms),
                &session_data,
                Some(&new_session_wallet),
            )
            .await?;
        let response = self.api.create_session(session_id, &signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    /// Update session
    pub async fn update_session(
        &self,
        session_id: &str,
        session_wallet: Option<LocalWallet>,
        timestamp_ms: u64,
        expires_at: u64,
        metadata: &[u8],
    ) -> Result<String> {
        let new_session_wallet = match session_wallet {
            Some(wallet) => wallet,
            None => self.config.l2_wallet.as_ref().cloned().ok_or_else(|| {
                AlphaSecError::invalid_parameter("L2 wallet is required for session operations")
            })?,
        };
        let session_data = self
            .signer
            .create_session_data(
                SESSION_COMMAND_UPDATE,
                new_session_wallet.clone(),
                timestamp_ms,
                expires_at,
                metadata,
            )
            .await?;

        let signed_tx = self
            .signer
            .generate_alphasec_transaction(
                Some(timestamp_ms),
                &session_data,
                Some(&new_session_wallet),
            )
            .await?;
        let response = self.api.update_session(session_id, &signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    /// Delete session
    ///
    /// # Arguments
    ///
    /// * `session_id` - Session ID
    /// * `timestamp_ms` - Timestamp in milliseconds
    /// * `expires_at` - Expiration time in milliseconds
    /// * `metadata` - Metadata
    pub async fn delete_session(
        &self,
        session_wallet: Option<LocalWallet>,
        timestamp_ms: u64,
    ) -> Result<String> {
        let new_session_wallet = match session_wallet {
            Some(wallet) => wallet,
            None => self.config.l2_wallet.as_ref().cloned().ok_or_else(|| {
                AlphaSecError::invalid_parameter("L2 wallet is required for session operations")
            })?,
        };
        let session_data = self
            .signer
            .create_session_data(
                SESSION_COMMAND_DELETE,
                new_session_wallet.clone(),
                timestamp_ms,
                0,
                &[],
            )
            .await?;
        let signed_tx = self
            .signer
            .generate_alphasec_transaction(
                Some(timestamp_ms),
                &session_data,
                Some(&new_session_wallet),
            )
            .await?;
        let response = self.api.delete_session(&signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    /// deposit token
    ///
    /// # Arguments
    ///
    /// * `token` - Token symbol (e.g., "KAIA")
    /// * `value` - Amount to deposit in trading units
    pub async fn deposit_token(&self, token: &str, value: f64) -> Result<String> {
        let token_id = self
            .api
            .token_metadata()
            .clone()
            .ok_or_else(|| AlphaSecError::config("Token metadata not initialized"))?
            .symbol_token_id_map
            .get(token)
            .ok_or_else(|| AlphaSecError::config(format!("Unknown token symbol: {}", token)))?;
        let token_l1_address = self
            .api
            .token_metadata()
            .ok_or_else(|| AlphaSecError::config("Token metadata not initialized"))?
            .token_id_address_map
            .get(token_id)
            .ok_or_else(|| AlphaSecError::config(format!("Unknown token symbol: {}", token)))?;
        let token_l1_decimals = self
            .api
            .token_metadata()
            .ok_or_else(|| AlphaSecError::config("Token metadata not initialized"))?
            .token_id_decimal_map
            .get(token_id)
            .ok_or_else(|| AlphaSecError::config(format!("Unknown token symbol: {}", token)))?;
        let l1_url = match self.config.network {
            crate::signer::config::Network::Mainnet => endpoints::KAIA_MAINNET_URL,
            crate::signer::config::Network::Kairos => endpoints::KAIA_KAIROS_URL,
        };
        let l1_url = l1_url
            .parse::<reqwest::Url>()
            .map_err(|e| AlphaSecError::config(format!("Invalid L1 URL: {}", e)))?;
        let l1_provider = std::sync::Arc::new(ethers::providers::Provider::new(
            ethers::providers::Http::new(l1_url.clone()),
        ));

        let signed_tx = self
            .signer
            .generate_deposit_transaction(
                &l1_provider,
                token_id,
                value,
                Some(token_l1_address),
                Some(token_l1_decimals.parse::<u8>().unwrap_or(18)),
            )
            .await?;

        let raw_tx_bytes = hex::decode(&signed_tx[2..])
            .map_err(|e| AlphaSecError::config(format!("Failed to decode signed tx: {}", e)))?;
        let send_result = l1_provider
            .send_raw_transaction(raw_tx_bytes.into())
            .await
            .map_err(|e| AlphaSecError::config(format!("Failed to send raw transaction: {}", e)))?;

        let receipt = send_result.await;
        match receipt {
            Ok(Some(receipt)) => {
                if receipt.status == Some(U64::from(1)) {
                    Ok(format!("{:#x}", receipt.transaction_hash))
                } else {
                    Err(AlphaSecError::config(format!(
                        "Deposit transaction was reverted: {:?}",
                        receipt.status
                    )))
                }
            }
            Ok(None) => Err(AlphaSecError::config("Failed to get receipt")),
            Err(e) => Err(AlphaSecError::config(format!(
                "Failed to get receipt: {}",
                e
            ))),
        }
    }

    /// withdraw token
    ///
    /// # Arguments
    ///
    /// * `token` - Token symbol (e.g., "KAIA")
    /// * `value` - Amount to withdraw in trading units
    pub async fn withdraw_token(&self, token: &str, value: f64, timestamp_ms: Option<u64>) -> Result<String> {
        let token_id = self
            .api
            .token_metadata()
            .clone()
            .ok_or_else(|| AlphaSecError::config("Token metadata not initialized"))?
            .symbol_token_id_map
            .get(token)
            .ok_or_else(|| AlphaSecError::config(format!("Unknown token symbol: {}", token)))?;
        let token_l1_address = self
            .api
            .token_metadata()
            .ok_or_else(|| AlphaSecError::config("Token metadata not initialized"))?
            .token_id_address_map
            .get(token_id)
            .ok_or_else(|| AlphaSecError::config(format!("Unknown token symbol: {}", token)))?;
        let l1_url = match self.config.network {
            crate::signer::config::Network::Mainnet => endpoints::KAIA_MAINNET_URL,
            crate::signer::config::Network::Kairos => endpoints::KAIA_KAIROS_URL,
        };

        let l1_url = l1_url
            .parse::<reqwest::Url>()
            .map_err(|e| AlphaSecError::config(format!("Invalid L1 URL: {}", e)))?;
        let l1_provider = std::sync::Arc::new(ethers::providers::Provider::new(
            ethers::providers::Http::new(l1_url.clone()),
        ));

        let signed_tx = self
            .signer
            .generate_withdraw_transaction(&l1_provider, token_id, value, Some(token_l1_address), timestamp_ms)
            .await?;

        let response = self.api.withdraw_token(&signed_tx).await?;
        if response.success {
            Ok(response.result_string())
        } else {
            Err(AlphaSecError::api(
                response.code.unwrap(),
                response.error.unwrap(),
            ))
        }
    }

    // === Market Data Helpers ===
    /// Get depth for specific market
    pub async fn get_depth(&self, market: &str, limit: Option<u32>) -> Result<Depth> {
        self.api.get_depth(market, limit).await
    }

    /// Get ticker for specific market
    pub async fn get_ticker(&self, market: &str) -> Result<Ticker> {
        self.api.get_ticker(market).await
    }

    /// Get all tickers
    pub async fn get_tickers(&self) -> Result<Vec<Ticker>> {
        self.api.get_tickers().await
    }

    /// Get market list
    pub async fn get_market_list(&self) -> Result<Vec<Market>> {
        self.api.get_market_list().await
    }

    /// Get recent trades
    pub async fn get_trades(&self, market: &str, limit: Option<u32>) -> Result<Vec<Trade>> {
        self.api.get_trades(market, limit).await
    }

    /// Get all tokens
    pub async fn get_tokens(&self) -> Result<Vec<Token>> {
        self.api.get_tokens().await
    }

    // === Order History Helpers ===

    /// Get open orders
    pub async fn get_open_orders(
        &self,
        addr: &str,
        market: Option<&str>,
        limit: Option<u32>,
        _from_msec: Option<i64>,
        _end_msec: Option<i64>,
    ) -> Result<Vec<Order>> {
        let mut query = OrdersQuery::new(addr);
        if let Some(market) = market {
            query = query.market(market);
        }
        if let Some(limit) = limit {
            query = query.limit(limit);
        }
        // TODO: Add time range support
        self.api.get_open_orders(&query).await
    }

    /// Get filled and canceled orders
    pub async fn get_filled_canceled_orders(
        &self,
        addr: &str,
        market: Option<&str>,
        limit: Option<u32>,
        _from_msec: Option<i64>,
        _end_msec: Option<i64>,
    ) -> Result<Vec<Order>> {
        let mut query = OrdersQuery::new(addr);
        if let Some(market) = market {
            query = query.market(market);
        }
        if let Some(limit) = limit {
            query = query.limit(limit);
        }
        // TODO: Add time range support
        self.api.get_filled_canceled_orders(&query).await
    }

    /// Get order by ID
    pub async fn get_order_by_id(&self, order_id: &str) -> Result<Option<Order>> {
        self.api.get_order_by_id(order_id).await
    }

    // === Wallet/Session Helpers ===

    /// Get balance
    pub async fn get_balance(&self, addr: &str) -> Result<Balances> {
        self.api.get_balance(addr).await
    }

    /// Get sessions
    pub async fn get_sessions(&self, addr: &str) -> Result<Vec<Session>> {
        self.api.get_sessions(addr).await
    }

    /// Get transfer history for a wallet address
    ///
    /// # Arguments
    ///
    /// * `addr` - Wallet address to query
    /// * `token_id` - Optional token ID to filter by
    /// * `from_msec` - Optional start timestamp in milliseconds
    /// * `to_msec` - Optional end timestamp in milliseconds
    /// * `limit` - Optional maximum records to return (default: 100, max: 500)
    pub async fn get_transfer_history(
        &self,
        addr: &str,
        token_id: Option<i64>,
        from_msec: Option<i64>,
        to_msec: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Transfer>> {
        let query = TransferHistoryQuery {
            address: addr.to_string(),
            token_id,
            from_msec,
            to_msec,
            limit,
        };
        self.api.get_transfer_history(&query).await
    }

    /// Get the signer's L1 address
    pub fn l1_address(&self) -> &str {
        self.signer.l1_address()
    }

    /// Check if session is enabled
    pub fn is_session_enabled(&self) -> bool {
        self.signer.is_session_enabled()
    }
}
