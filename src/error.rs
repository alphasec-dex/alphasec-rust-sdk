//! Error types for the AlphaSec SDK

use thiserror::Error;

/// Result type alias for AlphaSec operations
pub type Result<T> = std::result::Result<T, AlphaSecError>;

/// Main error type for AlphaSec SDK operations
#[derive(Error, Debug)]
pub enum AlphaSecError {
    /// HTTP request errors
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// WebSocket connection errors
    #[cfg(feature = "websocket")]
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    /// Ethereum wallet/signing errors
    #[error("Ethereum error: {0}")]
    Ethereum(#[from] ethers::core::types::SignatureError),

    /// EIP-712 signing errors
    #[error("EIP-712 signing error: {0}")]
    Eip712(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// API errors returned from AlphaSec
    #[error("API error {code}: {message}")]
    Api {
        /// Error code from the API
        code: i32,
        /// Error message from the API
        message: String,
    },

    /// Authentication/Session errors
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// Invalid parameter errors
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Network connectivity errors
    #[error("Network error: {0}")]
    Network(String),

    /// Address validation errors
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    /// Token/Market not found errors
    #[error("Not found: {0}")]
    NotFound(String),

    /// Signing/Wallet errors
    #[error("Signer error: {0}")]
    Signer(String),

    /// Transaction encoding errors
    #[error("Transaction encoding error: {0}")]
    TransactionEncoding(String),

    /// Nonce generation errors
    #[error("Nonce generation error: {0}")]
    Nonce(String),

    /// Generic errors
    #[error("AlphaSec error: {0}")]
    Generic(String),
}

impl AlphaSecError {
    /// Create a new API error
    pub fn api(code: i32, message: impl Into<String>) -> Self {
        Self::Api {
            code,
            message: message.into(),
        }
    }

    /// Create a new configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    /// Create a new authentication error
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth(message.into())
    }

    /// Create a new invalid parameter error
    pub fn invalid_parameter(message: impl Into<String>) -> Self {
        Self::InvalidParameter(message.into())
    }

    /// Create a new network error
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network(message.into())
    }

    /// Create a new invalid address error
    pub fn invalid_address(message: impl Into<String>) -> Self {
        Self::InvalidAddress(message.into())
    }

    /// Create a new not found error
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    /// Create a new signer error
    pub fn signer(message: impl Into<String>) -> Self {
        Self::Signer(message.into())
    }

    /// Create a new transaction encoding error
    pub fn transaction_encoding(message: impl Into<String>) -> Self {
        Self::TransactionEncoding(message.into())
    }

    /// Create a new nonce generation error
    pub fn nonce(message: impl Into<String>) -> Self {
        Self::Nonce(message.into())
    }

    /// Create a new EIP-712 signing error
    pub fn eip712(message: impl Into<String>) -> Self {
        Self::Eip712(message.into())
    }

    /// Create a new generic error
    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_display_interpolates_code_verbatim_with_colon_separator() {
        assert_eq!(
            AlphaSecError::api(400, "bad request").to_string(),
            "API error 400: bad request"
        );
        // Negative code must not be absolute-valued or zero-coerced.
        assert_eq!(AlphaSecError::api(-1, "x").to_string(), "API error -1: x");
        // Code 0 must still be printed, not treated as "no code".
        assert_eq!(
            AlphaSecError::api(0, "zero").to_string(),
            "API error 0: zero"
        );
    }

    #[test]
    fn api_display_preserves_message_verbatim_including_empty_and_colons() {
        // Empty message: separator retained, no panic.
        assert_eq!(AlphaSecError::api(500, "").to_string(), "API error 500: ");
        // Colons inside the message must survive untouched.
        assert_eq!(
            AlphaSecError::api(502, "a: b: c").to_string(),
            "API error 502: a: b: c"
        );
    }
}
