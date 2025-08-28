//! Server command implementation


use crate::drivers::http::router::{AppState, create_router};
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
        .with_env_filter("ctxset=debug,tower_http=debug")
        .init();

    info!("Starting CTXSET server...");

    // 앱 상태 로드
    let state = AppState::load()
        .await
        .map_err(|e| crate::domain::errors::ContextError::Other(format!("Failed to load app state: {}", e)))?;

    // 라우터 생성
    let app = create_router(state);

    // 서버 주소 설정
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .map_err(|e| crate::domain::errors::ContextError::Other(format!("Invalid address: {}", e)))?;

    info!("Server listening on http://{}", addr);
    info!("API endpoints:");
    info!("  GET  /healthz");
    info!("  POST /api/classify");
    info!("  POST /api/validate");
    info!("  POST /api/compose");
    info!("  GET  /api/rules");
    info!("  PUT  /api/rules");
    info!("  GET  /api/ontology");
    info!("  PUT  /api/ontology");

    // 서버 시작
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(crate::domain::errors::ContextError::Io)?;

    axum::serve(listener, app)
        .await
        .map_err(|e| crate::domain::errors::ContextError::Other(format!("Server error: {}", e)))?;

    Ok(())
}
