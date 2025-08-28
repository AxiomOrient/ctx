use super::dto::*;
use super::validation::ApiValidator;
use crate::pipeline::classifier::service::ClassifierService;
use crate::drivers::storage::sqlite::SqliteIndexManager;
use crate::domain::types::BuildQuery;
use crate::drivers::storage::local::LocalFsStorage;
use crate::knowledge::ontology::schema::OntologyRegistry;
use crate::knowledge::rules::loader as rules_loader;
use crate::knowledge::rules::loader::builtin_rules;
use crate::domain::errors::Result as CtxResult;
use std::{path::PathBuf, sync::Arc};
use askama::Template;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use axum_extra::extract::Form;
use sqlx::{SqlitePool, Row};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;
use tokio::sync::RwLock;
use tower_http::{cors::{CorsLayer, Any}, trace::TraceLayer, limit::RequestBodyLimitLayer};
use std::num::NonZeroUsize;

// Askama 커스텀 필터 모듈
mod filters {
    pub fn round(value: &f32, _values: &dyn askama::Values) -> ::askama::Result<i32> {
        Ok(value.round() as i32)
    }
    
    pub fn length<T>(slice: &[T], _values: &dyn askama::Values) -> ::askama::Result<usize> {
        Ok(slice.len())
    }
}

/// AppState - HTTP 서버 상태
#[derive(Clone)]
pub struct AppState {
    pub ontology_path: PathBuf,
    pub rules_path: PathBuf,
    pub ontology_yaml: Arc<RwLock<String>>,
    pub rules_yaml: Arc<RwLock<String>>,
    pub storage: Arc<LocalFsStorage>,
    pub db_path: PathBuf,
    pub db_pool: SqlitePool,
}

impl AppState {
    /// 기본 경로로 상태 로드
    pub async fn load() -> CtxResult<Self> {
        Self::load_with_paths("ontology.yaml", "rules.yaml").await
    }

    /// 커스텀 경로로 상태 로드
    pub async fn load_with_paths(ontology_path: &str, rules_path: &str) -> CtxResult<Self> {
        let ontology_path = PathBuf::from(ontology_path);
        let rules_path = PathBuf::from(rules_path);
        let ontology_yaml = tokio::fs::read_to_string(&ontology_path)
            .await
            .map_err(crate::domain::errors::ContextError::Io)?;
        let rules_yaml = tokio::fs::read_to_string(&rules_path)
            .await
            .map_err(crate::domain::errors::ContextError::Io)?;

        let storage = Arc::new(LocalFsStorage::new("."));
        let db_path = PathBuf::from("ctxindex.db");

        // Build SQLite connection pool (sqlx)
        let db_url = format!("sqlite:{}", db_path.display());
        let db_pool = SqlitePool::connect(&db_url)
            .await
            .map_err(|e| crate::domain::errors::ContextError::Other(format!("DB pool build error: {e}")))?;

        Ok(Self {
            ontology_path,
            rules_path,
            ontology_yaml: Arc::new(RwLock::new(ontology_yaml)),
            rules_yaml: Arc::new(RwLock::new(rules_yaml)),
            storage,
            db_path,
            db_pool,
        })
    }
}

/// 라우터 생성
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/api/classify", post(api_classify))
        .route("/api/compose", post(api_compose))
        .route("/health", get(health_check))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(axum::http::Method::GET)
                .allow_methods(axum::http::Method::POST)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(1024 * 1024 * 10)) // 10MB limit
        .with_state(state)
}

/// 대시보드 페이지 
#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    title: String,
    stats: DashboardStats,
    recent_operations: Vec<RecentOperation>,
}

#[derive(Debug)]
struct DashboardStats {
    total_classifications: u32,
    total_compositions: u32,
    avg_confidence: f32,
    active_namespaces: u32,
}

#[derive(Debug)]
struct RecentOperation {
    id: String,
    operation_type: String,
    timestamp: String,
    title: String,
    confidence: Option<f32>,
}

async fn dashboard() -> impl IntoResponse {
    let template = DashboardTemplate {
        title: "CtxSet Dashboard".to_string(),
        stats: DashboardStats {
            total_classifications: 42,
            total_compositions: 24,
            avg_confidence: 85.5,
            active_namespaces: 8,
        },
        recent_operations: vec![
            RecentOperation {
                id: "1".to_string(),
                operation_type: "classify".to_string(),
                timestamp: "2024-08-28T15:30:00Z".to_string(),
                title: "Sample Document".to_string(),
                confidence: Some(85.5),
            }
        ],
    };
    Html(template.render().unwrap_or_else(|_| "Template Error".to_string()))
}

/// 헬스 체크
async fn health_check() -> impl IntoResponse {
    Json(json!({"status": "ok", "service": "ctxset"}))
}

/// API: 텍스트 분류
async fn api_classify(
    State(state): State<AppState>,
    Json(req): Json<ClassifyRequest>,
) -> Result<Json<ClassifyResponse>, ApiError> {
    // 입력 검증
    ApiValidator::validate_classify(&req)?;

    // 온톨로지와 룰 로드
    let ontology_yaml = state.ontology_yaml.read().await.clone();
    let rules_yaml = state.rules_yaml.read().await.clone();

    let ontology = OntologyRegistry::from_yaml(&ontology_yaml)
        .map_err(|e| ApiError::internal_error(format!("Failed to load ontology: {}", e)))?;

    let rules = rules_loader::load_yaml_rules_from_string(&rules_yaml)
        .map_err(|e| ApiError::internal_error(format!("Failed to load rules: {}", e)))?;

    // 분류 서비스 생성 및 실행
    let mut classifier = ClassifierService::new();
    classifier.set_ontology(ontology);
    classifier.set_rules(rules);

    let text = format!("{} {}", req.title, req.body);
    let facets = classifier.classify_text(&text)
        .map_err(|e| ApiError::internal_error(format!("Classification failed: {}", e)))?;

    Ok(Json(ClassifyResponse {
        facets,
        confidence: 0.8, // 기본값
    }))
}

/// API: 프롬프트 구성
async fn api_compose(
    State(_state): State<AppState>,
    Json(req): Json<ComposeRequest>,
) -> Result<Json<ComposeResponse>, ApiError> {
    // 입력 검증
    ApiValidator::validate_compose(&req)?;

    // 단순한 응답 (나중에 실제 구현으로 교체)
    Ok(Json(ComposeResponse {
        prompt: format!("Query: {}\nFacets: {:?}", req.query_text.unwrap_or_default(), req.facets),
        sources: vec![],
        confidence: 0.5,
        tokens_used: 100,
    }))
}

/// 분류 응답
#[derive(Debug, Serialize)]
pub struct ClassifyResponse {
    pub facets: Vec<String>,
    pub confidence: f32,
}

/// 구성 응답
#[derive(Debug, Serialize)]
pub struct ComposeResponse {
    pub prompt: String,
    pub sources: Vec<String>,
    pub confidence: f32,
    pub tokens_used: u32,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.code.as_str() {
            "VALIDATION_ERROR" => StatusCode::BAD_REQUEST,
            "NOT_FOUND" => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}