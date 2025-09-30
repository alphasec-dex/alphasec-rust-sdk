//! Constants used throughout the AlphaSec SDK

/// AlphaSec contract addresses and constants
pub mod l2_contracts {
    /// AlphaSec Gateway Router contract address
    pub const ALPHASEC_GATEWAY_ROUTER_CONTRACT_ADDR: &str = "0xD2b30f9548DEE14093CF903ec70866469EFff97A";
    
    /// AlphaSec Order contract address  
    pub const ALPHASEC_ORDER_CONTRACT_ADDR: &str = "0x00000000000000000000000000000000000000cc";
    
    /// AlphaSec System contract address
    pub const ALPHASEC_SYSTEM_CONTRACT_ADDR: &str = "0x0000000000000000000000000000000000000064";
    
    /// AlphaSec ZK Interface contract address
    pub const ALPHASEC_ZK_INTERFACE_CONTRACT_ADDR: &str = "0x0000000000000000000000000000000000000000";
}

/// L1 contract addresses for deposit/withdrawal
pub mod l1_contracts {
    /// Mainnet Inbox contract address
    pub const MAINNET_INBOX_CONTRACT_ADDR: &str = "0x6EE619c6E74e34a802279437e22c98633c28643e";
    
    /// Kairos Inbox contract address
    pub const KAIROS_INBOX_CONTRACT_ADDR: &str = "0x6EE619c6E74e34a802279437e22c98633c28643e";
    
    /// Mainnet ERC20 Gateway contract address
    pub const MAINNET_ERC20_GATEWAY_CONTRACT_ADDR: &str = "0xec5cD95184124Ee2cc4C90fb7f74E3b717160d51";
    
    /// Kairos ERC20 Gateway contract address
    pub const KAIROS_ERC20_GATEWAY_CONTRACT_ADDR: &str = "0xec5cD95184124Ee2cc4C90fb7f74E3b717160d51";
    
    /// Mainnet ERC20 Router contract address
    pub const MAINNET_ERC20_ROUTER_CONTRACT_ADDR: &str = "0x6c1f5fef508715b6E1a541594046DB2831f0F6CE";
   
    /// Kairos ERC20 Router contract address
    pub const KAIROS_ERC20_ROUTER_CONTRACT_ADDR: &str = "0x6c1f5fef508715b6E1a541594046DB2831f0F6CE";
}

/// Chain IDs for different networks
pub mod chain_ids {
    /// AlphaSec L2 chain ID
    pub const ALPHASEC_CHAIN_ID: u64 = 41001;
    
    /// Kaia mainnet chain ID
    pub const KAIA_MAINNET_CHAIN_ID: u64 = 8217;
    
    /// Kaia testnet (Kairos) chain ID
    pub const KAIA_KAIROS_CHAIN_ID: u64 = 1001;
}

/// Session command types
pub mod session_commands {
    /// Session command
    pub const SESSION_COMMAND_CREATE: u8 = 0x01;
    /// Session command
    pub const SESSION_COMMAND_UPDATE: u8 = 0x02;
    /// Session command
    pub const SESSION_COMMAND_DELETE: u8 = 0x03;
}

/// DEX command types
pub mod dex_commands {
    /// Session command
    pub const DEX_COMMAND_SESSION: u8 = 0x01;
    
    /// Transfer command
    pub const DEX_COMMAND_TRANSFER: u8 = 0x02;
    
    /// Token transfer command
    pub const DEX_COMMAND_TOKEN_TRANSFER: u8 = 0x11;
    
    /// Order command (alphasec style)
    pub const DEX_COMMAND_ORDER: u8 = 0x21;
    
    /// Cancel command (alphasec style)
    pub const DEX_COMMAND_CANCEL: u8 = 0x22;
    
    /// Cancel all command (alphasec style)
    pub const DEX_COMMAND_CANCEL_ALL: u8 = 0x23;
    
    /// Modify command (alphasec style)
    pub const DEX_COMMAND_MODIFY: u8 = 0x24;
    
    /// Stop order command (alphasec style)
    pub const DEX_COMMAND_STOP_ORDER: u8 = 0x25;
}

/// Native token ID
pub const ALPHASEC_NATIVE_TOKEN_ID: u32 = 1;

/// Default API endpoints
pub mod endpoints {
    /// AlphaSec API base URL
    pub const ALPHASEC_API_URL: &str = "https://api-testnet.alphasec.trade";
    
    /// AlphaSec mainnet L2 RPC URL
    pub const ALPHASEC_MAINNET_URL: &str = "https://rpc.alphasec.trade";
    
    /// AlphaSec kairos L2 RPC URL
    pub const ALPHASEC_KAIROS_URL: &str = "https://kairos-rpc.alphasec.trade";
    
    /// Kaia mainnet RPC URL
    pub const KAIA_MAINNET_URL: &str = "https://public-en-cypress.klaytn.net";
    
    /// Kaia kairos RPC URL
    pub const KAIA_KAIROS_URL: &str = "https://public-en-kairos.node.kaia.io";
}

/// Gas and transaction constants
pub mod gas {
    /// Default gas limit for AlphaSec transactions
    pub const DEFAULT_GAS_LIMIT: u64 = 1_000_000;
    
    /// Default gas price (0 for AlphaSec L2)
    pub const DEFAULT_GAS_PRICE: u64 = 0;
    
    /// Default max fee per gas (0 for AlphaSec L2)
    pub const DEFAULT_MAX_FEE_PER_GAS: u64 = 0;
    
    /// Default max priority fee per gas (0 for AlphaSec L2)
    pub const DEFAULT_MAX_PRIORITY_FEE_PER_GAS: u64 = 0;
}

/// EIP-712 domain constants
pub mod eip712 {
    /// EIP-712 domain name
    pub const DOMAIN_NAME: &str = "DEXSignTransaction";
    
    /// EIP-712 domain version
    pub const DOMAIN_VERSION: &str = "1";
    
    /// EIP-712 verifying contract (zero address)
    pub const VERIFYING_CONTRACT: &str = "0x0000000000000000000000000000000000000000";
}

/// Contract ABI constants
pub mod abi {
    /// Native L1 Inbox ABI for ETH deposits
    pub const NATIVE_L1_ABI: &str = r#"[
        {
            "inputs": [],
            "name": "depositEth",
            "outputs": [],
            "stateMutability": "payable",
            "type": "function"
        }
    ]"#;

    /// ERC20 ABI for token operations
    pub const ERC20_ABI: &str = r#"[
        {
            "inputs": [
                {"internalType": "address", "name": "spender", "type": "address"},
                {"internalType": "uint256", "name": "amount", "type": "uint256"}
            ],
            "name": "approve",
            "outputs": [{"internalType": "bool", "name": "", "type": "bool"}],
            "stateMutability": "nonpayable",
            "type": "function"
        },
        {
            "inputs": [
                {"internalType": "address", "name": "owner", "type": "address"},
                {"internalType": "address", "name": "spender", "type": "address"}
            ],
            "name": "allowance",
            "outputs": [{"internalType": "uint256", "name": "", "type": "uint256"}],
            "stateMutability": "view",
            "type": "function"
        }
    ]"#;

    /// ERC20 Router ABI for L1 deposits
    pub const ERC20_ROUTER_ABI: &str = r#"[
        {
            "inputs": [
                {"internalType": "address", "name": "token", "type": "address"},
                {"internalType": "address", "name": "to", "type": "address"},
                {"internalType": "uint256", "name": "amount", "type": "uint256"},
                {"internalType": "uint256", "name": "maxGas", "type": "uint256"},
                {"internalType": "uint256", "name": "gasPriceBid", "type": "uint256"},
                {"internalType": "bytes", "name": "data", "type": "bytes"}
            ],
            "name": "outboundTransfer",
            "outputs": [{"internalType": "bytes", "name": "", "type": "bytes"}],
            "stateMutability": "payable",
            "type": "function"
        }
    ]"#;

    /// L2 System ABI for ETH withdrawals
    pub const L2_SYSTEM_ABI: &str = r#"[
        {
            "inputs": [{"internalType": "address", "name": "to", "type": "address"}],
            "name": "withdrawEth",
            "outputs": [],
            "stateMutability": "payable",
            "type": "function"
        }
    ]"#;

    /// L2 ERC20 Router ABI for token withdrawals
    pub const L2_ERC20_ROUTER_ABI: &str = r#"[
        {
            "inputs": [
                {"internalType": "address", "name": "token", "type": "address"},
                {"internalType": "address", "name": "to", "type": "address"},
                {"internalType": "uint256", "name": "amount", "type": "uint256"},
                {"internalType": "bytes", "name": "data", "type": "bytes"}
            ],
            "name": "outboundTransfer",
            "outputs": [{"internalType": "bytes", "name": "", "type": "bytes"}],
            "stateMutability": "nonpayable",
            "type": "function"
        }
    ]"#;
}
