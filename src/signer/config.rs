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

    /// WebSocket API URL for trade operations (`/ws-api`)
    pub ws_api_url: Url,

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

        // Derive WebSocket Trade API URL (/ws-api) for low-latency order operations
        let ws_api_url = {
            let mut url = api_url.clone();
            match url.scheme() {
                "https" => url.set_scheme("wss").unwrap(),
                "http" => url.set_scheme("ws").unwrap(),
                _ => return Err(AlphaSecError::config("Unsupported URL scheme")),
            }
            let path = if url.path() == "/" {
                "/ws-api".to_string()
            } else {
                format!("{}/ws-api", url.path().trim_end_matches('/'))
            };
            url.set_path(&path);
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
            ws_api_url,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Well-known dev key (Anvil/Hardhat account #0). Test-only constant.
    const DEV_KEY_1: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    /// Address derived from DEV_KEY_1, lowercase as produced by `format!("0x{:x}", ..)`.
    const DEV_KEY_1_ADDR: &str = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
    /// Well-known dev key (Anvil/Hardhat account #1). Used to prove WHICH wallet a branch picks.
    const DEV_KEY_2: &str = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
    /// Address derived from DEV_KEY_2.
    const DEV_KEY_2_ADDR: &str = "0x70997970c51812dc3a010c7d01b50e0d17dc79c8";

    /// Build a keyless config for URL-derivation tests.
    fn base_config(api_url: &str) -> Config {
        Config::new(api_url, "kairos", DEV_KEY_1_ADDR, None, None, false, None)
            .expect("base config should build")
    }

    // ---- Network::from_str ----

    #[test]
    fn network_from_str_is_case_insensitive_via_lowercase() {
        assert_eq!(Network::from_str("MAINNET").unwrap(), Network::Mainnet);
        assert_eq!(Network::from_str("Mainnet").unwrap(), Network::Mainnet);
        assert_eq!(Network::from_str("mAiNnEt").unwrap(), Network::Mainnet);
        assert_eq!(Network::from_str("KAIROS").unwrap(), Network::Kairos);
        assert_eq!(Network::from_str("kAiRoS").unwrap(), Network::Kairos);
    }

    #[test]
    fn network_from_str_rejects_surrounding_whitespace() {
        assert!(Network::from_str(" mainnet").is_err());
        assert!(Network::from_str("mainnet ").is_err());
        assert!(Network::from_str(" kairos ").is_err());
        assert!(Network::from_str("\tmainnet\n").is_err());
    }

    #[test]
    fn network_from_str_rejects_empty_unknown_and_aliases() {
        assert!(Network::from_str("").is_err());
        assert!(Network::from_str("testnet").is_err());
        assert!(Network::from_str("main").is_err());
    }

    #[test]
    fn network_display_roundtrips_through_from_str_in_lowercase() {
        assert_eq!(Network::Mainnet.to_string(), "mainnet");
        assert_eq!(Network::Kairos.to_string(), "kairos");
        for n in [Network::Mainnet, Network::Kairos] {
            assert_eq!(Network::from_str(&n.to_string()).unwrap(), n);
        }
    }

    // ---- get_chain_id ----

    #[test]
    fn get_chain_id_maps_networks_to_raw_kaia_ids() {
        let mainnet = Config::new(
            "https://h",
            "mainnet",
            DEV_KEY_1_ADDR,
            None,
            None,
            false,
            None,
        )
        .unwrap();
        assert_eq!(mainnet.get_chain_id(), 8217);
        assert_eq!(base_config("https://h").get_chain_id(), 1001);
    }

    #[test]
    fn get_chain_id_ignores_with_chain_id_override() {
        let cfg = Config::new(
            "https://h",
            "mainnet",
            DEV_KEY_1_ADDR,
            None,
            None,
            false,
            None,
        )
        .unwrap()
        .with_chain_id(999999);
        assert_eq!(cfg.chain_id, Some(999999), "override must be stored");
        assert_eq!(cfg.get_chain_id(), 8217, "but get_chain_id must ignore it");
    }

    // ---- ws_url derivation ----

    #[test]
    fn ws_url_swaps_scheme_appends_ws_and_preserves_port() {
        assert_eq!(base_config("https://h").ws_url.as_str(), "wss://h/ws");
        assert_eq!(
            base_config("http://localhost:8080").ws_url.as_str(),
            "ws://localhost:8080/ws"
        );
    }

    #[test]
    fn ws_url_root_trailing_slash_does_not_double_slash() {
        let cfg = base_config("https://h/");
        assert_eq!(cfg.ws_url.path(), "/ws");
        assert_eq!(cfg.ws_url.as_str(), "wss://h/ws");
    }

    #[test]
    fn ws_url_derivation_is_idempotent_for_existing_ws_path() {
        let cfg = base_config("https://h/ws");
        assert_eq!(cfg.ws_url.as_str(), "wss://h/ws");
    }

    #[test]
    fn unsupported_schemes_rejected_for_ws_derivation() {
        for url in ["ftp://h", "wss://h"] {
            let err = Config::new(url, "kairos", DEV_KEY_1_ADDR, None, None, false, None)
                .expect_err("non-http(s) scheme must be rejected");
            assert!(
                err.to_string().contains("Unsupported URL scheme"),
                "unexpected error for {url}: {err}"
            );
        }
    }

    // ---- ws_api_url derivation (asymmetry vs ws_url) ----

    #[test]
    fn ws_api_url_appends_ws_api_and_trims_trailing_slashes() {
        assert_eq!(
            base_config("https://h").ws_api_url.as_str(),
            "wss://h/ws-api"
        );
        let cfg = base_config("https://h/api/");
        assert_eq!(
            cfg.ws_api_url.as_str(),
            "wss://h/api/ws-api",
            "ws_api_url trims trailing '/' before appending"
        );
        assert_eq!(
            cfg.ws_url.as_str(),
            "wss://h/api//ws",
            "ws_url has NO trailing-slash trim for non-root paths (intended asymmetry)"
        );
    }

    #[test]
    fn ws_api_url_derivation_is_not_idempotent() {
        let cfg = base_config("https://h/ws-api");
        assert_eq!(cfg.ws_api_url.as_str(), "wss://h/ws-api/ws-api");
    }

    // ---- B2 regression: derived WS host can never differ from api host ----

    #[test]
    fn b2_derived_ws_urls_always_share_api_host() {
        let cfg = base_config("https://api.alphasec.trade");
        assert_eq!(cfg.ws_url.as_str(), "wss://api.alphasec.trade/ws");
        assert_eq!(cfg.ws_url.host_str(), cfg.api_url.host_str());
        assert_eq!(cfg.ws_api_url.host_str(), cfg.api_url.host_str());
    }

    // NOTE (intentionally NOT tested): ws_url/ws_api_url are plain pub fields, so "assignment
    // sticks" and "fields are independent" are guaranteed by language semantics and cannot fail
    // at runtime. Per CLAUDE.md, a test whose failure mode the compiler already catches must
    // not be written.

    // ---- address validation vs key derivation ----

    #[test]
    fn key_derived_address_overrides_passed_l1_address() {
        let cfg = Config::new(
            "https://h",
            "kairos",
            "0x0000000000000000000000000000000000000001",
            Some(DEV_KEY_1),
            None,
            false,
            None,
        )
        .unwrap();
        assert_eq!(cfg.l1_address, DEV_KEY_1_ADDR);
    }

    #[test]
    fn address_format_validation_skipped_when_key_present() {
        let cfg = Config::new(
            "https://h",
            "kairos",
            "garbage",
            Some(DEV_KEY_1),
            None,
            false,
            None,
        )
        .expect("garbage address must be ignored when a key is provided");
        assert_eq!(cfg.l1_address, DEV_KEY_1_ADDR);
    }

    #[test]
    fn address_format_validated_without_key() {
        // 41 chars: "0x" + 39 hex chars. Assert the address-format message specifically so
        // an unrelated config error (URL/network) cannot satisfy this test.
        let short = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb9226";
        assert_eq!(short.len(), 41);
        let err = Config::new("https://h", "kairos", short, None, None, false, None).unwrap_err();
        assert!(
            err.to_string().contains("Invalid L1 address format"),
            "expected the address-format guard, got: {err}"
        );
        // 42 chars but no 0x prefix: prefix check must fire independently of length.
        let unprefixed = "f39fd6e51aad88f6f4ce6ab8827279cfffb92266ff";
        assert_eq!(unprefixed.len(), 42);
        let err =
            Config::new("https://h", "kairos", unprefixed, None, None, false, None).unwrap_err();
        assert!(
            err.to_string().contains("Invalid L1 address format"),
            "expected the address-format guard, got: {err}"
        );
    }

    #[test]
    fn non_hex_address_content_currently_accepted_without_key() {
        let non_hex = format!("0x{}", "z".repeat(40));
        assert_eq!(non_hex.len(), 42);
        let cfg = Config::new("https://h", "kairos", &non_hex, None, None, false, None)
            .expect("content is not hex-validated today");
        assert_eq!(cfg.l1_address, non_hex);
    }

    // ---- builder ----

    #[test]
    fn chain_id_last_write_wins_and_some_zero_preserved() {
        let cfg = base_config("https://h").with_chain_id(1).with_chain_id(2);
        assert_eq!(cfg.chain_id, Some(2));

        let zero = Config::new(
            "https://h",
            "kairos",
            DEV_KEY_1_ADDR,
            None,
            None,
            false,
            Some(0),
        )
        .unwrap();
        assert_eq!(zero.chain_id, Some(0));
    }

    #[test]
    fn with_timeout_leaves_max_retries_and_zero_retries_allowed() {
        let cfg = base_config("https://h");
        assert_eq!((cfg.timeout_secs, cfg.max_retries), (30, 3), "defaults");

        let cfg = cfg.with_timeout(7);
        assert_eq!(cfg.timeout_secs, 7);
        assert_eq!(
            cfg.max_retries, 3,
            "with_timeout must not touch max_retries"
        );

        let cfg = cfg.with_max_retries(0);
        assert_eq!(cfg.max_retries, 0, "zero retries is allowed");
    }

    // ---- l1_address() quirk ----

    #[test]
    fn l1_address_getter_returns_debug_quoted_string() {
        let cfg = base_config("https://h");
        let got = cfg.l1_address();
        assert_ne!(got, cfg.l1_address, "getter output differs from raw field");
        assert!(got.starts_with('"') && got.ends_with('"'), "got: {got}");
        assert_eq!(got, format!("\"{}\"", cfg.l1_address));
    }

    // ---- get_wallet session branch ----

    #[test]
    fn get_wallet_session_true_without_l2_errs_no_l1_fallback() {
        let cfg = Config::new(
            "https://h",
            "kairos",
            DEV_KEY_1_ADDR,
            Some(DEV_KEY_1),
            None,
            true,
            None,
        )
        .unwrap();
        let err = cfg
            .get_wallet()
            .expect_err("session mode must not fall back to L1");
        assert!(err.to_string().contains("L2 wallet"), "got: {err}");
    }

    #[test]
    fn get_wallet_session_false_without_l1_errs() {
        let cfg = Config::new(
            "https://h",
            "kairos",
            DEV_KEY_1_ADDR,
            None,
            Some(DEV_KEY_2),
            false,
            None,
        )
        .unwrap();
        let err = cfg.get_wallet().expect_err("must require L1 wallet");
        assert!(err.to_string().contains("L1 wallet"), "got: {err}");
    }

    #[test]
    fn get_wallet_selects_wallet_matching_session_flag() {
        let mk = |session: bool| {
            Config::new(
                "https://h",
                "kairos",
                DEV_KEY_1_ADDR,
                Some(DEV_KEY_1),
                Some(DEV_KEY_2),
                session,
                None,
            )
            .unwrap()
        };

        let l2 = mk(true);
        let addr = format!("0x{:x}", l2.get_wallet().unwrap().address());
        assert_eq!(
            addr, DEV_KEY_2_ADDR,
            "session=true must return the L2 wallet"
        );
        assert_ne!(addr, DEV_KEY_1_ADDR);

        let l1 = mk(false);
        let addr = format!("0x{:x}", l1.get_wallet().unwrap().address());
        assert_eq!(
            addr, DEV_KEY_1_ADDR,
            "session=false must return the L1 wallet"
        );
    }
}
