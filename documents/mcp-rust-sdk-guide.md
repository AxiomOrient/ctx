# MCP Rust SDK 완전 가이드

> Model Context Protocol (MCP) Rust SDK를 사용한 서버 개발 완전 가이드

## 목차

1. [개요](#개요)
2. [환경 설정](#환경-설정)
3. [기본 개념](#기본-개념)
4. [프로젝트 설정](#프로젝트-설정)
5. [기본 서버 구현](#기본-서버-구현)
6. [고급 기능](#고급-기능)
7. [실전 예제](#실전-예제)
8. [트러블슈팅](#트러블슈팅)
9. [성능 최적화](#성능-최적화)
10. [배포 가이드](#배포-가이드)

## 개요

### MCP (Model Context Protocol)란?

Model Context Protocol은 AI 모델과 외부 도구 간의 표준화된 통신 프로토콜입니다. MCP를 통해 AI 모델은 다양한 외부 리소스와 도구에 안전하고 일관된 방식으로 접근할 수 있습니다.

### Rust SDK의 장점

- **성능**: Rust의 제로 코스트 추상화와 메모리 안전성
- **타입 안전성**: 컴파일 타임에 프로토콜 오류 검출
- **비동기 지원**: Tokio 기반 고성능 비동기 처리
- **매크로 지원**: 보일러플레이트 코드 최소화

## 환경 설정

### 필수 요구사항

```bash
# Rust 설치 (최신 stable 버전 권장)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 버전 확인
rustc --version  # 1.70.0 이상 권장
cargo --version
```

### 개발 도구 설치

```bash
# 유용한 개발 도구들
cargo install cargo-watch    # 파일 변경 감지 및 자동 빌드
cargo install cargo-expand   # 매크로 확장 결과 확인
cargo install cargo-audit    # 보안 취약점 검사
```

## 기본 개념

### 핵심 컴포넌트

1. **Server**: MCP 서버의 메인 구조체
2. **Tool**: 외부에서 호출 가능한 기능
3. **Transport**: 통신 계층 (stdio, HTTP, WebSocket 등)
4. **Handler**: 요청 처리 로직

### 아키텍처 패턴

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   MCP Client    │◄──►│   MCP Server    │◄──►│  Backend API    │
│  (Claude, etc)  │    │  (Your Code)    │    │  (Your Service) │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## 프로젝트 설정

### Cargo.toml 설정

```toml
[package]
name = "my-mcp-server"
version = "0.1.0"
edition = "2021"

[dependencies]
# MCP 공식 SDK
rmcp = { 
    git = "https://github.com/modelcontextprotocol/rust-sdk", 
    branch = "main", 
    features = [
        "server",        # 서버 기능
        "transport-io",  # stdio 전송
        "macros",        # 매크로 지원
        "schemars"       # JSON 스키마 생성
    ] 
}

# 비동기 런타임
tokio = { version = "1.0", features = ["full"] }

# 직렬화/역직렬화
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 에러 처리
anyhow = "1.0"
thiserror = "1.0"

# 로깅
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# CLI (선택사항)
clap = { version = "4.0", features = ["derive"] }
```

### 프로젝트 구조

```
my-mcp-server/
├── Cargo.toml
├── src/
│   ├── main.rs          # 메인 엔트리포인트
│   ├── server.rs        # 서버 구현
│   ├── tools/           # 도구 구현
│   │   ├── mod.rs
│   │   ├── calculator.rs
│   │   └── file_ops.rs
│   └── types.rs         # 타입 정의
├── tests/               # 통합 테스트
└── examples/            # 예제 코드
```

## 기본 서버 구현

### 1. 최소 서버 구현

```rust
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde_json::json;

#[derive(Clone)]
struct MyMcpServer;

#[tool_router]
impl MyMcpServer {
    fn new() -> Self {
        Self
    }

    #[tool(description = "Simple greeting tool")]
    fn greet(&self, name: String) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![
            Content::text(format!("Hello, {}!", name))
        ]))
    }
}

#[tool_handler]
impl ServerHandler for MyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "my-mcp-server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some("A simple MCP server example".to_string()),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 로깅 초기화
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // 서버 생성 및 실행
    let server = MyMcpServer::new();
    let service = server.serve(stdio()).await?;
    
    service.waiting().await?;
    Ok(())
}
```

### 2. 매개변수가 있는 도구

```rust
use rmcp::handler::server::tool::Parameters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct CalculateRequest {
    operation: String,  // "add", "subtract", "multiply", "divide"
    a: f64,
    b: f64,
}

#[tool_router]
impl MyMcpServer {
    #[tool(description = "Perform basic mathematical operations")]
    fn calculate(
        &self,
        Parameters(req): Parameters<CalculateRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = match req.operation.as_str() {
            "add" => req.a + req.b,
            "subtract" => req.a - req.b,
            "multiply" => req.a * req.b,
            "divide" => {
                if req.b == 0.0 {
                    return Err(McpError::invalid_params(
                        "Division by zero", 
                        Some(json!({"divisor": req.b}))
                    ));
                }
                req.a / req.b
            }
            _ => {
                return Err(McpError::invalid_params(
                    "Invalid operation", 
                    Some(json!({"operation": req.operation}))
                ));
            }
        };

        Ok(CallToolResult::success(vec![
            Content::text(format!("{} {} {} = {}", req.a, req.operation, req.b, result))
        ]))
    }
}
```

### 3. 비동기 도구

```rust
use std::time::Duration;
use tokio::time::sleep;

#[tool_router]
impl MyMcpServer {
    #[tool(description = "Simulate a long-running operation")]
    async fn long_operation(&self, duration_secs: u64) -> Result<CallToolResult, McpError> {
        if duration_secs > 60 {
            return Err(McpError::invalid_params(
                "Duration too long (max 60 seconds)", 
                None
            ));
        }

        sleep(Duration::from_secs(duration_secs)).await;
        
        Ok(CallToolResult::success(vec![
            Content::text(format!("Operation completed after {} seconds", duration_secs))
        ]))
    }
}
```

## 고급 기능

### 1. 리소스 제공

```rust
#[tool_handler]
impl ServerHandler for MyMcpServer {
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![
                Resource {
                    uri: "file:///config.json".into(),
                    name: "Configuration".into(),
                    description: Some("Server configuration file".into()),
                    mime_type: Some("application/json".into()),
                    annotations: None,
                }
            ],
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match uri.as_str() {
            "file:///config.json" => {
                let config = json!({
                    "server_name": "my-mcp-server",
                    "version": "1.0.0",
                    "features": ["tools", "resources"]
                });
                
                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::text(
                        serde_json::to_string_pretty(&config).unwrap(),
                        uri
                    )],
                })
            }
            _ => Err(McpError::resource_not_found("Resource not found", None))
        }
    }
}
```

### 2. 프롬프트 제공

```rust
#[tool_handler]
impl ServerHandler for MyMcpServer {
    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            prompts: vec![
                Prompt::new(
                    "code_review",
                    Some("Review code for best practices and potential issues"),
                    Some(vec![
                        PromptArgument {
                            name: "code".to_string(),
                            description: Some("Code to review".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "language".to_string(),
                            description: Some("Programming language".to_string()),
                            required: Some(false),
                        }
                    ])
                )
            ],
            next_cursor: None,
        })
    }

    async fn get_prompt(
        &self,
        GetPromptRequestParam { name, arguments }: GetPromptRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        match name.as_str() {
            "code_review" => {
                let code = arguments
                    .as_ref()
                    .and_then(|args| args.get("code"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing code parameter", None))?;

                let language = arguments
                    .as_ref()
                    .and_then(|args| args.get("language"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                let prompt = format!(
                    "Please review the following {} code for best practices, \
                     potential bugs, and improvements:\n\n```{}\n{}\n```",
                    language, language, code
                );

                Ok(GetPromptResult {
                    description: Some("Code review prompt".into()),
                    messages: vec![PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(prompt),
                    }],
                })
            }
            _ => Err(McpError::invalid_params("Unknown prompt", None))
        }
    }
}
```

### 3. 상태 관리

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Clone)]
struct StatefulServer {
    state: Arc<RwLock<HashMap<String, String>>>,
}

#[tool_router]
impl StatefulServer {
    fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[tool(description = "Store a key-value pair")]
    async fn store(&self, key: String, value: String) -> Result<CallToolResult, McpError> {
        let mut state = self.state.write().await;
        state.insert(key.clone(), value.clone());
        
        Ok(CallToolResult::success(vec![
            Content::text(format!("Stored: {} = {}", key, value))
        ]))
    }

    #[tool(description = "Retrieve a value by key")]
    async fn retrieve(&self, key: String) -> Result<CallToolResult, McpError> {
        let state = self.state.read().await;
        
        match state.get(&key) {
            Some(value) => Ok(CallToolResult::success(vec![
                Content::text(format!("{} = {}", key, value))
            ])),
            None => Err(McpError::invalid_params(
                "Key not found", 
                Some(json!({"key": key}))
            ))
        }
    }
}
```

## 실전 예제

### 외부 API 통합 서버

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct ApiIntegrationServer {
    http_client: Client,
    api_base_url: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct WeatherRequest {
    city: String,
    country_code: Option<String>,
}

#[tool_router]
impl ApiIntegrationServer {
    fn new(api_base_url: String) -> Self {
        Self {
            http_client: Client::new(),
            api_base_url,
        }
    }

    #[tool(description = "Get current weather for a city")]
    async fn get_weather(
        &self,
        Parameters(req): Parameters<WeatherRequest>,
    ) -> Result<CallToolResult, McpError> {
        let url = format!(
            "{}/weather?q={}&appid=YOUR_API_KEY",
            self.api_base_url,
            req.city
        );

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| McpError::internal_error(
                "Failed to fetch weather data", 
                Some(json!({"error": e.to_string()}))
            ))?;

        if !response.status().is_success() {
            return Err(McpError::internal_error(
                "Weather API returned error",
                Some(json!({"status": response.status().as_u16()}))
            ));
        }

        let weather_data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| McpError::internal_error(
                "Failed to parse weather data",
                Some(json!({"error": e.to_string()}))
            ))?;

        Ok(CallToolResult::success(vec![
            Content::text(format!("Weather data: {}", 
                serde_json::to_string_pretty(&weather_data).unwrap()
            ))
        ]))
    }
}

#[tool_handler]
impl ServerHandler for ApiIntegrationServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "weather-mcp-server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some("Weather information MCP server".to_string()),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        // 초기화 시 API 연결 테스트
        tracing::info!("Weather MCP server initialized");
        Ok(self.get_info())
    }
}
```

### CLI 인터페이스 추가

```rust
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// API base URL
    #[arg(long, default_value = "https://api.openweathermap.org/data/2.5")]
    api_url: String,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // 로깅 설정
    let log_level = if args.verbose { "debug" } else { &args.log_level };
    tracing_subscriber::fmt()
        .with_env_filter(format!("my_server={}", log_level))
        .init();

    // 서버 생성 및 실행
    let server = ApiIntegrationServer::new(args.api_url);
    let service = server.serve(stdio()).await?;
    
    tracing::info!("MCP server started");
    service.waiting().await?;
    
    Ok(())
}
```

## 트러블슈팅

### 일반적인 문제들

#### 1. 매크로 관련 오류

**문제**: `#[tool]` 매크로가 인식되지 않음
```
error: cannot find attribute `tool` in this scope
```

**해결책**: 
```rust
// 필요한 매크로 import 확인
use rmcp::{tool, tool_handler, tool_router};

// Cargo.toml에 macros 기능 활성화 확인
rmcp = { ..., features = ["macros"] }
```

#### 2. 타입 변환 오류

**문제**: Tool 구조체 필드 타입 불일치
```
error: expected `Cow<'_, str>`, found `String`
```

**해결책**:
```rust
// String을 Cow로 변환
Tool {
    name: "my_tool".to_string().into(),  // 또는 "my_tool".into()
    description: Some("description".into()),
    // ...
}
```

#### 3. 비동기 함수 오류

**문제**: 비동기 도구에서 Future 트레이트 오류

**해결책**:
```rust
use std::future::Future;  // Future 트레이트 import 추가
```

### 디버깅 팁

#### 1. 매크로 확장 확인

```bash
# 매크로가 어떻게 확장되는지 확인
cargo expand --bin my-server
```

#### 2. 로깅 활성화

```rust
// 상세한 로깅 설정
tracing_subscriber::fmt()
    .with_env_filter("debug,rmcp=trace")
    .with_target(true)
    .with_line_number(true)
    .init();
```

#### 3. 테스트 작성

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::transport::stdio;

    #[tokio::test]
    async fn test_server_creation() {
        let server = MyMcpServer::new();
        // 서버 생성 테스트
        assert!(true); // 실제 테스트 로직 추가
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let server = MyMcpServer::new();
        let result = server.greet("World".to_string());
        assert!(result.is_ok());
    }
}
```

## 성능 최적화

### 1. 연결 풀링

```rust
use std::sync::Arc;

#[derive(Clone)]
struct OptimizedServer {
    http_client: Arc<reqwest::Client>,  // 클라이언트 재사용
}

impl OptimizedServer {
    fn new() -> Self {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http_client: Arc::new(client),
        }
    }
}
```

### 2. 캐싱

```rust
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone)]
struct CacheEntry<T> {
    data: T,
    expires_at: Instant,
}

#[derive(Clone)]
struct CachedServer {
    cache: Arc<RwLock<HashMap<String, CacheEntry<String>>>>,
    cache_ttl: Duration,
}

impl CachedServer {
    async fn get_cached_or_fetch(&self, key: &str) -> Result<String, McpError> {
        // 캐시 확인
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(key) {
                if entry.expires_at > Instant::now() {
                    return Ok(entry.data.clone());
                }
            }
        }

        // 캐시 미스 - 새 데이터 가져오기
        let data = self.fetch_data(key).await?;
        
        // 캐시 업데이트
        {
            let mut cache = self.cache.write().await;
            cache.insert(key.to_string(), CacheEntry {
                data: data.clone(),
                expires_at: Instant::now() + self.cache_ttl,
            });
        }

        Ok(data)
    }

    async fn fetch_data(&self, key: &str) -> Result<String, McpError> {
        // 실제 데이터 가져오기 로직
        Ok(format!("Data for {}", key))
    }
}
```

### 3. 배치 처리

```rust
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

#[derive(Clone)]
struct BatchProcessor {
    sender: mpsc::UnboundedSender<String>,
}

impl BatchProcessor {
    fn new() -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        
        // 배치 처리 태스크 시작
        tokio::spawn(async move {
            let mut batch = Vec::new();
            let mut interval = interval(Duration::from_secs(5));
            
            loop {
                tokio::select! {
                    item = receiver.recv() => {
                        if let Some(item) = item {
                            batch.push(item);
                            if batch.len() >= 100 {
                                Self::process_batch(&batch).await;
                                batch.clear();
                            }
                        }
                    }
                    _ = interval.tick() => {
                        if !batch.is_empty() {
                            Self::process_batch(&batch).await;
                            batch.clear();
                        }
                    }
                }
            }
        });

        Self { sender }
    }

    async fn process_batch(items: &[String]) {
        tracing::info!("Processing batch of {} items", items.len());
        // 배치 처리 로직
    }

    fn add_item(&self, item: String) {
        let _ = self.sender.send(item);
    }
}
```

## 배포 가이드

### 1. Docker 배포

```dockerfile
# Dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/my-mcp-server /usr/local/bin/
EXPOSE 8080
CMD ["my-mcp-server"]
```

### 2. 시스템 서비스

```ini
# /etc/systemd/system/mcp-server.service
[Unit]
Description=MCP Server
After=network.target

[Service]
Type=simple
User=mcp
WorkingDirectory=/opt/mcp-server
ExecStart=/opt/mcp-server/my-mcp-server --log-level info
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### 3. 환경 설정

```bash
# .env 파일
MCP_LOG_LEVEL=info
MCP_API_URL=https://api.example.com
MCP_TIMEOUT=30
```

```rust
// 환경 변수 로딩
use std::env;

fn load_config() -> Config {
    Config {
        log_level: env::var("MCP_LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
        api_url: env::var("MCP_API_URL").expect("MCP_API_URL must be set"),
        timeout: env::var("MCP_TIMEOUT")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .expect("Invalid timeout value"),
    }
}
```

## 모범 사례

### 1. 에러 처리

```rust
// 커스텀 에러 타입 정의
#[derive(thiserror::Error, Debug)]
enum ServerError {
    #[error("Database connection failed: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },
}

impl From<ServerError> for McpError {
    fn from(err: ServerError) -> Self {
        match err {
            ServerError::InvalidInput { message } => {
                McpError::invalid_params(&message, None)
            }
            _ => McpError::internal_error(&err.to_string(), None)
        }
    }
}
```

### 2. 구조화된 로깅

```rust
use tracing::{info, warn, error, instrument};

#[tool_router]
impl MyServer {
    #[instrument(skip(self), fields(tool_name = "calculate"))]
    #[tool(description = "Calculate something")]
    async fn calculate(&self, input: f64) -> Result<CallToolResult, McpError> {
        info!(input = %input, "Starting calculation");
        
        let result = input * 2.0;
        
        info!(result = %result, "Calculation completed");
        Ok(CallToolResult::success(vec![
            Content::text(result.to_string())
        ]))
    }
}
```

### 3. 설정 관리

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct ServerConfig {
    pub server: ServerSettings,
    pub database: DatabaseSettings,
    pub api: ApiSettings,
}

#[derive(Debug, Deserialize, Serialize)]
struct ServerSettings {
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

impl ServerConfig {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
```

## 참고 자료

### 공식 문서
- [MCP 사양](https://spec.modelcontextprotocol.io/)
- [Rust SDK GitHub](https://github.com/modelcontextprotocol/rust-sdk)
- [MCP 공식 사이트](https://modelcontextprotocol.io/)

### 유용한 크레이트
- `tokio`: 비동기 런타임
- `serde`: 직렬화/역직렬화
- `tracing`: 구조화된 로깅
- `anyhow`: 에러 처리
- `clap`: CLI 인터페이스
- `reqwest`: HTTP 클라이언트

### 커뮤니티
- [MCP Discord](https://discord.gg/modelcontextprotocol)
- [GitHub Discussions](https://github.com/modelcontextprotocol/rust-sdk/discussions)

---

이 가이드는 MCP Rust SDK를 사용한 실제 프로젝트 경험을 바탕으로 작성되었습니다. 추가 질문이나 개선 사항이 있다면 언제든 문의해 주세요.