//! AlphaSec transaction signer with EIP-712 support

use crate::{
    error::{AlphaSecError, Result},
    signer::{config::Config, normalize_price_quantity, transaction::*},
    types::{
        chain_ids::{ALPHASEC_MAINNET_CHAIN_ID, ALPHASEC_TESTNET_CHAIN_ID},
        constants::{abi::*, l1_contracts::*, ALPHASEC_NATIVE_TOKEN_ID},
        dex_commands::*,
        eip712::*,
        gas::*,
        l2_contracts::ALPHASEC_ORDER_CONTRACT_ADDR,
    },
    OrderType,
};
use base64::{self, Engine};
use ethers::{
    abi::{Abi, Token},
    contract::Contract,
    core::types::{
        transaction::eip2718::TypedTransaction, Address, Bytes, Eip1559TransactionRequest, U256,
        U64,
    },
    providers::{Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::transaction::eip712::{Eip712, TypedData as Eip712TypedData},
};
use rust_decimal::Decimal;
use serde_json;
use std::str::FromStr;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

/// AlphaSec transaction signer
///
/// This struct handles all transaction signing operations for AlphaSec,
/// including EIP-712 typed data signing for session management and
/// transaction encoding for trading operations.
#[derive(Debug)]
pub struct AlphaSecSigner {
    /// Configuration
    config: Config,
    /// Nonce counter for alphasec-style nonce generation
    nonce_counter: AtomicU64,
}

impl Clone for AlphaSecSigner {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            nonce_counter: AtomicU64::new(self.nonce_counter.load(Ordering::SeqCst)),
        }
    }
}

impl AlphaSecSigner {
    /// Create a new AlphaSec signer
    pub fn new(config: Config) -> Self {
        Self {
            config,
            nonce_counter: AtomicU64::new(0),
        }
    }

    /// Get the wallet
    pub fn get_wallet(&self) -> Result<&LocalWallet> {
        self.config.get_wallet()
    }

    /// Get the L1 address
    pub fn l1_address(&self) -> &str {
        &self.config.l1_address
    }

    /// Check if session is enabled
    pub fn is_session_enabled(&self) -> bool {
        self.config.session_enabled
    }

    /// Generate current timestamp in milliseconds
    fn current_timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Convert a human `value` (float) to on-chain base units.
    ///
    /// Notes:
    /// - Rejects NaN/inf and negative values.
    /// - Floors/truncates towards zero (matching previous `as u64` behavior for non-negative values).
    /// - Returns an error instead of silently saturating on overflow.
    fn to_onchain_units(value: f64, decimals: u32) -> Result<U256> {
        if !value.is_finite() {
            return Err(AlphaSecError::invalid_parameter(
                "value must be a finite number",
            ));
        }
        if value < 0.0 {
            return Err(AlphaSecError::invalid_parameter(
                "value must be non-negative",
            ));
        }

        // Use float math (input is f64 anyway), but validate bounds before converting.
        let scale = 10_f64.powi(decimals as i32);
        if !scale.is_finite() {
            return Err(AlphaSecError::invalid_parameter(
                "invalid decimals (scale overflow)",
            ));
        }

        let scaled = value * scale;
        if !scaled.is_finite() {
            return Err(AlphaSecError::invalid_parameter(
                "value is too large (scaled overflow)",
            ));
        }

        // We only support values that fit into u128 because the input is `f64` and cannot
        // precisely represent larger integers anyway.
        if scaled > (u128::MAX as f64) {
            return Err(AlphaSecError::invalid_parameter(
                "value is too large (exceeds supported range)",
            ));
        }

        Ok(U256::from(scaled.trunc() as u128))
    }

    /// Create EIP-712 typed data for session registration
    fn create_session_register_typed_data(
        &self,
        session_addr: &str,
        nonce: u64,
        expiry: u64,
    ) -> serde_json::Value {
        serde_json::json!({
            "domain": {
                "name": DOMAIN_NAME,
                "version": DOMAIN_VERSION,
                "chainId": self.config.get_chain_id(),
                "verifyingContract": VERIFYING_CONTRACT
            },
            "types": {
                "EIP712Domain": [
                    {"name": "name", "type": "string"},
                    {"name": "version", "type": "string"},
                    {"name": "chainId", "type": "uint256"},
                    {"name": "verifyingContract", "type": "address"}
                ],
                "RegisterSessionWallet": [
                    {"name": "sessionWallet", "type": "address"},
                    {"name": "expiry", "type": "uint64"},
                    {"name": "nonce", "type": "uint64"}
                ]
            },
            "primaryType": "RegisterSessionWallet",
            "message": {
                "sessionWallet": session_addr,
                "expiry": expiry,
                "nonce": nonce
            }
        })
    }

    /// Create session data for session management
    pub async fn create_session_data(
        &self,
        cmd: u8,
        session_wallet: LocalWallet,
        timestamp_ms: u64,
        expires_at: u64,
        metadata: &[u8],
    ) -> Result<Vec<u8>> {
        if self.config.l1_wallet.is_none() {
            return Err(AlphaSecError::invalid_parameter(
                "L1 wallet is required for session operations",
            ));
        }

        let session_addr = ethers::utils::to_checksum(&session_wallet.address(), None);

        // Create EIP-712 typed data and sign
        let typed_json =
            self.create_session_register_typed_data(&session_addr, timestamp_ms, expires_at);
        let l1_wallet = self.config.l1_wallet.as_ref().ok_or_else(|| {
            AlphaSecError::invalid_parameter("L1 wallet is required for session registration")
        })?;

        let typed_data: Eip712TypedData = serde_json::from_value(typed_json)
            .map_err(|e| AlphaSecError::generic(&format!("Invalid EIP-712 typed data: {}", e)))?;
        let digest = typed_data
            .encode_eip712()
            .map_err(|e| AlphaSecError::generic(&format!("Failed to encode EIP-712: {}", e)))?;
        let signature_placeholder = l1_wallet
            .sign_hash(ethers::types::TxHash(digest))
            .map_err(|e| AlphaSecError::signer(&format!("Failed to sign EIP-712 digest: {}", e)))?;
        let signature_b64 =
            base64::engine::general_purpose::STANDARD.encode(signature_placeholder.to_vec());

        let model = SessionContextModel {
            r#type: cmd,
            publickey: session_addr.to_string(),
            expires_at,
            nonce: timestamp_ms,
            l1owner: self.l1_address().to_string(), // Use l1_address
            l1signature: signature_b64,
            metadata: if metadata.is_empty() {
                None
            } else {
                Some(base64::engine::general_purpose::STANDARD.encode(metadata))
            },
        };

        let json_bytes = serde_json::to_vec(&model.to_wire())?;
        let mut result = vec![DEX_COMMAND_SESSION];
        result.extend_from_slice(&json_bytes);
        Ok(result)
    }

    /// Create value transfer data
    pub fn create_value_transfer_data(&self, to: &str, value: Decimal) -> Result<Vec<u8>> {
        let model = ValueTransferModel {
            l1owner: self.l1_address().to_string(), // Use l1_address
            to: to.to_string(),
            value: value.to_string(),
        };

        let payload_json = serde_json::to_string(&model.to_wire())?;
        let payload_bytes = payload_json.replace(" ", ""); // Remove all spaces
        let mut result = vec![DEX_COMMAND_TRANSFER];
        result.extend_from_slice(payload_bytes.as_bytes());
        Ok(result)
    }

    /// Create token transfer data
    pub fn create_token_transfer_data(&self, to: &str, value: f64, token: &str) -> Result<Vec<u8>> {
        let model = TokenTransferModel {
            l1owner: self.l1_address().to_string(), // Use l1_address
            to: to.to_string(),
            value: value.to_string(),
            token: token.to_string(),
        };

        let payload_json = serde_json::to_string(&model.to_wire())?;
        let payload_bytes = payload_json.replace(" ", ""); // Remove all spaces
        let mut result = vec![DEX_COMMAND_TOKEN_TRANSFER];
        result.extend_from_slice(payload_bytes.as_bytes());
        Ok(result)
    }

    /// Create order data
    pub fn create_order_data(
        &self,
        base_token: &str,
        quote_token: &str,
        side: u32,
        price: Decimal,
        quantity: Decimal,
        order_type: u32,
        order_mode: u32,
        tp_limit: Option<Decimal>,
        sl_trigger: Option<Decimal>,
        sl_limit: Option<Decimal>,
    ) -> Result<Vec<u8>> {
        let tpsl_model = if tp_limit.is_some() || sl_trigger.is_some() {
            Some(TpslModel {
                tp_limit: tp_limit.map(|v| v.to_string()),
                sl_trigger: sl_trigger.map(|v| v.to_string()),
                sl_limit: sl_limit.map(|v| v.to_string()),
            })
        } else {
            None
        };

        let (normalized_price, normalized_quantity) = normalize_price_quantity(price, quantity)?;

        let model = OrderModel {
            l1owner: self.l1_address().to_string(), // Use l1_address instead of wallet.address
            base_token: base_token.to_string(),
            quote_token: quote_token.to_string(),
            side,
            price: normalized_price.to_string(),
            quantity: if order_type == OrderType::Market as u32 {
                quantity.to_string()
            } else {
                normalized_quantity.to_string()
            },
            order_type,
            order_mode,
            tpsl: tpsl_model,
        };

        // Debug: Log the order data
        tracing::debug!("🔍 Order model: {:?}", model);

        // Use model's to_wire method for alphasec-style encoding
        let final_tx_bytes = model.to_wire()?;
        tracing::debug!(
            "🔍 Order payload bytes: {:?}",
            String::from_utf8_lossy(&final_tx_bytes[1..])
        );

        Ok(final_tx_bytes)
    }

    /// Create cancel data
    pub fn create_cancel_data(&self, order_id: &str) -> Result<Vec<u8>> {
        let model = CancelModel {
            l1owner: self.l1_address().to_string(), // Use l1_address
            order_id: order_id.to_string(),
        };

        // Use model's to_wire method for alphasec-style encoding
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    /// Create cancel all data
    pub fn create_cancel_all_data(&self) -> Result<Vec<u8>> {
        let model = CancelAllModel {
            l1owner: self.l1_address().to_string(), // Use l1_address
        };

        // Use model's to_wire method for alphasec-style encoding
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    /// Create modify data
    pub fn create_modify_data(
        &self,
        order_id: &str,
        new_price: Decimal,
        new_qty: Decimal,
        order_mode: u32,
    ) -> Result<Vec<u8>> {
        let (normalized_price, normalized_qty) = normalize_price_quantity(new_price, new_qty)?;
        let model = ModifyModel {
            l1owner: self.l1_address().to_string(), // Use l1_address
            order_id: order_id.to_string(),
            new_price: normalized_price.to_string(),
            new_qty: normalized_qty.to_string(),
            order_mode: order_mode as u32,
        };

        // Use model's to_wire method for alphasec-style encoding
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    /// Create stop order data
    pub fn create_stop_order_data(
        &self,
        base_token: &str,
        quote_token: &str,
        stop_price: Decimal,
        price: Decimal,
        quantity: Decimal,
        side: u32,
        order_type: u32,
        order_mode: u32,
    ) -> Result<Vec<u8>> {
        let (normalized_price, normalized_quantity) = normalize_price_quantity(price, quantity)?;
        let (normalized_stop_price, _) = normalize_price_quantity(stop_price, quantity)?;
        let model = StopOrderModel {
            l1owner: self.l1_address().to_string(), // Use l1_address
            base_token: base_token.to_string(),
            quote_token: quote_token.to_string(),
            stop_price: normalized_stop_price.to_string(),
            price: normalized_price.to_string(),
            quantity: normalized_quantity.to_string(),
            side,
            order_type,
            order_mode,
        };

        // Use model's to_wire method for alphasec-style encoding
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    // =========================================================================
    // Perp commands
    // =========================================================================

    /// Create perp order data (0x41)
    ///
    /// price and quantity are Decimal values; internally scaled ×10^18 via perp_scale.
    pub fn create_perp_order_data(
        &self,
        market_id: u64,
        side: u8,
        price: Decimal,
        quantity: Decimal,
        is_reduce_only: bool,
        time_in_force: u8,
        client_order_id: Option<&str>,
    ) -> Result<Vec<u8>> {
        use crate::signer::perp_transaction::PerpOrderModel;
        let model = PerpOrderModel {
            l1owner: self.l1_address().to_string(),
            market_id,
            side,
            price: crate::signer::perp_scale(price)?,
            quantity: crate::signer::perp_scale(quantity)?,
            is_reduce_only,
            time_in_force,
            client_order_id: client_order_id.map(|s| s.to_string()),
        };
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    /// Create perp cancel data (0x42)
    pub fn create_perp_cancel_data(&self, market_id: u64, order_id: &str) -> Result<Vec<u8>> {
        use crate::signer::perp_transaction::PerpCancelModel;
        let model = PerpCancelModel {
            l1owner: self.l1_address().to_string(),
            market_id,
            order_id: order_id.to_string(),
        };
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    /// Create perp cancel all data (0x43)
    pub fn create_perp_cancel_all_data(&self, market_id: u64) -> Result<Vec<u8>> {
        use crate::signer::perp_transaction::PerpCancelAllModel;
        let model = PerpCancelAllModel {
            l1owner: self.l1_address().to_string(),
            market_id,
        };
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    /// Create perp set leverage data (0x45)
    pub fn create_perp_set_leverage_data(&self, market_id: u64, leverage: u32) -> Result<Vec<u8>> {
        use crate::signer::perp_transaction::PerpSetLeverageModel;
        let model = PerpSetLeverageModel {
            l1owner: self.l1_address().to_string(),
            market_id,
            leverage,
        };
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    /// Create perp modify order data (0x4A) — cancel-and-replace
    ///
    /// new_price/new_quantity are Option<Decimal>: None means omit the key (server inherits).
    /// Some(value) is scaled ×10^18 via perp_scale and emitted as a raw JSON number.
    pub fn create_perp_modify_data(
        &self,
        market_id: u64,
        order_id: &str,
        new_price: Option<Decimal>,
        new_quantity: Option<Decimal>,
        client_order_id: Option<&str>,
    ) -> Result<Vec<u8>> {
        use crate::signer::perp_transaction::PerpModifyModel;
        let model = PerpModifyModel {
            l1owner: self.l1_address().to_string(),
            market_id,
            order_id: order_id.to_string(),
            new_price: new_price.map(crate::signer::perp_scale).transpose()?,
            new_quantity: new_quantity.map(crate::signer::perp_scale).transpose()?,
            client_order_id: client_order_id.map(|s| s.to_string()),
        };
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    /// Create perp deposit data (0x12) — Spot→Perp
    ///
    /// token: token identifier (e.g., "2" for USDT)
    /// amount: Decimal value; internally scaled ×10^18 via perp_scale.
    pub fn create_perp_deposit_data(&self, token: &str, amount: Decimal) -> Result<Vec<u8>> {
        use crate::signer::perp_transaction::PerpDepositModel;
        let model = PerpDepositModel {
            l1owner: self.l1_address().to_string(),
            token: token.to_string(),
            amount: crate::signer::perp_scale(amount)?,
        };
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    /// Create perp withdraw data (0x44) — Perp→Spot
    ///
    /// token: token identifier (e.g., "2" for USDT)
    /// amount: Decimal value; internally scaled ×10^18 via perp_scale.
    pub fn create_perp_withdraw_data(&self, token: &str, amount: Decimal) -> Result<Vec<u8>> {
        use crate::signer::perp_transaction::PerpWithdrawModel;
        let model = PerpWithdrawModel {
            l1owner: self.l1_address().to_string(),
            token: token.to_string(),
            amount: crate::signer::perp_scale(amount)?,
        };
        model
            .to_wire()
            .map_err(|e| AlphaSecError::signer(e.to_string()))
    }

    /// Generate AlphaSec transaction
    pub async fn generate_alphasec_transaction(
        &self,
        timestamp_ms: Option<u64>,
        data: &[u8],
        wallet: Option<&LocalWallet>,
    ) -> Result<String> {
        let wallet = match wallet {
            Some(w) => w,
            None => self.get_wallet()?,
        };
        let nonce = timestamp_ms.unwrap_or_else(Self::current_timestamp_ms);

        let chain_id = if self.config.chain_id.is_some() {
            self.config.chain_id.unwrap()
        } else {
            match self.config.network {
                crate::signer::config::Network::Mainnet => ALPHASEC_MAINNET_CHAIN_ID,
                crate::signer::config::Network::Kairos => ALPHASEC_TESTNET_CHAIN_ID,
            }
        };

        // Create EIP-1559 transaction
        let tx = Eip1559TransactionRequest {
            from: Some(wallet.address()),
            to: Some(
                ALPHASEC_ORDER_CONTRACT_ADDR
                    .parse::<Address>()
                    .unwrap()
                    .into(),
            ),
            gas: Some(U256::from(DEFAULT_GAS_LIMIT)),
            max_fee_per_gas: Some(U256::from(DEFAULT_MAX_FEE_PER_GAS)),
            max_priority_fee_per_gas: Some(U256::from(DEFAULT_MAX_PRIORITY_FEE_PER_GAS)),
            value: Some(U256::zero()),
            nonce: Some(U256::from(nonce)),
            data: Some(data.to_vec().into()),
            chain_id: Some(U64::from(chain_id)),
            access_list: Default::default(),
        };

        // Sign the transaction
        let typed_tx = TypedTransaction::Eip1559(tx);
        let signature = wallet
            .sign_transaction(&typed_tx)
            .await
            .map_err(|e| AlphaSecError::signer(format!("Failed to sign transaction: {}", e)))?;

        let raw_signed_tx = typed_tx.rlp_signed(&signature);
        Ok(format!("0x{}", hex::encode(raw_signed_tx)))
    }

    /// Generate deposit transaction for L1 to L2 transfer
    ///
    /// # Arguments
    /// * `l1_provider` - L1 provider for contract interaction
    /// * `token_id` - Token ID to deposit (0 for native token)
    /// * `value` - Amount to deposit in trading units
    /// * `token_l1_address` - L1 token contract address (required for ERC20 tokens)
    /// * `token_l1_decimals` - L1 token decimals (default: 18)
    ///
    /// # Returns
    /// * `Ok(String)` - Signed transaction hex string
    /// * `Err(AlphaSecError)` - If operation fails
    pub async fn generate_deposit_transaction(
        &self,
        l1_provider: &Arc<Provider<ethers::providers::Http>>,
        token_id: &str,
        value: f64,
        token_l1_address: Option<&str>,
        token_l1_decimals: Option<u8>,
    ) -> Result<String> {
        let l1_wallet = self.config.l1_wallet.as_ref().ok_or_else(|| {
            AlphaSecError::invalid_parameter("L1 wallet is required for deposit operations")
        })?;

        let decimals = token_l1_decimals.unwrap_or(18);
        let value_onchain_unit = Self::to_onchain_units(value, decimals as u32)?;

        if token_id == ALPHASEC_NATIVE_TOKEN_ID.to_string() {
            // Native token deposit
            let inbox_addr = match self.config.network {
                crate::signer::config::Network::Mainnet => MAINNET_INBOX_CONTRACT_ADDR,
                crate::signer::config::Network::Kairos => KAIROS_INBOX_CONTRACT_ADDR,
            };

            // Parse ABI and create contract instance
            let abi: Abi = serde_json::from_str(NATIVE_L1_ABI)
                .map_err(|e| AlphaSecError::generic(&format!("Failed to parse ABI: {}", e)))?;

            let inbox_address: Address = inbox_addr.parse().map_err(|e| {
                AlphaSecError::invalid_address(&format!("Invalid inbox address: {}", e))
            })?;

            let contract = Contract::new(inbox_address, abi, l1_provider.clone());

            // Get current nonce
            let l1_address: Address = self.l1_address().parse().unwrap();
            let nonce = l1_provider
                .get_transaction_count(l1_address, None)
                .await
                .map_err(|e| AlphaSecError::generic(&format!("Failed to get nonce: {}", e)))?;

            // Build transaction
            let tx = contract
                .method::<_, ()>("depositEth", ())
                .map_err(|e| AlphaSecError::generic(&format!("Failed to create method: {}", e)))?
                .value(value_onchain_unit)
                .gas(1_000_000)
                .nonce(nonce)
                .gas_price(U256::from(l1_provider.get_gas_price().await.unwrap()))
                .from(l1_address);

            let mut tx = tx.tx;
            tx.set_chain_id(self.config.get_chain_id());

            // Sign and return transaction (ensure correct L1 chain ID)
            let l1_wallet_chain = l1_wallet.clone();
            let signature = l1_wallet_chain.sign_transaction(&tx).await.map_err(|e| {
                AlphaSecError::signer(&format!("Failed to sign transaction: {}", e))
            })?;

            let raw_tx = tx.rlp_signed(&signature);
            Ok(format!("0x{}", hex::encode(raw_tx)))
        } else {
            // ERC20 token deposit
            let token_l1_addr = token_l1_address.ok_or_else(|| {
                AlphaSecError::invalid_parameter("token_l1_address is required for ERC20 tokens")
            })?;

            // Validate address format
            if !token_l1_addr.starts_with("0x") || token_l1_addr.len() != 42 {
                return Err(AlphaSecError::invalid_address(
                    "Invalid token_l1_address format",
                ));
            }

            let erc20_gateway_addr = match self.config.network {
                crate::signer::config::Network::Mainnet => MAINNET_ERC20_GATEWAY_CONTRACT_ADDR,
                crate::signer::config::Network::Kairos => KAIROS_ERC20_GATEWAY_CONTRACT_ADDR,
            };

            let erc20_router_addr = match self.config.network {
                crate::signer::config::Network::Mainnet => MAINNET_ERC20_ROUTER_CONTRACT_ADDR,
                crate::signer::config::Network::Kairos => KAIROS_ERC20_ROUTER_CONTRACT_ADDR,
            };

            // Parse ERC20 ABI and create contract instance
            let erc20_abi: Abi = serde_json::from_str(ERC20_ABI).map_err(|e| {
                AlphaSecError::generic(&format!("Failed to parse ERC20 ABI: {}", e))
            })?;

            let token_address: Address = token_l1_addr.parse().map_err(|e| {
                AlphaSecError::invalid_address(&format!("Invalid token address: {}", e))
            })?;

            let erc20_contract = Contract::new(token_address, erc20_abi, l1_provider.clone());

            // Check allowance
            let gateway_address: Address = erc20_gateway_addr.parse().map_err(|e| {
                AlphaSecError::invalid_address(&format!("Invalid gateway address: {}", e))
            })?;

            let allowance: U256 = erc20_contract
                .method::<_, U256>(
                    "allowance",
                    (
                        self.l1_address().parse::<Address>().unwrap(),
                        gateway_address,
                    ),
                )
                .map_err(|e| {
                    AlphaSecError::generic(&format!("Failed to create allowance method: {}", e))
                })?
                .call()
                .await
                .map_err(|e| {
                    AlphaSecError::generic(&format!("Failed to check allowance: {}", e))
                })?;

            // Approve if needed
            if allowance < value_onchain_unit {
                let l1_address: Address = self.l1_address().parse().unwrap();
                let nonce = l1_provider
                    .get_transaction_count(l1_address, None)
                    .await
                    .map_err(|e| AlphaSecError::generic(&format!("Failed to get nonce: {}", e)))?;

                let approve_tx = erc20_contract
                    .method::<_, ()>("approve", (gateway_address, value_onchain_unit))
                    .map_err(|e| {
                        AlphaSecError::generic(&format!("Failed to create approve method: {}", e))
                    })?
                    .gas(1_000_000)
                    .nonce(nonce)
                    .gas_price(U256::from(l1_provider.get_gas_price().await.unwrap()))
                    .from(l1_address);

                let mut approve_tx = approve_tx.tx;
                approve_tx.set_chain_id(self.config.get_chain_id());

                // Sign approve transaction with proper L1 chain ID
                let l1_wallet_chain = l1_wallet.clone();
                let signed_approve_tx = l1_wallet_chain
                    .sign_transaction(&approve_tx)
                    .await
                    .map_err(|e| {
                        AlphaSecError::signer(&format!("Failed to sign approve transaction: {}", e))
                    })?;

                let raw_approve_tx = approve_tx.rlp_signed(&signed_approve_tx);

                // Send approve transaction
                let _tx_hash = l1_provider
                    .send_raw_transaction(Bytes::from(raw_approve_tx))
                    .await
                    .map_err(|e| {
                        AlphaSecError::generic(&format!(
                            "Failed to send approve transaction: {}",
                            e
                        ))
                    })?;

                // Wait for approval (simplified - in production you might want to poll)
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }

            // Parse ERC20 Router ABI and create contract instance
            let router_abi: Abi = serde_json::from_str(ERC20_ROUTER_ABI).map_err(|e| {
                AlphaSecError::generic(&format!("Failed to parse router ABI: {}", e))
            })?;

            let router_address: Address = erc20_router_addr.parse().map_err(|e| {
                AlphaSecError::invalid_address(&format!("Invalid router address: {}", e))
            })?;

            let router_contract = Contract::new(router_address, router_abi, l1_provider.clone());

            // Prepare data for outbound transfer: abi.encode(uint256, bytes)
            let max_submission_cost = ((0.01f64) * 1e18) as u64;
            let encoded = ethers::abi::encode(&[
                Token::Uint(U256::from(max_submission_cost)),
                Token::Bytes(vec![]),
            ]);
            let data = Bytes::from(encoded);

            let l2_gas_limit = 1_000_000u64;
            let l2_gas_price = 1_000_000u64;
            let value_eth = (0.02 * 1e18) as u64;

            // Get nonce for main transaction
            let l1_address: Address = self.l1_address().parse().unwrap();
            let nonce = l1_provider
                .get_transaction_count(l1_address, None)
                .await
                .map_err(|e| AlphaSecError::generic(&format!("Failed to get nonce: {}", e)))?;

            // Build outbound transfer transaction
            let tx = router_contract
                .method::<_, ()>(
                    "outboundTransfer",
                    (
                        token_address,
                        l1_address,
                        value_onchain_unit,
                        l2_gas_limit,
                        l2_gas_price,
                        data,
                    ),
                )
                .map_err(|e| {
                    AlphaSecError::generic(&format!(
                        "Failed to create outboundTransfer method: {}",
                        e
                    ))
                })?
                .value(value_eth)
                .gas(1_000_000)
                .nonce(nonce)
                .gas_price(l1_provider.get_gas_price().await.unwrap())
                .from(l1_address);

            let mut tx = tx.tx;
            tx.set_chain_id(self.config.get_chain_id());

            // Sign and return transaction with proper L1 chain ID
            let l1_wallet_chain = l1_wallet.clone();
            let signed_tx = l1_wallet_chain.sign_transaction(&tx).await.map_err(|e| {
                AlphaSecError::signer(&format!("Failed to sign transaction: {}", e))
            })?;

            let raw_tx = tx.rlp_signed(&signed_tx);
            Ok(format!("0x{}", hex::encode(raw_tx)))
        }
    }

    /// Generate withdraw transaction for L2 to L1 transfer
    ///
    /// # Arguments
    /// * `l2_provider` - L2 provider for contract interaction
    /// * `token_id` - Token ID to withdraw (0 for native token)
    /// * `value` - Amount to withdraw in trading units
    /// * `token_l1_address` - L1 token contract address (required for ERC20 tokens)
    ///
    /// # Returns
    /// * `Ok(String)` - Signed transaction hex string
    /// * `Err(AlphaSecError)` - If operation fails
    pub async fn generate_withdraw_transaction(
        &self,
        l2_provider: &Arc<Provider<ethers::providers::Http>>,
        token_id: &str,
        value: f64,
        token_l1_address: Option<&str>,
        token_l1_decimals: Option<u8>,
        timestamp_ms: Option<u64>,
    ) -> Result<String> {
        let l1_wallet = self.config.l1_wallet.as_ref().ok_or_else(|| {
            AlphaSecError::invalid_parameter("L1 wallet is required for withdraw operations")
        })?;

        let chain_id = if self.config.chain_id.is_some() {
            self.config.chain_id.unwrap()
        } else {
            match self.config.network {
                crate::signer::config::Network::Mainnet => ALPHASEC_MAINNET_CHAIN_ID,
                crate::signer::config::Network::Kairos => ALPHASEC_TESTNET_CHAIN_ID,
            }
        };
        // For ERC20 tokens, use L1 decimals if provided (e.g., USDT has 6 decimals on L1)
        // For native tokens, always use 18 decimals
        let decimals = if token_id == ALPHASEC_NATIVE_TOKEN_ID.to_string() {
            18
        } else {
            token_l1_decimals.unwrap_or(18) as u32
        };
        let value_onchain_unit = Self::to_onchain_units(value, decimals)?;

        if token_id == ALPHASEC_NATIVE_TOKEN_ID.to_string() {
            // Native token withdrawal
            let system_contract_addr = crate::types::l2_contracts::ALPHASEC_SYSTEM_CONTRACT_ADDR;

            // Parse L2 System ABI and create contract instance
            let abi: Abi = serde_json::from_str(L2_SYSTEM_ABI).map_err(|e| {
                AlphaSecError::generic(&format!("Failed to parse L2 System ABI: {}", e))
            })?;

            let system_address: Address = system_contract_addr.parse().map_err(|e| {
                AlphaSecError::invalid_address(&format!("Invalid system contract address: {}", e))
            })?;

            let contract = Contract::new(system_address, abi, l2_provider.clone());

            // Generate nonce using current timestamp
            let nonce = timestamp_ms.unwrap_or_else(Self::current_timestamp_ms);

            // Build transaction
            let l1_address: Address = self.l1_address().parse().unwrap();
            let call = contract
                .method::<_, ()>("withdrawEth", l1_address)
                .map_err(|e| {
                    AlphaSecError::generic(&format!("Failed to create withdrawEth method: {}", e))
                })?
                .value(value_onchain_unit)
                .gas(1_000_000)
                .nonce(nonce)
                .from(l1_address);

            let tx = call.tx;
            let tx = match tx {
                TypedTransaction::Eip1559(mut inner) => {
                    inner.max_fee_per_gas = Some(U256::from(DEFAULT_MAX_FEE_PER_GAS));
                    inner.max_priority_fee_per_gas =
                        Some(U256::from(DEFAULT_MAX_PRIORITY_FEE_PER_GAS));
                    inner.chain_id = Some(U64::from(chain_id));
                    TypedTransaction::Eip1559(inner)
                }
                TypedTransaction::Legacy(mut inner) => {
                    inner.gas_price = Some(U256::from(DEFAULT_GAS_PRICE));
                    inner.chain_id = Some(U64::from(chain_id));
                    TypedTransaction::Legacy(inner)
                }
                TypedTransaction::Eip2930(inner) => TypedTransaction::Eip2930(inner),
            };

            tracing::info!("tx: {:?}", tx);

            // Sign and return transaction (ensure correct L2 AlphaSec chain ID)
            let signed_tx = l1_wallet.clone().sign_transaction(&tx).await.map_err(|e| {
                AlphaSecError::signer(&format!("Failed to sign transaction: {}", e))
            })?;

            let raw_tx = tx.rlp_signed(&signed_tx);
            Ok(format!("0x{}", hex::encode(raw_tx)))
        } else {
            // ERC20 token withdrawal
            let token_l1_addr = token_l1_address.ok_or_else(|| {
                AlphaSecError::invalid_parameter("token_l1_address is required for ERC20 tokens")
            })?;

            // Validate address format
            if !token_l1_addr.starts_with("0x") || token_l1_addr.len() != 42 {
                return Err(AlphaSecError::invalid_address(
                    "Invalid token_l1_address format",
                ));
            }

            let erc20_router_addr = match self.config.network {
                crate::signer::config::Network::Mainnet => {
                    crate::types::l2_contracts::ALPHASEC_MAINNET_GATEWAY_ROUTER_CONTRACT_ADDR
                }
                crate::signer::config::Network::Kairos => {
                    crate::types::l2_contracts::ALPHASEC_KAIROS_GATEWAY_ROUTER_CONTRACT_ADDR
                }
            };

            // Parse L2 ERC20 Router ABI and create contract instance
            let abi: Abi = serde_json::from_str(L2_ERC20_ROUTER_ABI).map_err(|e| {
                AlphaSecError::generic(&format!("Failed to parse L2 ERC20 Router ABI: {}", e))
            })?;

            let router_address: Address = erc20_router_addr.parse().map_err(|e| {
                AlphaSecError::invalid_address(&format!("Invalid router address: {}", e))
            })?;

            let contract = Contract::new(router_address, abi, l2_provider.clone());

            // Generate nonce using current timestamp
            let nonce = timestamp_ms.unwrap_or_else(Self::current_timestamp_ms);

            // Prepare data for outbound transfer
            // Must be empty bytes (NOT ASCII "0x", which becomes 0x3078 in calldata).
            let data = Bytes::from_static(&[]);
            let token_address: Address = token_l1_addr.parse().map_err(|e| {
                AlphaSecError::invalid_address(&format!("Invalid token address: {}", e))
            })?;

            // Build transaction
            let l1_address: Address = self.l1_address().parse().unwrap();
            let call = contract
                .method::<_, ()>(
                    "outboundTransfer",
                    (token_address, l1_address, value_onchain_unit, data),
                )
                .map_err(|e| {
                    AlphaSecError::generic(&format!(
                        "Failed to create outboundTransfer method: {}",
                        e
                    ))
                })?
                .gas(1_000_000)
                .nonce(nonce)
                .from(l1_address);

            let mut tx = call.tx;
            tx.set_chain_id(chain_id);

            // Sign and return transaction (ensure correct L2 AlphaSec chain ID)
            let signed_tx = l1_wallet.clone().sign_transaction(&tx).await.map_err(|e| {
                AlphaSecError::signer(&format!("Failed to sign transaction: {}", e))
            })?;

            let tx = match tx {
                TypedTransaction::Eip1559(mut inner) => {
                    inner.max_fee_per_gas = Some(U256::from(DEFAULT_MAX_FEE_PER_GAS));
                    inner.max_priority_fee_per_gas =
                        Some(U256::from(DEFAULT_MAX_PRIORITY_FEE_PER_GAS));
                    inner.chain_id = Some(U64::from(chain_id));
                    TypedTransaction::Eip1559(inner)
                }
                TypedTransaction::Legacy(mut inner) => {
                    inner.gas_price = Some(U256::from(DEFAULT_GAS_PRICE));
                    inner.chain_id = Some(U64::from(chain_id));
                    TypedTransaction::Legacy(inner)
                }
                TypedTransaction::Eip2930(inner) => TypedTransaction::Eip2930(inner),
            };
            let raw_tx = tx.rlp_signed(&signed_tx);
            Ok(format!("0x{}", hex::encode(raw_tx)))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{endpoints, signer::Config};

    fn create_test_config() -> Config {
        // These are well-known test keys from Hardhat/Anvil - DO NOT USE IN PRODUCTION
        Config::new(
            "https://api-testnet.alphasec.trade",
            "kairos",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", // Address for first test private key
            Some("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"), // First test private key
            None,  // L2 key, no session
            false, // L1 key, no session
            None,  // Chain ID
        )
        .unwrap()
    }

    fn create_test_config_mainnet() -> Config {
        // Same well-known Hardhat/Anvil test key, mainnet network selector.
        Config::new(
            "https://api.alphasec.trade",
            "mainnet",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            Some("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"),
            None,  // L2 key, no session
            false, // L1 key, no session
            None,  // Chain ID
        )
        .unwrap()
    }

    /// Decode a `0x`-prefixed signed raw transaction back into a TypedTransaction.
    fn decode_signed_tx(tx_hex: &str) -> TypedTransaction {
        let raw = hex::decode(tx_hex.trim_start_matches("0x")).expect("signed tx must be hex");
        let rlp = ethers::core::utils::rlp::Rlp::new(&raw);
        let (tx, _signature) =
            TypedTransaction::decode_signed(&rlp).expect("signed tx must RLP-decode");
        tx
    }

    #[test]
    fn test_signer_creation() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // Key-derived, lowercase — the mixed-case address passed to Config::new is ignored
        // (this exact-equality also rules out preserving the caller's checksum casing).
        assert_eq!(
            signer.l1_address(),
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
    }

    #[test]
    fn test_timestamp_generation() {
        let ts1 = AlphaSecSigner::current_timestamp_ms();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let ts2 = AlphaSecSigner::current_timestamp_ms();

        assert!(ts2 > ts1);
    }

    #[test]
    fn test_session_register_typed_data() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let session_addr = "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd";
        let nonce: u64 = 123456789;
        let expiry: u64 = 987654321;

        let typed_data = signer.create_session_register_typed_data(session_addr, nonce, expiry);

        // Check domain
        assert_eq!(typed_data["domain"]["name"], DOMAIN_NAME);
        assert_eq!(typed_data["domain"]["version"], DOMAIN_VERSION);
        assert_eq!(typed_data["domain"]["chainId"], 1001);
        assert_eq!(
            typed_data["domain"]["verifyingContract"],
            VERIFYING_CONTRACT
        );

        // Check message: nonce/expiry are JSON Numbers, not strings.
        assert_eq!(typed_data["message"]["sessionWallet"], session_addr);
        assert_eq!(typed_data["message"]["nonce"], nonce);
        assert_eq!(typed_data["message"]["expiry"], expiry);
        assert!(
            typed_data["message"]["nonce"].is_u64(),
            "nonce must be a JSON Number, got: {:?}",
            typed_data["message"]["nonce"]
        );
        assert!(
            typed_data["message"]["expiry"].is_u64(),
            "expiry must be a JSON Number, got: {:?}",
            typed_data["message"]["expiry"]
        );

        // Check primary type
        assert_eq!(typed_data["primaryType"], "RegisterSessionWallet");
    }

    #[test]
    fn test_create_value_transfer_data() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let to = "0xrecipientaddressrecipientaddressrecipient";
        let value = Decimal::from_str("1").unwrap(); // 1 KAIA

        let result = signer.create_value_transfer_data(to, value);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(!data.is_empty());
        assert_eq!(data[0], DEX_COMMAND_TRANSFER); // First byte should be the command
    }

    #[test]
    fn test_create_token_transfer_data() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let to = "0xrecipientaddressrecipientaddressrecipient";
        let value = 100f64; // 100 USDT
        let token = "USDT";

        let result = signer.create_token_transfer_data(to, value, token);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(!data.is_empty());
        assert_eq!(data[0], DEX_COMMAND_TOKEN_TRANSFER); // First byte should be the command
    }

    #[test]
    fn test_create_order_data() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let base_token = "BTC";
        let quote_token = "USDT";
        let side = 0; // Buy
        let price = Decimal::from_str("50000.0").unwrap(); // $50,000
        let quantity = Decimal::from_str("1").unwrap(); // 1 BTC
        let order_type = 0; // Limit
        let order_mode = 0; // GTC

        let result = signer.create_order_data(
            base_token,
            quote_token,
            side,
            price,
            quantity,
            order_type,
            order_mode,
            None, // tp_limit
            None, // sl_trigger
            None, // sl_limit
        );
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(!data.is_empty());
        assert_eq!(data[0], DEX_COMMAND_ORDER); // First byte should be the command
    }

    #[test]
    fn test_create_cancel_data() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let order_id = "test-order-id-12345";

        let result = signer.create_cancel_data(order_id);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(!data.is_empty());
        assert_eq!(data[0], DEX_COMMAND_CANCEL); // First byte should be the command
    }

    #[test]
    fn test_create_cancel_all_data() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let result = signer.create_cancel_all_data();
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(!data.is_empty());
        assert_eq!(data[0], DEX_COMMAND_CANCEL_ALL); // First byte should be the command
    }

    #[test]
    fn test_create_modify_data() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let order_id = "test-order-id-12345";
        let new_price = Decimal::from_str("51000").unwrap(); // New price $51,000
        let new_qty = Decimal::from_str("2").unwrap(); // New quantity 2 BTC
        let order_mode = 1u32; // IOC

        let result = signer.create_modify_data(order_id, new_price, new_qty, order_mode);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(!data.is_empty());
        assert_eq!(data[0], DEX_COMMAND_MODIFY); // First byte should be the command
    }

    #[test]
    fn test_create_stop_order_data() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let base_token = "ETH";
        let quote_token = "USDT";
        let stop_price = Decimal::from_str("3000").unwrap(); // $3,000
        let price = Decimal::from_str("2950").unwrap(); // $2,950
        let quantity = Decimal::from_str("1").unwrap(); // 1 ETH
        let side = 1; // Sell
        let order_type = 1; // Market
        let order_mode = 0; // GTC

        let result = signer.create_stop_order_data(
            base_token,
            quote_token,
            stop_price,
            price,
            quantity,
            side,
            order_type,
            order_mode,
        );
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(!data.is_empty());
        assert_eq!(data[0], DEX_COMMAND_STOP_ORDER); // First byte should be the command
    }

    #[tokio::test]
    async fn test_generate_alphasec_transaction() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // Create some test data (cancel all command)
        let data = signer.create_cancel_all_data().unwrap();

        let result = signer
            .generate_alphasec_transaction(None, &data, None)
            .await;
        assert!(result.is_ok());

        let tx_hex = result.unwrap();
        assert!(tx_hex.starts_with("0x"));
        assert!(tx_hex.len() > 10); // Should be a valid hex string
    }

    #[tokio::test]
    async fn test_generate_deposit_transaction() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let url = url::Url::parse(endpoints::KAIA_KAIROS_URL).unwrap();
        let http = ethers::providers::Http::new(url);
        let provider = Arc::new(ethers::providers::Provider::new(http));

        let result = signer
            .generate_deposit_transaction(&provider, "1", 1f64, None, None)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_generate_withdraw_transaction() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // Use a mock provider URL that will fail but we can test the function structure
        let url = url::Url::parse(endpoints::ALPHASEC_API_TESTNET_URL).unwrap();
        let http = ethers::providers::Http::new(url);
        let provider = Arc::new(ethers::providers::Provider::new(http));

        // Test that the function compiles and runs (will fail on network call, but that's expected)
        let result = signer
            .generate_withdraw_transaction(&provider, "1", 1f64, None, Some(18), None)
            .await;

        // We expect this to fail due to network connection, but the function should be callable
        // assert!(result.is_err());

        println!("result: {:#?}", result);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Perp command tests
    // =========================================================================

    #[test]
    fn test_create_perp_order_data_command_byte_is_0x41() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let result = signer.create_perp_order_data(
            1,                                     // market_id
            0,                                     // side: Buy
            Decimal::from_str("42000.5").unwrap(), // price: 42000.5 → 42000500000000000000000 in 18dec
            Decimal::from_str("0.01").unwrap(),    // quantity: 0.01 → 10000000000000000 in 18dec
            false,
            2, // POST
            Some("coid-abc"),
        );

        assert!(
            result.is_ok(),
            "create_perp_order_data returned Err: {:?}",
            result.err()
        );
        let data = result.unwrap();
        assert!(!data.is_empty());
        assert_eq!(
            data[0], 0x41,
            "first byte must be DEX_COMMAND_PERP_ORDER (0x41)"
        );
    }

    #[test]
    fn test_create_perp_order_data_json_fields_are_correct() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let result = signer.create_perp_order_data(
            7,
            1,                                  // side: Sell
            Decimal::from_str("2000").unwrap(), // price: 2000 → 2000000000000000000000 in 18dec
            Decimal::from_str("0.5").unwrap(),  // quantity: 0.5 → 500000000000000000 in 18dec
            true,
            1, // IOC
            Some("order-xyz"),
        );

        assert!(result.is_ok());
        let data = result.unwrap();

        // Bytes after the command byte are JSON
        let json: serde_json::Value = serde_json::from_slice(&data[1..])
            .expect("payload after command byte must be valid JSON");

        assert_eq!(
            json["l1owner"],
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
        assert_eq!(json["marketId"], 7u64);
        assert_eq!(json["side"], 1u8);
        assert_eq!(json["isReduceOnly"], true);
        assert_eq!(json["timeInForce"], 1u8);
        assert_eq!(json["clientOrderId"], "order-xyz");
        // price/quantity are raw numbers for Go big.Int compat — check raw bytes
        let raw_json = std::str::from_utf8(&data[1..]).unwrap();
        assert!(
            raw_json.contains("\"price\":2000000000000000000000"),
            "price must be raw number, got: {}",
            raw_json
        );
        assert!(
            raw_json.contains("\"quantity\":500000000000000000"),
            "qty must be raw number, got: {}",
            raw_json
        );
    }

    #[test]
    fn test_create_perp_order_data_without_client_order_id_omits_field() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let result = signer.create_perp_order_data(
            1,
            0,
            Decimal::from_str("1000").unwrap(), // price: 1000 → 1000000000000000000000
            Decimal::from_str("0.1").unwrap(),  // quantity: 0.1 → 100000000000000000
            false,
            0,
            None,
        );

        assert!(result.is_ok());
        let data = result.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&data[1..]).unwrap();

        // clientOrderId must not appear when None (skip_serializing_if)
        assert!(
            json.get("clientOrderId").is_none(),
            "clientOrderId must be absent when not provided"
        );
    }

    #[test]
    fn test_create_perp_cancel_data_command_byte_is_0x42() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let result = signer.create_perp_cancel_data(
            1,
            "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        );

        assert!(
            result.is_ok(),
            "create_perp_cancel_data returned Err: {:?}",
            result.err()
        );
        let data = result.unwrap();
        assert_eq!(
            data[0], 0x42,
            "first byte must be DEX_COMMAND_PERP_CANCEL (0x42)"
        );
    }

    #[test]
    fn test_create_perp_cancel_data_json_fields_are_correct() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let order_id = "0xabcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234";
        let result = signer.create_perp_cancel_data(3, order_id);

        assert!(result.is_ok());
        let data = result.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&data[1..]).unwrap();

        assert_eq!(
            json["l1owner"],
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
        assert_eq!(json["marketId"], 3u64);
        assert_eq!(json["orderId"], order_id);
    }

    #[test]
    fn test_create_perp_cancel_all_data_command_byte_is_0x43() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let result = signer.create_perp_cancel_all_data(1);

        assert!(
            result.is_ok(),
            "create_perp_cancel_all_data returned Err: {:?}",
            result.err()
        );
        let data = result.unwrap();
        assert_eq!(
            data[0], 0x43,
            "first byte must be DEX_COMMAND_PERP_CANCEL_ALL (0x43)"
        );
    }

    #[test]
    fn test_create_perp_cancel_all_data_market_id_zero_cancels_all_markets() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let result = signer.create_perp_cancel_all_data(0); // 0 = all markets

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data[0], 0x43);

        let json: serde_json::Value = serde_json::from_slice(&data[1..]).unwrap();
        assert_eq!(json["marketId"], 0u64);
        assert_eq!(
            json["l1owner"],
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
    }

    #[test]
    fn test_create_perp_set_leverage_data_command_byte_is_0x45() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let result = signer.create_perp_set_leverage_data(1, 10);

        assert!(
            result.is_ok(),
            "create_perp_set_leverage_data returned Err: {:?}",
            result.err()
        );
        let data = result.unwrap();
        assert_eq!(
            data[0], 0x45,
            "first byte must be DEX_COMMAND_PERP_SET_LEVERAGE (0x45)"
        );
    }

    #[test]
    fn test_create_perp_set_leverage_data_json_fields_are_correct() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let result = signer.create_perp_set_leverage_data(2, 50);

        assert!(result.is_ok());
        let data = result.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&data[1..]).unwrap();

        assert_eq!(
            json["l1owner"],
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
        assert_eq!(json["marketId"], 2u64);
        assert_eq!(json["leverage"], 50u32);
    }

    #[test]
    fn test_perp_order_model_to_wire_camel_case_field_names() {
        use crate::signer::perp_transaction::PerpOrderModel;

        let model = PerpOrderModel {
            l1owner: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".to_string(),
            market_id: 1,
            side: 0,
            price: "1000000000000000000000".to_string(),
            quantity: "100000000000000000".to_string(),
            is_reduce_only: false,
            time_in_force: 2,
            client_order_id: Some("my-coid".to_string()),
        };

        let wire = model.to_wire().expect("to_wire must not fail");
        assert_eq!(wire[0], 0x41);

        let json: serde_json::Value = serde_json::from_slice(&wire[1..]).unwrap();

        // Verify camelCase rename attributes are applied
        assert!(json.get("marketId").is_some(), "marketId must be camelCase");
        assert!(
            json.get("market_id").is_none(),
            "snake_case market_id must not appear"
        );
        assert!(
            json.get("isReduceOnly").is_some(),
            "isReduceOnly must be camelCase"
        );
        assert!(
            json.get("is_reduce_only").is_none(),
            "snake_case is_reduce_only must not appear"
        );
        assert!(
            json.get("timeInForce").is_some(),
            "timeInForce must be camelCase"
        );
        assert!(
            json.get("time_in_force").is_none(),
            "snake_case time_in_force must not appear"
        );
        assert!(
            json.get("clientOrderId").is_some(),
            "clientOrderId must be camelCase"
        );
        assert!(
            json.get("client_order_id").is_none(),
            "snake_case client_order_id must not appear"
        );

        // Values round-trip correctly
        assert_eq!(json["marketId"], 1u64);
        assert_eq!(json["isReduceOnly"], false);
        assert_eq!(json["timeInForce"], 2u8);
        assert_eq!(json["clientOrderId"], "my-coid");
    }

    #[test]
    fn test_perp_cancel_model_to_wire_camel_case_field_names() {
        use crate::signer::perp_transaction::PerpCancelModel;

        let model = PerpCancelModel {
            l1owner: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".to_string(),
            market_id: 5,
            order_id: "0xdeadbeef".to_string(),
        };

        let wire = model.to_wire().expect("to_wire must not fail");
        assert_eq!(wire[0], 0x42);

        let json: serde_json::Value = serde_json::from_slice(&wire[1..]).unwrap();
        assert!(json.get("marketId").is_some(), "marketId must be camelCase");
        assert!(
            json.get("market_id").is_none(),
            "snake_case market_id must not appear"
        );
        assert!(json.get("orderId").is_some(), "orderId must be camelCase");
        assert!(
            json.get("order_id").is_none(),
            "snake_case order_id must not appear"
        );
    }

    #[test]
    fn test_perp_set_leverage_model_to_wire_camel_case_field_names() {
        use crate::signer::perp_transaction::PerpSetLeverageModel;

        let model = PerpSetLeverageModel {
            l1owner: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".to_string(),
            market_id: 1,
            leverage: 25,
        };

        let wire = model.to_wire().expect("to_wire must not fail");
        assert_eq!(wire[0], 0x45);

        let json: serde_json::Value = serde_json::from_slice(&wire[1..]).unwrap();
        assert!(json.get("marketId").is_some(), "marketId must be camelCase");
        assert!(
            json.get("market_id").is_none(),
            "snake_case market_id must not appear"
        );
        assert_eq!(json["leverage"], 25u32);
    }

    // ---- Perp deposit/withdraw tests ----

    #[test]
    fn test_create_perp_deposit_data_command_byte_is_0x12() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // 1000 USDT → 1000000000000000000000 in 18dec
        let result = signer.create_perp_deposit_data("2", Decimal::from_str("1000").unwrap());
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data[0], 0x12, "command byte must be 0x12 for PerpDeposit");
    }

    #[test]
    fn test_create_perp_deposit_data_json_fields_are_correct() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // 5000 USDT → 5000000000000000000000 in 18dec
        let result = signer.create_perp_deposit_data("2", Decimal::from_str("5000").unwrap());
        assert!(result.is_ok());
        let data = result.unwrap();

        let json: serde_json::Value = serde_json::from_slice(&data[1..])
            .expect("payload after command byte must be valid JSON");

        assert_eq!(
            json["l1owner"],
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
        assert_eq!(json["token"], "2");
        // amount is a JSON *string* (server's perpDepositContextJSON.amount is a Go string field;
        // a raw number is rejected with -1103). Must be quoted, not a bare number.
        assert_eq!(json["amount"], "5000000000000000000000");
        let raw_json = std::str::from_utf8(&data[1..]).unwrap();
        assert!(
            raw_json.contains("\"amount\":\"5000000000000000000000\""),
            "amount must be a quoted string, got: {}",
            raw_json
        );
    }

    #[test]
    fn test_create_perp_withdraw_data_command_byte_is_0x44() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // 1000 USDT → 1000000000000000000000 in 18dec
        let result = signer.create_perp_withdraw_data("2", Decimal::from_str("1000").unwrap());
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data[0], 0x44, "command byte must be 0x44 for PerpWithdraw");
    }

    #[test]
    fn test_create_perp_withdraw_data_json_fields_are_correct() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // 3000 USDT → 3000000000000000000000 in 18dec
        let result = signer.create_perp_withdraw_data("2", Decimal::from_str("3000").unwrap());
        assert!(result.is_ok());
        let data = result.unwrap();

        let json: serde_json::Value = serde_json::from_slice(&data[1..])
            .expect("payload after command byte must be valid JSON");

        assert_eq!(
            json["l1owner"],
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
        assert_eq!(json["token"], "2");
        // amount is a JSON *string* (server's perpWithdrawContextJSON.amount is a Go string field;
        // a raw number is rejected with -1103). Must be quoted, not a bare number.
        assert_eq!(json["amount"], "3000000000000000000000");
        let raw_json = std::str::from_utf8(&data[1..]).unwrap();
        assert!(
            raw_json.contains("\"amount\":\"3000000000000000000000\""),
            "amount must be a quoted string, got: {}",
            raw_json
        );
    }

    #[test]
    fn test_perp_modify_omits_none_keys() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // Only newPrice specified; newQuantity and clientOrderId are None (inherited)
        let data = signer
            .create_perp_modify_data(
                1,
                "0xORDER",
                Some(Decimal::from_str("91000").unwrap()),
                None,
                None,
            )
            .unwrap();

        assert_eq!(data[0], 0x4A, "first byte must be 0x4A (PERP_MODIFY)");

        let json = std::str::from_utf8(&data[1..]).unwrap();
        // newPrice must appear as a raw (unquoted) number
        assert!(
            json.contains("\"newPrice\":91000000000000000000000"),
            "newPrice must be raw number, got: {}",
            json
        );
        // None fields must not appear as keys
        assert!(
            !json.contains("newQuantity"),
            "newQuantity must be absent when None"
        );
        assert!(
            !json.contains("clientOrderId"),
            "clientOrderId must be absent when None"
        );
    }

    #[test]
    fn test_perp_modify_all_none_is_valid_json_no_trailing_comma() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // Every optional field None: only l1owner/marketId/orderId remain.
        let data = signer
            .create_perp_modify_data(4, "0xORDER", None, None, None)
            .unwrap();
        assert_eq!(data[0], 0x4A, "first byte must be 0x4A (PERP_MODIFY)");

        // A trailing comma or brace imbalance makes this parse fail.
        let json: serde_json::Value = serde_json::from_slice(&data[1..])
            .expect("all-None modify payload must still be valid JSON");

        // Required fields present, optional fields fully absent (key-absent = server inherits).
        assert_eq!(json["marketId"], 4u64);
        assert_eq!(json["orderId"], "0xORDER");
        assert!(
            json.get("newPrice").is_none(),
            "newPrice must be absent when None"
        );
        assert!(
            json.get("newQuantity").is_none(),
            "newQuantity must be absent when None"
        );
        assert!(
            json.get("clientOrderId").is_none(),
            "clientOrderId must be absent when None"
        );

        // Structurally: must end with a single closing brace, no dangling comma before it.
        let raw = std::str::from_utf8(&data[1..]).unwrap();
        assert!(
            raw.ends_with('}'),
            "must end with closing brace, got: {}",
            raw
        );
        assert!(
            !raw.contains(",}"),
            "must not contain a trailing comma, got: {}",
            raw
        );
    }

    #[test]
    fn test_perp_modify_empty_client_order_id_is_included_and_distinct_from_none() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // Some("") — explicitly empty, must appear as a present key with empty value.
        let with_empty = signer
            .create_perp_modify_data(1, "0xORDER", None, None, Some(""))
            .unwrap();
        let json_empty: serde_json::Value = serde_json::from_slice(&with_empty[1..]).unwrap();
        assert!(
            json_empty.get("clientOrderId").is_some(),
            "Some(\"\") must keep the clientOrderId key, got: {}",
            std::str::from_utf8(&with_empty[1..]).unwrap()
        );
        assert_eq!(
            json_empty["clientOrderId"], "",
            "empty coid must serialize as empty string"
        );

        // None — key omitted entirely. Proves the two are distinct.
        let with_none = signer
            .create_perp_modify_data(1, "0xORDER", None, None, None)
            .unwrap();
        let json_none: serde_json::Value = serde_json::from_slice(&with_none[1..]).unwrap();
        assert!(
            json_none.get("clientOrderId").is_none(),
            "None must omit the clientOrderId key"
        );
    }

    #[test]
    fn test_perp_modify_camel_case_field_names_and_raw_quantity() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // Both newPrice and newQuantity present, plus a clientOrderId.
        let data = signer
            .create_perp_modify_data(
                9,
                "0xMODID",
                Some(Decimal::from_str("2000").unwrap()), // → 2000000000000000000000
                Some(Decimal::from_str("0.5").unwrap()),  // → 500000000000000000
                Some("c-1"),
            )
            .unwrap();
        assert_eq!(data[0], 0x4A);

        let raw = std::str::from_utf8(&data[1..]).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&data[1..]).unwrap();

        // camelCase present, snake_case absent.
        assert!(json.get("marketId").is_some(), "marketId must be camelCase");
        assert!(
            json.get("market_id").is_none(),
            "snake_case market_id must not appear"
        );
        assert!(json.get("orderId").is_some(), "orderId must be camelCase");
        assert!(
            json.get("order_id").is_none(),
            "snake_case order_id must not appear"
        );
        assert!(json.get("newPrice").is_some(), "newPrice must be camelCase");
        assert!(
            json.get("new_price").is_none(),
            "snake_case new_price must not appear"
        );
        assert!(
            json.get("newQuantity").is_some(),
            "newQuantity must be camelCase"
        );
        assert!(
            json.get("new_quantity").is_none(),
            "snake_case new_quantity must not appear"
        );
        assert!(
            json.get("clientOrderId").is_some(),
            "clientOrderId must be camelCase"
        );
        assert!(
            json.get("client_order_id").is_none(),
            "snake_case client_order_id must not appear"
        );

        // newQuantity must be a raw (unquoted) number, like newPrice.
        assert!(
            raw.contains("\"newQuantity\":500000000000000000"),
            "newQuantity must be a raw number, got: {}",
            raw
        );
        assert!(
            raw.contains("\"newPrice\":2000000000000000000000"),
            "newPrice must be a raw number, got: {}",
            raw
        );
    }

    #[test]
    fn test_create_perp_order_data_negative_price_or_qty_errors() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // Negative price → error.
        let neg_price = signer.create_perp_order_data(
            1,
            0,
            Decimal::from_str("-1").unwrap(),
            Decimal::from_str("0.1").unwrap(),
            false,
            0,
            None,
        );
        assert!(
            neg_price.is_err(),
            "negative price must error, got: {:?}",
            neg_price
        );

        // Negative quantity → error (independent of price sign).
        let neg_qty = signer.create_perp_order_data(
            1,
            0,
            Decimal::from_str("1000").unwrap(),
            Decimal::from_str("-0.1").unwrap(),
            false,
            0,
            None,
        );
        assert!(
            neg_qty.is_err(),
            "negative quantity must error, got: {:?}",
            neg_qty
        );
    }

    #[test]
    fn test_create_perp_modify_data_some_negative_errors() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        // Some(negative) new_price → error.
        let neg_price = signer.create_perp_modify_data(
            1,
            "0xORDER",
            Some(Decimal::from_str("-100").unwrap()),
            None,
            None,
        );
        assert!(
            neg_price.is_err(),
            "Some(negative) new_price must error, got: {:?}",
            neg_price
        );

        // Some(negative) new_quantity → error (independent leg).
        let neg_qty = signer.create_perp_modify_data(
            1,
            "0xORDER",
            None,
            Some(Decimal::from_str("-0.5").unwrap()),
            None,
        );
        assert!(
            neg_qty.is_err(),
            "Some(negative) new_quantity must error, got: {:?}",
            neg_qty
        );
    }

    #[test]
    fn test_create_perp_deposit_and_withdraw_negative_amount_errors() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let dep = signer.create_perp_deposit_data("2", Decimal::from_str("-1000").unwrap());
        assert!(
            dep.is_err(),
            "negative deposit amount must error, got: {:?}",
            dep
        );

        let wd = signer.create_perp_withdraw_data("2", Decimal::from_str("-1000").unwrap());
        assert!(
            wd.is_err(),
            "negative withdraw amount must error, got: {:?}",
            wd
        );
    }

    // =========================================================================
    // §3.5 to_onchain_units numeric guards
    // =========================================================================

    #[test]
    fn to_onchain_units_rejects_non_finite_with_finite_guard_before_negative_guard() {
        let nan = AlphaSecSigner::to_onchain_units(f64::NAN, 18);
        let nan_msg = nan.expect_err("NaN must error").to_string();
        assert!(
            nan_msg.contains("finite"),
            "NaN error must mention 'finite', got: {}",
            nan_msg
        );

        let pos_inf = AlphaSecSigner::to_onchain_units(f64::INFINITY, 18);
        let pos_msg = pos_inf.expect_err("+inf must error").to_string();
        assert!(
            pos_msg.contains("finite"),
            "+inf error must mention 'finite', got: {}",
            pos_msg
        );

        // NEG_INFINITY hits the finite guard first, never the negative guard.
        let neg_inf = AlphaSecSigner::to_onchain_units(f64::NEG_INFINITY, 18);
        let neg_msg = neg_inf.expect_err("-inf must error").to_string();
        assert!(
            neg_msg.contains("finite"),
            "-inf must hit the finite guard, got: {}",
            neg_msg
        );
        assert!(
            !neg_msg.contains("non-negative"),
            "-inf must NOT be reported as a negative-value error, got: {}",
            neg_msg
        );
    }

    #[test]
    fn to_onchain_units_rejects_negative_but_accepts_zero() {
        let neg = AlphaSecSigner::to_onchain_units(-1.0, 18);
        let neg_msg = neg.expect_err("negative must error").to_string();
        assert!(
            neg_msg.contains("non-negative"),
            "negative error must mention 'non-negative', got: {}",
            neg_msg
        );

        let zero = AlphaSecSigner::to_onchain_units(0.0, 18).expect("0.0 must be accepted");
        assert_eq!(zero, U256::zero(), "0.0 must convert to exactly 0");
    }

    #[test]
    fn to_onchain_units_overflow_guard_rejects_above_u128_max_but_passes_just_below() {
        // 1e30 * 10^18 = 1e48 >> u128::MAX (~3.4e38) -> must error, not clamp.
        let too_big = AlphaSecSigner::to_onchain_units(1e30, 18);
        assert!(
            too_big.is_err(),
            "1e30 at scale 18 must overflow, got: {:?}",
            too_big
        );

        // 3e20 * 10^18 = 3e38 < u128::MAX -> must pass (guard boundary actually bites).
        let just_below = AlphaSecSigner::to_onchain_units(3e20, 18)
            .expect("3e20 at scale 18 fits in u128 and must succeed");
        assert!(just_below > U256::zero());
    }

    #[test]
    fn to_onchain_units_truncates_toward_zero_not_rounds() {
        assert_eq!(
            AlphaSecSigner::to_onchain_units(1.9, 0).unwrap(),
            U256::from(1u8),
            "1.9 must truncate to 1"
        );
        assert_eq!(
            AlphaSecSigner::to_onchain_units(2.5, 0).unwrap(),
            U256::from(2u8),
            "2.5 must truncate to 2"
        );
        assert_eq!(
            AlphaSecSigner::to_onchain_units(0.4, 0).unwrap(),
            U256::zero(),
            "0.4 must truncate to 0"
        );
    }

    // =========================================================================
    // §3.5 session EIP-712 domain.chainId (get_chain_id branch)
    // =========================================================================

    #[test]
    fn session_typed_data_domain_chain_id_is_kaia_l1_id_per_network() {
        // Mainnet -> KAIA mainnet L1 id 8217 (the previously untested branch).
        let mainnet_signer = AlphaSecSigner::new(create_test_config_mainnet());
        let mainnet_td = mainnet_signer.create_session_register_typed_data(
            "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
            1,
            2,
        );
        assert_eq!(mainnet_td["domain"]["chainId"], 8217u64);

        // Kairos -> KAIA kairos L1 id 1001, explicitly NOT the AlphaSec L2 ids.
        let kairos_signer = AlphaSecSigner::new(create_test_config());
        let kairos_td = kairos_signer.create_session_register_typed_data(
            "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
            1,
            2,
        );
        assert_eq!(kairos_td["domain"]["chainId"], 1001u64);
        assert_ne!(
            kairos_td["domain"]["chainId"], 41001u64,
            "must not be the AlphaSec L2 testnet id"
        );
        assert_ne!(
            kairos_td["domain"]["chainId"], 48217u64,
            "must not be the AlphaSec L2 mainnet id"
        );
    }

    #[test]
    fn session_typed_data_domain_ignores_chain_id_override() {
        let config = create_test_config().with_chain_id(99999);
        let signer = AlphaSecSigner::new(config);

        let typed_data = signer.create_session_register_typed_data(
            "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
            1,
            2,
        );
        assert_eq!(
            typed_data["domain"]["chainId"], 1001u64,
            "EIP-712 domain must ignore the chain_id override"
        );
    }

    #[test]
    fn session_typed_data_with_numeric_expiry_nonce_encodes_eip712() {
        let signer = AlphaSecSigner::new(create_test_config());
        let typed_json = signer.create_session_register_typed_data(
            "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
            123456789,
            987654321,
        );

        // The JSON-Number representation of expiry/nonce is pinned by
        // test_session_register_typed_data; here the whole document must round-trip
        // through ethers and encode (encode_eip712 succeeding implies valid uint64s).
        let typed_data: Eip712TypedData =
            serde_json::from_value(typed_json).expect("typed data must parse as EIP-712");
        let digest = typed_data
            .encode_eip712()
            .expect("encode_eip712 must succeed with numeric uint64 fields");
        assert_ne!(digest, [0u8; 32], "digest must not be all-zero");
    }

    // =========================================================================
    // §3.5 generate_alphasec_transaction — RLP decode of the signed tx
    // =========================================================================

    #[tokio::test]
    async fn generated_tx_chain_id_defaults_to_alphasec_l2_ids_not_kaia_l1() {
        let kairos = AlphaSecSigner::new(create_test_config());
        let kairos_hex = kairos
            .generate_alphasec_transaction(Some(1), &[0x23], None)
            .await
            .unwrap();
        let kairos_tx = decode_signed_tx(&kairos_hex);
        let kairos_id = kairos_tx.chain_id().expect("chain id must be set").as_u64();
        assert_eq!(kairos_id, 41001, "Kairos default must be the L2 id 41001");
        assert_ne!(kairos_id, 1001, "must NOT be the KAIA L1 kairos id");

        let mainnet = AlphaSecSigner::new(create_test_config_mainnet());
        let mainnet_hex = mainnet
            .generate_alphasec_transaction(Some(1), &[0x23], None)
            .await
            .unwrap();
        let mainnet_tx = decode_signed_tx(&mainnet_hex);
        assert_eq!(
            mainnet_tx
                .chain_id()
                .expect("chain id must be set")
                .as_u64(),
            48217,
            "Mainnet default must be the L2 id 48217"
        );
    }

    #[tokio::test]
    async fn generated_tx_respects_explicit_chain_id_override() {
        let signer = AlphaSecSigner::new(create_test_config().with_chain_id(12345));
        let tx_hex = signer
            .generate_alphasec_transaction(Some(1), &[0x23], None)
            .await
            .unwrap();
        let tx = decode_signed_tx(&tx_hex);
        assert_eq!(
            tx.chain_id().expect("chain id must be set").as_u64(),
            12345,
            "L2 signing path must respect the chain_id override"
        );
    }

    #[tokio::test]
    async fn generated_tx_pins_to_value_data_and_nonce() {
        let signer = AlphaSecSigner::new(create_test_config());
        let calldata: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x7b];
        let timestamp_ms: u64 = 1_717_171_717_171;

        let tx_hex = signer
            .generate_alphasec_transaction(Some(timestamp_ms), &calldata, None)
            .await
            .unwrap();
        let tx = decode_signed_tx(&tx_hex);

        // to == order precompile contract
        let to = match tx.to().expect("to must be set") {
            ethers::types::NameOrAddress::Address(addr) => *addr,
            other => panic!("expected a plain address, got: {:?}", other),
        };
        assert_eq!(to, ALPHASEC_ORDER_CONTRACT_ADDR.parse::<Address>().unwrap());

        // value == 0 (expect() keeps this fail-loud like the to/data/nonce assertions:
        // an absent decoded value must flag the decode gap, not pass as 0)
        assert_eq!(
            *tx.value().expect("value must be set"),
            U256::zero(),
            "tx value must be exactly 0"
        );

        // data == exact input bytes
        assert_eq!(
            tx.data().expect("data must be set").as_ref(),
            calldata.as_slice(),
            "calldata must round-trip byte-for-byte"
        );

        // nonce == timestamp_ms argument
        assert_eq!(
            *tx.nonce().expect("nonce must be set"),
            U256::from(timestamp_ms),
            "nonce must be the provided timestamp_ms"
        );
    }

    #[tokio::test]
    async fn generated_tx_zero_timestamp_yields_nonce_zero() {
        let signer = AlphaSecSigner::new(create_test_config());
        let tx_hex = signer
            .generate_alphasec_transaction(Some(0), &[0x23], None)
            .await
            .unwrap();
        let tx = decode_signed_tx(&tx_hex);
        assert_eq!(
            *tx.nonce().expect("nonce must be set"),
            U256::zero(),
            "Some(0) timestamp must yield nonce 0, not the current time"
        );
    }

    #[tokio::test]
    async fn generated_tx_empty_calldata_is_ok_and_stays_empty() {
        let signer = AlphaSecSigner::new(create_test_config());
        let result = signer
            .generate_alphasec_transaction(Some(1), &[], None)
            .await;
        let tx_hex = result.expect("empty calldata must be signable");
        let tx = decode_signed_tx(&tx_hex);
        // ethers decodes an empty calldata byte string as an ABSENT data field (None) -
        // pin that exact representation instead of a lenient "None or empty" check:
        // injected bytes would surface as Some(non-empty) and fail loudly here.
        assert!(
            tx.data().is_none(),
            "empty calldata must decode to an absent data field, got: {:?}",
            tx.data()
        );
    }

    // =========================================================================
    // §3.5 transfer builders
    // =========================================================================

    #[test]
    fn token_transfer_space_stripping_corrupts_values_inside_strings() {
        let signer = AlphaSecSigner::new(create_test_config());
        let data = signer
            .create_token_transfer_data("0xrecipient", 1.0, "US DT")
            .unwrap();

        // Not a single space byte survives anywhere in the payload.
        assert!(
            !data[1..].contains(&b' '),
            "payload must contain no space bytes at all"
        );

        // The space inside the token VALUE was destroyed: "US DT" -> "USDT".
        let json: serde_json::Value = serde_json::from_slice(&data[1..]).unwrap();
        assert_eq!(
            json["token"], "USDT",
            "replace(\" \", \"\") must strip spaces inside values too"
        );
    }

    #[test]
    fn value_transfer_command_byte_0x02_and_unscaled_decimal_value() {
        let signer = AlphaSecSigner::new(create_test_config());
        let data = signer
            .create_value_transfer_data("0xrecipient", Decimal::from_str("1.5").unwrap())
            .unwrap();

        assert_eq!(
            data[0], 0x02,
            "first byte must be DEX_COMMAND_TRANSFER (0x02)"
        );

        let json: serde_json::Value = serde_json::from_slice(&data[1..]).unwrap();
        assert_eq!(
            json["value"], "1.5",
            "value must be the unscaled human amount"
        );
    }

    #[test]
    fn transfer_builders_do_not_share_numeric_formatting() {
        let signer = AlphaSecSigner::new(create_test_config());

        // Decimal path: trailing zero is preserved.
        let value_tx = signer
            .create_value_transfer_data("0xrecipient", Decimal::from_str("100.0").unwrap())
            .unwrap();
        let value_json: serde_json::Value = serde_json::from_slice(&value_tx[1..]).unwrap();
        assert_eq!(
            value_json["value"], "100.0",
            "Decimal path must keep the scale"
        );

        // f64 path: Display drops the trailing zero for the same human amount.
        let token_tx = signer
            .create_token_transfer_data("0xrecipient", 100.0, "USDT")
            .unwrap();
        let token_json: serde_json::Value = serde_json::from_slice(&token_tx[1..]).unwrap();
        assert_eq!(token_json["value"], "100", "f64 path must drop the scale");

        assert_ne!(
            value_json["value"], token_json["value"],
            "same human amount must format differently on the two paths"
        );
    }

    #[test]
    fn transfer_payload_l1owner_is_key_derived_lowercase_without_debug_quotes() {
        let signer = AlphaSecSigner::new(create_test_config());
        let expected = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";

        let value_tx = signer
            .create_value_transfer_data("0xrecipient", Decimal::from_str("1").unwrap())
            .unwrap();
        let token_tx = signer
            .create_token_transfer_data("0xrecipient", 1.0, "USDT")
            .unwrap();

        for (name, payload) in [("value", value_tx), ("token", token_tx)] {
            let json: serde_json::Value = serde_json::from_slice(&payload[1..]).unwrap();
            let owner = json["l1owner"].as_str().expect("l1owner must be a string");
            assert_eq!(
                owner, expected,
                "{} transfer l1owner must be lowercase key-derived",
                name
            );
            assert!(
                !owner.contains('"'),
                "{} transfer l1owner must not contain Debug-quote artifacts: {}",
                name,
                owner
            );
        }
    }

    // =========================================================================
    // §3.5 create_order_data — market/limit branch + TPSL gate
    // =========================================================================

    #[test]
    fn market_order_keeps_quantity_raw_but_normalizes_price() {
        let signer = AlphaSecSigner::new(create_test_config());
        let data = signer
            .create_order_data(
                "KAIA",
                "USDT",
                0,
                Decimal::from_str("0.123456789").unwrap(),
                Decimal::from_str("123.456789").unwrap(),
                OrderType::Market as u32, // 1
                0,
                None,
                None,
                None,
            )
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&data[1..]).unwrap();

        // Quantity passes through untouched on the Market branch.
        assert_eq!(
            json["quantity"], "123.456789",
            "Market order quantity must stay unnormalized"
        );
        // Price is still normalized (band >= 0.1 -> 5 dp).
        assert_eq!(
            json["price"], "0.12346",
            "Market order price must be normalized"
        );
    }

    #[test]
    fn limit_order_normalizes_both_price_and_quantity() {
        let signer = AlphaSecSigner::new(create_test_config());
        let data = signer
            .create_order_data(
                "KAIA",
                "USDT",
                0,
                Decimal::from_str("0.123456789").unwrap(),
                Decimal::from_str("123.456789").unwrap(),
                OrderType::Limit as u32, // 0
                0,
                None,
                None,
                None,
            )
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&data[1..]).unwrap();

        assert_eq!(
            json["price"], "0.12346",
            "Limit order price must be normalized"
        );
        assert_eq!(
            json["quantity"], "123.5",
            "Limit order quantity must be normalized by the price band"
        );
    }

    #[test]
    fn sl_limit_alone_is_silently_dropped_from_tpsl() {
        let signer = AlphaSecSigner::new(create_test_config());
        let price = Decimal::from_str("1.2345").unwrap();
        let qty = Decimal::from_str("10").unwrap();
        let aux = Decimal::from_str("1.1").unwrap();

        // sl_limit ALONE -> no tpsl key at all (silently dropped).
        let sl_limit_only = signer
            .create_order_data("KAIA", "USDT", 0, price, qty, 0, 0, None, None, Some(aux))
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&sl_limit_only[1..]).unwrap();
        assert!(
            json.get("tpsl").is_none(),
            "sl_limit alone must NOT produce a tpsl key, got: {}",
            json
        );

        // sl_trigger alone DOES open the gate.
        let sl_trigger_only = signer
            .create_order_data("KAIA", "USDT", 0, price, qty, 0, 0, None, Some(aux), None)
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&sl_trigger_only[1..]).unwrap();
        assert!(
            json.get("tpsl").is_some(),
            "sl_trigger alone must produce tpsl"
        );
        assert_eq!(json["tpsl"]["slTrigger"], "1.1");

        // tp_limit alone DOES open the gate.
        let tp_limit_only = signer
            .create_order_data("KAIA", "USDT", 0, price, qty, 0, 0, Some(aux), None, None)
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&tp_limit_only[1..]).unwrap();
        assert!(
            json.get("tpsl").is_some(),
            "tp_limit alone must produce tpsl"
        );
        assert_eq!(json["tpsl"]["tpLimit"], "1.1");
    }
}
