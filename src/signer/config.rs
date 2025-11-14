//! Configuration for AlphaSec SDK

use crate::error::{AlphaSecError, Result};
use ethers::signers::{LocalWallet, Signer};
use std::str::FromStr;
use url::Url;

/// Network type for AlphaSec
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Network {
    /// Mainnet (production)
    Mainnet,
    /// Kairos testnet
    Kairos,
}

impl FromStr for Network {
    type Err = AlphaSecError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "mainnet" => Ok(Network::Mainnet),
            "kairos" => Ok(Network::Kairos),
            _ => Err(AlphaSecError::config(
                "Invalid network. Use 'mainnet' or 'kairos'",
            )),
        }
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::Mainnet => write!(f, "mainnet"),
            Network::Kairos => write!(f, "kairos"),
        }
    }
}

/// Configuration for AlphaSec client
#[derive(Debug, Clone)]
pub struct Config {
    /// Chain ID
    pub chain_id: Option<u64>,

    /// API base URL
    pub api_url: Url,

    /// WebSocket URL for real-time data  
    pub ws_url: Url,

    /// Network (mainnet or kairos)
    pub network: Network,

    /// L1 wallet address (Kaia blockchain)
    pub l1_address: String,

    /// l1 wallet
    pub l1_wallet: Option<LocalWallet>,

    /// l2 wallet
    pub l2_wallet: Option<LocalWallet>,

    /// Whether session is enabled (use L2 wallet for signing)
    pub session_enabled: bool,

    /// Request timeout in seconds
    pub timeout_secs: u64,

    /// Maximum retry attempts for failed requests
    pub max_retries: u32,
}

impl Config {
    /// Create a new configuration
    ///
    /// # Arguments
    ///
    /// * `api_url` - The API base URL (e.g., "https://api-testnet.alphasec.trade")
    /// * `network` - Network name ("mainnet" or "kairos")
    /// * `l1_address` - L1 wallet address (0x... format)
    /// * `private_key` - Private key (hex string without 0x prefix)
    /// * `session_enabled` - Whether to use session mode (L2 key) or direct L1 key
    pub fn new(
        _api_url: &str,
        _network: &str,
        _l1_address: &str,
        _l1_private_key: Option<&str>,
        _l2_private_key: Option<&str>,
        _session_enabled: bool,
        _chain_id: Option<u64>,
    ) -> Result<Self> {
        let api_url = Url::parse(_api_url).map_err(|_| AlphaSecError::config("Invalid API URL"))?;

        // Convert HTTP URL to WebSocket URL and add /ws path
        let ws_url = {
            let mut url = api_url.clone();
            match url.scheme() {
                "https" => url.set_scheme("wss").unwrap(),
                "http" => url.set_scheme("ws").unwrap(),
                _ => return Err(AlphaSecError::config("Unsupported URL scheme")),
            }
            // Add /ws path if not already present
            if !url.path().ends_with("/ws") {
                let path = if url.path() == "/" {
                    "/ws".to_string()
                } else {
                    format!("{}/ws", url.path())
                };
                url.set_path(&path);
            }
            url
        };

        let network = Network::from_str(_network)?;

        // Validate L1 address format (only if we won't derive from key below)
        if _l1_private_key.is_none() {
            if !_l1_address.starts_with("0x") || _l1_address.len() != 42 {
                return Err(AlphaSecError::config(
                    "Invalid L1 address format. Expected 0x followed by 40 hex characters",
                ));
            }
        }

        // Parse the private key and create wallet
        let l1_wallet = _l1_private_key.and_then(|key| LocalWallet::from_str(key).ok());
        let l2_wallet = _l2_private_key.and_then(|key| LocalWallet::from_str(key).ok());

        // If a private key is provided, derive the address from it to avoid mismatches
        let resolved_l1_address = if let Some(ref wallet) = l1_wallet {
            format!("0x{:x}", wallet.address())
        } else {
            _l1_address.to_string()
        };

        let chain_id = if _chain_id.is_some() { _chain_id } else { None };

        Ok(Self {
            chain_id,
            api_url,
            ws_url,
            network,
            l1_address: resolved_l1_address,
            l1_wallet,
            l2_wallet,
            session_enabled: _session_enabled,
            timeout_secs: 30,
            max_retries: 3,
        })
    }

    /// Set the request timeout
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Set the maximum retry attempts
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the chain ID
    pub fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Get the wallet address
    pub fn l1_address(&self) -> String {
        format!("{:?}", self.l1_address)
    }

    /// Check if this is mainnet
    pub fn is_mainnet(&self) -> bool {
        self.network == Network::Mainnet
    }

    /// Check if this is kairos testnet
    pub fn is_kairos(&self) -> bool {
        self.network == Network::Kairos
    }

    /// Get the wallet
    pub fn get_wallet(&self) -> Result<&LocalWallet> {
        if self.session_enabled {
            self.l2_wallet
                .as_ref()
                .ok_or_else(|| AlphaSecError::config("L2 wallet is not available"))
        } else {
            self.l1_wallet
                .as_ref()
                .ok_or_else(|| AlphaSecError::config("L1 wallet is not available"))
        }
    }

    /// Get the chain ID for the current network
    pub fn get_chain_id(&self) -> u64 {
        match self.network {
            Network::Mainnet => crate::types::chain_ids::KAIA_MAINNET_CHAIN_ID,
            Network::Kairos => crate::types::chain_ids::KAIA_KAIROS_CHAIN_ID,
        }
    }
}
