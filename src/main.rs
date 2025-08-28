//! CtxSet - AI Context Management Library
//! 
//! Single binary with multiple modes: CLI/HTTP/MCP
//! Follows PLAN.md architecture specification

// Re-export existing modules to preserve all functionality
pub mod app;
pub mod pipeline;
pub mod domain;
pub mod drivers; 
pub mod services;
pub mod util;
pub mod doc;
pub mod knowledge;

use app::run_app;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    
    // Run the application with async support
    run_app().await.map_err(|e| anyhow::anyhow!("{}", e.user_friendly_message()))?;
    
    Ok(())
}