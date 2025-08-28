pub mod dto;
pub mod providers;
pub mod router;
pub mod validation;

use crate::domain::{errors::Result, types::*};

/// Simple HTTP server placeholder
pub async fn run_server(_host: String, _port: u16) -> Result<()> {
    // HTTP server requires axum and other dependencies
    // For now, return error indicating this needs implementation
    Err(crate::domain::errors::ContextError::ConfigError(
        "HTTP server requires full implementation - use CLI or MCP modes instead".to_string()
    ))
}