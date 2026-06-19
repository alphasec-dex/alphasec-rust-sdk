//! Perpetual futures (perp) support — v1.0
pub mod agent;
pub mod client;
pub mod types;
pub mod ws;

pub use agent::PerpAgent;
pub use types::*;
