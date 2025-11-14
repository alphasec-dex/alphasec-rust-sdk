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
