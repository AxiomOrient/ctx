use axum::{routing::get, Json, Router};
use serde_json::json;
use crate::domain::errors::{Result as CtxResult, ContextError};

/// Minimal HTTP router for health only (UI-Lite is planned in P1)
pub fn create_router() -> Router {
    async fn health() -> Json<serde_json::Value> {
        Json(json!({"status":"ok","service":"ctx"}))
    }
    Router::new().route("/health", get(health))
}

/// Run HTTP server (host:port)
pub async fn run(host: String, port: u16) -> CtxResult<()> {
    let app = create_router();
    let addr: std::net::SocketAddr = format!("{}:{}", host, port).parse().map_err(|e| ContextError::ConfigError(format!("bad addr: {}", e)))?;
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| ContextError::ServerError(e.to_string()))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| ContextError::ServerError(e.to_string()))
}
