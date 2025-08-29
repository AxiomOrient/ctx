---
id: backend-rust-axum
category: stack
priority: high
resolution_levels: [overview, standard, detailed, expert]
tags: [rust, backend, axum, web-server, api]
dependencies: [rust-language, core-principles]
---

# Rust Backend Stack with Axum

## 🚀 스택 개요

Axum은 Rust로 작성된 현대적이고 성능이 뛰어난 웹 프레임워크입니다.
Tokio 생태계를 기반으로 하며, 타입 안전성과 성능을 모두 제공합니다.

### 핵심 구성 요소
- **Axum**: 웹 프레임워크
- **Tokio**: 비동기 런타임
- **SQLx**: 데이터베이스 ORM
- **Serde**: 직렬화/역직렬화
- **Tower**: 미들웨어 시스템

## 📦 의존성 설정

```toml
[dependencies]
# 웹 프레임워크
axum = { version = "0.7", features = ["macros"] }
tokio = { version = "1.0", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

# 데이터베이스
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid"] }

# 직렬화
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 에러 처리
thiserror = "1.0"
anyhow = "1.0"

# 로깅
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# 인증
jsonwebtoken = "9.0"
bcrypt = "0.15"

# 검증
validator = { version = "0.18", features = ["derive"] }

# 환경 변수
dotenvy = "0.15"

# 시간 처리
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }

[dev-dependencies]
tokio-test = "0.4"
```

## 🔧 기본 애플리케이션 설정

### main.rs
```rust
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod handlers;
mod middleware;
mod models;
mod repositories;
mod services;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 로깅 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend_rust_axum=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 환경 변수 로드
    dotenvy::dotenv().ok();

    // 데이터베이스 연결
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let pool = sqlx::PgPool::connect(&database_url).await?;

    // 애플리케이션 상태
    let app_state = AppState { pool };

    // 라우터 구성
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .nest("/api/v1", api_routes())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // 서버 시작
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> &'static str {
    "Backend Rust Axum API"
}

async fn health_check() -> &'static str {
    "OK"
}

fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(handlers::users::list_users).post(handlers::users::create_user))
        .route("/users/:id", get(handlers::users::get_user).put(handlers::users::update_user))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/register", post(handlers::auth::register))
}

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
}
```

## 🎯 핸들러 구현

### handlers/users.rs
```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{
    models::user::{User, CreateUserRequest, UpdateUserRequest},
    services::user_service::UserService,
    utils::validation::ValidatedJson,
    AppState,
};

#[derive(Deserialize)]
pub struct ListUsersQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct ListUsersResponse {
    pub users: Vec<User>,
    pub total: u64,
    pub page: u32,
    pub limit: u32,
}

pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<ListUsersQuery>,
) -> Result<Json<ListUsersResponse>, (StatusCode, String)> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);

    let user_service = UserService::new(&state.pool);

    match user_service.list_users(page, limit).await {
        Ok((users, total)) => Ok(Json(ListUsersResponse {
            users,
            total,
            page,
            limit,
        })),
        Err(e) => {
            tracing::error!("Failed to list users: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to list users".to_string()))
        }
    }
}

pub async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, (StatusCode, String)> {
    let user_service = UserService::new(&state.pool);

    match user_service.get_user(id).await {
        Ok(Some(user)) => Ok(Json(user)),
        Ok(None) => Err((StatusCode::NOT_FOUND, "User not found".to_string())),
        Err(e) => {
            tracing::error!("Failed to get user {}: {}", id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to get user".to_string()))
        }
    }
}

pub async fn create_user(
    State(state): State<AppState>,
    ValidatedJson(request): ValidatedJson<CreateUserRequest>,
) -> Result<(StatusCode, Json<User>), (StatusCode, String)> {
    let user_service = UserService::new(&state.pool);

    match user_service.create_user(request).await {
        Ok(user) => Ok((StatusCode::CREATED, Json(user))),
        Err(e) => {
            tracing::error!("Failed to create user: {}", e);
            Err((StatusCode::BAD_REQUEST, format!("Failed to create user: {}", e)))
        }
    }
}

pub async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ValidatedJson(request): ValidatedJson<UpdateUserRequest>,
) -> Result<Json<User>, (StatusCode, String)> {
    let user_service = UserService::new(&state.pool);

    match user_service.update_user(id, request).await {
        Ok(Some(user)) => Ok(Json(user)),
        Ok(None) => Err((StatusCode::NOT_FOUND, "User not found".to_string())),
        Err(e) => {
            tracing::error!("Failed to update user {}: {}", id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to update user".to_string()))
        }
    }
}
```

## 🗄️ 데이터 모델

### models/user.rs
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[validate(length(min = 1, max = 100, message = "Name must be between 1 and 100 characters"))]
    pub name: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,

    #[validate(length(min = 1, max = 100, message = "Name must be between 1 and 100 characters"))]
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            name: user.name,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}
```

## 🔐 인증 미들웨어

### middleware/auth.rs
```rust
use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: usize,
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let jwt_secret = std::env::var("JWT_SECRET")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_ref()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?
    .claims;

    // 요청에 사용자 ID 추가
    request.extensions_mut().insert(claims.sub);

    Ok(next.run(request).await)
}
```

## 🧪 테스트 전략

### tests/integration_tests.rs
```rust
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

use backend_rust_axum::{create_app, AppState};

#[tokio::test]
async fn test_health_check() {
    let pool = create_test_pool().await;
    let app_state = AppState { pool };
    let app = create_app(app_state);

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_create_user() {
    let pool = create_test_pool().await;
    let app_state = AppState { pool };
    let app = create_app(app_state);

    let user_data = json!({
        "email": "test@example.com",
        "name": "Test User",
        "password": "password123"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/users")
                .header("content-type", "application/json")
                .body(Body::from(user_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn create_test_pool() -> sqlx::PgPool {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/test_db".to_string());

    sqlx::PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database")
}
```

## 🚀 배포 및 운영

### Dockerfile
```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/backend-rust-axum /usr/local/bin/backend-rust-axum

EXPOSE 3000

CMD ["backend-rust-axum"]
```

### docker-compose.yml
```yaml
version: '3.8'

services:
  app:
    build: .
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=postgres://user:password@db:5432/myapp
      - JWT_SECRET=your-secret-key
    depends_on:
      - db

  db:
    image: postgres:15
    environment:
      - POSTGRES_USER=user
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=myapp
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"

volumes:
  postgres_data:
```

이 스택 가이드는 Rust와 Axum을 사용한 백엔드 개발의 모범 사례를 제시합니다.
프로젝트의 특성에 따라 추가적인 미들웨어나 서비스를 구성할 수 있습니다.
