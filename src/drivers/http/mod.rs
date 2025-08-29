#[cfg(feature = "http")] pub mod dto;
#[cfg(feature = "http")] pub mod providers;
#[cfg(feature = "http")] pub mod router;
#[cfg(feature = "http")] pub mod validation;

use crate::domain::errors::Result;

/// Simple HTTP server stub (feature-gated)
pub async fn run_server(_host: String, _port: u16) -> Result<()> {
    #[cfg(feature = "http")]
    { router::run(_host, _port).await }
    #[cfg(not(feature = "http"))]
    { Err(crate::domain::errors::ContextError::ConfigError("HTTP feature disabled".to_string())) }
}
