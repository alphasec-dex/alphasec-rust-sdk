//! Account-related types for AlphaSec API

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Account balance information from /api/v1/wallet/balance
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    /// Token ID
    pub token_id: String,
    /// Locked balance (as string, optional)
    #[serde(default)]
    pub locked: Option<String>,
    /// Unlocked balance (as string, optional)
    #[serde(default)]
    pub unlocked: Option<String>,
}
/// Account balances information from /api/v1/wallet/balance
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balances {
    /// Balances
    pub balances: Vec<Balance>,
    /// Block number
    pub block_number: u64,
}

impl Balance {
    /// Get available balance as Decimal (converted from wei)
    pub fn available_decimal(&self, decimals: u32) -> Option<Result<Decimal, rust_decimal::Error>> {
        self.unlocked.as_ref().map(|unlocked| {
            let available = unlocked.parse::<Decimal>()?;
            Ok(available / Decimal::from(10u64.pow(decimals)))
        })
    }

    /// Get locked balance as Decimal (converted from wei)
    pub fn locked_decimal(&self, decimals: u32) -> Option<Result<Decimal, rust_decimal::Error>> {
        self.locked.as_ref().map(|locked| {
            let locked = locked.parse::<Decimal>()?;
            Ok(locked / Decimal::from(10u64.pow(decimals)))
        })
    }

    /// Get total balance as Decimal (converted from wei)
    pub fn total_decimal(&self, decimals: u32) -> Option<Result<Decimal, rust_decimal::Error>> {
        self.locked.as_ref().map(|locked| {
            let total = locked.parse::<Decimal>()?;
            Ok(total / Decimal::from(10u64.pow(decimals)))
        })
    }
}

/// Session information from /api/v1/wallet/session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Session name
    pub name: String,
    /// Session wallet address
    pub session_address: String,
    /// L1 owner address
    pub owner_address: String,
    /// Session expiry timestamp (milliseconds since Unix epoch)
    pub expiry: u64,
    /// Whether session is applied
    pub applied: bool,
}

impl Session {
    /// Convert expiry timestamp to DateTime<Utc>
    pub fn expiry_datetime(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        use chrono::{TimeZone, Utc};
        Utc.timestamp_millis_opt(self.expiry as i64).single()
    }
}

/// Transfer record from /api/v1/wallet/transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    /// Transfer ID
    pub id: i64,
    /// Sender address
    pub from_address: String,
    /// Recipient address
    pub to_address: String,
    /// Transaction type (e.g., "Token Transfer", "Value Transfer")
    pub tx_type: String,
    /// Token ID
    pub token_id: String,
    /// Transfer amount (as string for precision)
    pub amount: String,
    /// Transfer status (e.g., "Success", "Pending")
    pub status: String,
    /// Timestamp in milliseconds
    pub timestamp: i64,
    /// Transaction hash
    pub hash: String,
}

impl Transfer {
    /// Convert timestamp to DateTime<Utc>
    pub fn timestamp_datetime(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        use chrono::{TimeZone, Utc};
        Utc.timestamp_millis_opt(self.timestamp).single()
    }

    /// Get amount as Decimal (converted from wei)
    pub fn amount_decimal(&self, decimals: u32) -> Result<Decimal, rust_decimal::Error> {
        let amount = self.amount.parse::<Decimal>()?;
        Ok(amount / Decimal::from(10u64.pow(decimals)))
    }
}

/// Query parameters for transfer history
#[derive(Debug, Clone, Default)]
pub struct TransferHistoryQuery {
    /// Wallet address to query (required)
    pub address: String,
    /// Filter by specific token ID (optional)
    pub token_id: Option<i64>,
    /// Start timestamp in milliseconds (optional)
    pub from_msec: Option<i64>,
    /// End timestamp in milliseconds (optional)
    pub to_msec: Option<i64>,
    /// Maximum records to return (default: 100, max: 500)
    pub limit: Option<u32>,
}
