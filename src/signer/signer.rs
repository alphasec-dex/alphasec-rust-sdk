//! AlphaSec transaction signer with EIP-712 support

use crate::{
    error::{AlphaSecError, Result},
    signer::{config::Config, normalize_price_quantity, transaction::*},
    types::{
        chain_ids::ALPHASEC_MAINNET_CHAIN_ID,
        chain_ids::ALPHASEC_TESTNET_CHAIN_ID,
        constants::{abi::*, l1_contracts::*, ALPHASEC_NATIVE_TOKEN_ID},
        dex_commands::*,
        eip712::*,
        gas::*,
        l2_contracts::ALPHASEC_ORDER_CONTRACT_ADDR,
    },
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
use serde_json;
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

    /// Generate alphasec-style nonce (timestamp + counter)
    fn get_alphasec_nonce(&self) -> u64 {
        Self::current_timestamp_ms() + self.nonce_counter.fetch_add(1, Ordering::SeqCst)
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
    pub fn create_value_transfer_data(&self, to: &str, value: f64) -> Result<Vec<u8>> {
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
        price: f64,
        quantity: f64,
        order_type: u32,
        order_mode: u32,
        tp_limit: Option<f64>,
        sl_trigger: Option<f64>,
        sl_limit: Option<f64>,
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
            price: (normalized_price as f64).to_string(),
            quantity: (normalized_quantity as f64).to_string(),
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
        new_price: f64,
        new_qty: f64,
        order_mode: u32,
    ) -> Result<Vec<u8>> {
        let (normalized_price, normalized_qty) = normalize_price_quantity(new_price, new_qty)?;
        let model = ModifyModel {
            l1owner: self.l1_address().to_string(), // Use l1_address
            order_id: order_id.to_string(),
            new_price: (normalized_price as f64).to_string(),
            new_qty: (normalized_qty as f64).to_string(),
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
        stop_price: f64,
        price: f64,
        quantity: f64,
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
            stop_price: (normalized_stop_price as f64).to_string(),
            price: (normalized_price as f64).to_string(),
            quantity: (normalized_quantity as f64).to_string(),
            side,
            order_type,
            order_mode,
        };

        // Use model's to_wire method for alphasec-style encoding
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
        let nonce = timestamp_ms.unwrap_or_else(|| self.get_alphasec_nonce());

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
        let value_onchain_unit = (value * 10_f64.powi(decimals as i32)) as u64;

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
            if allowance < value_onchain_unit.into() {
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
        // All tokens have 18 decimals in AlphaSec L2
        let value_onchain_unit = (value * 1e18) as u64;

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
            let nonce = (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()) as u64;

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
                TypedTransaction::Eip2930(mut inner) => TypedTransaction::Eip2930(inner),
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

            let erc20_router_addr =
                crate::types::l2_contracts::ALPHASEC_GATEWAY_ROUTER_CONTRACT_ADDR;

            // Parse L2 ERC20 Router ABI and create contract instance
            let abi: Abi = serde_json::from_str(L2_ERC20_ROUTER_ABI).map_err(|e| {
                AlphaSecError::generic(&format!("Failed to parse L2 ERC20 Router ABI: {}", e))
            })?;

            let router_address: Address = erc20_router_addr.parse().map_err(|e| {
                AlphaSecError::invalid_address(&format!("Invalid router address: {}", e))
            })?;

            let contract = Contract::new(router_address, abi, l2_provider.clone());

            // Generate nonce using current timestamp
            let nonce = (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()) as u64;

            // Prepare data for outbound transfer
            let data = Bytes::from_static(b"0x");
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
    use crate::signer::Config;

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

    #[test]
    fn test_signer_creation() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        assert_eq!(
            signer.l1_address(),
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
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
        let nonce = 123456789;
        let expiry = 987654321;

        let typed_data = signer.create_session_register_typed_data(session_addr, nonce, expiry);

        // Check domain
        assert_eq!(typed_data["domain"]["name"], DOMAIN_NAME);
        assert_eq!(typed_data["domain"]["version"], DOMAIN_VERSION);
        assert_eq!(typed_data["domain"]["chainId"], 1001);
        assert_eq!(
            typed_data["domain"]["verifyingContract"],
            VERIFYING_CONTRACT
        );

        // Check message
        assert_eq!(typed_data["message"]["sessionWallet"], session_addr);
        assert_eq!(typed_data["message"]["nonce"], nonce.to_string());
        assert_eq!(typed_data["message"]["expiry"], expiry.to_string());

        // Check primary type
        assert_eq!(typed_data["primaryType"], "RegisterSessionWallet");
    }

    #[test]
    fn test_create_value_transfer_data() {
        let config = create_test_config();
        let signer = AlphaSecSigner::new(config);

        let to = "0xrecipientaddressrecipientaddressrecipient";
        let value = 1f64; // 1 KAIA

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
        let price = 50000f64; // $50,000
        let quantity = 1f64; // 1 BTC
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
        let new_price = 51000f64; // New price $51,000
        let new_qty = 2f64; // New quantity 2 BTC
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
        let stop_price = 3000f64; // $3,000
        let price = 2950f64; // $2,950
        let quantity = 1f64; // 1 ETH
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

        let url = url::Url::parse("https://public-en-kairos.node.kaia.io").unwrap();
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
        let url = url::Url::parse("http://app-testnet.alphasec.trade").unwrap();
        let http = ethers::providers::Http::new(url);
        let provider = Arc::new(ethers::providers::Provider::new(http));

        // Test that the function compiles and runs (will fail on network call, but that's expected)
        let result = signer
            .generate_withdraw_transaction(&provider, "1", 1f64, None)
            .await;

        // We expect this to fail due to network connection, but the function should be callable
        // assert!(result.is_err());

        println!("result: {:#?}", result);
        assert!(result.is_ok());
    }
}
