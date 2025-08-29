//! Server command implementation

use crate::drivers::http::router::create_router;
use std::net::SocketAddr;
use tracing::info;

#[derive(Debug)]
pub struct ServerArgs {
    pub port: u16,
    pub host: String,
}

pub fn run_server(args: ServerArgs) -> crate::domain::errors::Result<()> {
    // Tokio 런타임 생성
    let rt = tokio::runtime::Runtime::new().map_err(crate::domain::errors::ContextError::Io)?;

    rt.block_on(async { run_server_async(args).await })
}

async fn run_server_async(args: ServerArgs) -> crate::domain::errors::Result<()> {
    // 트레이싱 초기화
    tracing_subscriber::fmt()
        .with_env_filter("ctx=debug,tower_http=debug")
        .init();

    info!("Starting ctx server...");

    // 라우터 생성 (minimal health)
    let app = create_router();

    // 서버 주소 설정
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .map_err(|e| {
            crate::domain::errors::ContextError::Other(format!("Invalid address: {}", e))
        })?;

    info!("Server listening on http://{}", addr);
    info!("API endpoints:");
    info!("  GET  /health");

    // 서버 시작
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(crate::domain::errors::ContextError::Io)?;

    axum::serve(listener, app)
        .await
        .map_err(|e| crate::domain::errors::ContextError::Other(format!("Server error: {}", e)))?;

    Ok(())
}
