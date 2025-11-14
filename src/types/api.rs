//! Transaction-related types

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Generic API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Whether the request was successful
    pub success: bool,
    /// Response data
    pub result: Option<T>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Error code (if failed)
    pub code: Option<i32>,
}

impl ApiResponse<Value> {
    /// Get the result as a clean string (without JSON quotes for string values)
    pub fn result_string(&self) -> String {
        if let Some(ref result) = self.result {
            match result {
                Value::String(s) => s.clone(),
                _ => result.to_string(),
            }
        } else if let Some(ref error) = self.error {
            format!("Error: {}", error)
        } else {
            "No result".to_string()
        }
    }
}

impl<T> fmt::Display for ApiResponse<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref result) = self.result {
            write!(f, "{}", result)
        } else if let Some(ref error) = self.error {
            write!(f, "Error: {}", error)
        } else {
            write!(f, "No result")
        }
    }
}
