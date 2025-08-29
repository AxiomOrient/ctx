//! ctx — Single binary entry (CLI/HTTP/MCP)
use ctx::app::run_app;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    // Tracing subscriber (simple fmt)
    let _ = tracing_subscriber::fmt::try_init();
    
    // Run the application with async support
    run_app().await.map_err(|e| anyhow::anyhow!("{}", e.user_friendly_message()))?;
    
    Ok(())
}
