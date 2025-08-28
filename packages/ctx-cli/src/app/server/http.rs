use super::dto::*;
use super::validation::ApiValidator;
use ctx_core::core::classifier::service::ClassifierService;
use ctx_core::data::index::sqlite::SqliteIndexManager;
use ctx_core::{BuildComposer, BuildQuery, LocalFsStorage, OntologyRegistry, Result as CtxResult,builtin_rules,};
use std::{path::PathBuf, sync::Arc};

use ctx_core::knowledge::rules::loader as rules_loader;
use askama::Template;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use axum_extra::extract::Form;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;
use tokio::sync::RwLock;
use tower_http::{cors::{CorsLayer, Any}, trace::TraceLayer, limit::RequestBodyLimitLayer};
use std::num::NonZeroUsize;

fn format_timestamp(timestamp: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(timestamp) {
        Ok(dt) => {
            let now = chrono::Utc::now();
            let diff = now.signed_duration_since(dt.with_timezone(&chrono::Utc));

            if diff.num_minutes() < 1 {
                "방금 전".to_string()
            } else if diff.num_minutes() < 60 {
                format!("{}분 전", diff.num_minutes())
            } else if diff.num_hours() < 24 {
                format!("{}시간 전", diff.num_hours())
            } else {
                format!("{}일 전", diff.num_days())
            }
        }
        Err(_) => timestamp.to_string(),
    }
}

// Askama 커스텀 필터 모듈
mod filters {
    #[allow(dead_code)]
    pub fn title(s: &str) -> ::askama::Result<String> {
        Ok(s.split_whitespace()
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "))
    }

    #[allow(dead_code)]
    pub fn length<T>(slice: &[T], _values: &dyn askama::Values) -> ::askama::Result<usize> {
        Ok(slice.len())
    }

    #[allow(dead_code)]
    pub fn round(value: &f32, _values: &dyn askama::Values) -> ::askama::Result<i32> {
        Ok(value.round() as i32)
    }

    #[allow(dead_code)]
    pub fn truncate(s: &str, len: usize) -> ::askama::Result<String> {
        if s.len() <= len {
            Ok(s.to_string())
        } else {
            Ok(format!("{}...", &s[..len]))
        }
    }

    #[allow(dead_code)]
    pub fn str_length(s: &str, _values: &dyn askama::Values) -> ::askama::Result<usize> {
        Ok(s.len())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_filters_usage() {
            match title("hello world") {
                Ok(v) => assert_eq!(v, "Hello World"),
                Err(e) => panic!("title filter error: {e:?}"),
            }
            match length(&[1, 2, 3]) {
                Ok(v) => assert_eq!(v, 3),
                Err(e) => panic!("length filter error: {e:?}"),
            }
            match round(&3.6_f32) {
                Ok(v) => assert_eq!(v, 4),
                Err(e) => panic!("round filter error: {e:?}"),
            }
            match truncate("abcdef", 3) {
                Ok(v) => assert_eq!(v, "abc..."),
                Err(e) => panic!("truncate filter error: {e:?}"),
            }
            match str_length("xyz") {
                Ok(v) => assert_eq!(v, 3),
                Err(e) => panic!("str_length filter error: {e:?}"),
            }
        }
    }
}

/// 10MB body limit by default (configurable by env CTX_BODY_LIMIT_MB)
fn body_limit_layer_from_env() -> RequestBodyLimitLayer {
    let mb: usize = std::env::var("CTX_BODY_LIMIT_MB")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(10);
    // NonZero guaranteed by filter
    RequestBodyLimitLayer::new(NonZeroUsize::new(mb * 1024 * 1024).unwrap().into())
}

/// Restrictive-by-default CORS
fn cors_layer_from_env() -> CorsLayer {
    if std::env::var_os("CTX_CORS_ANY").is_some() {
        return CorsLayer::very_permissive();
    }
    let allow = std::env::var("CTX_CORS_ALLOW").unwrap_or_default();
    if allow.trim().is_empty() {
        // Keep fairly limited by default (adjust as needed for your deployment)
        CorsLayer::new().allow_origin(Any)
    } else {
        // For simplicity, if any allow list provided, be permissive (can be refined to exact list)
        CorsLayer::permissive()
    }
}

/// Basic filename sanitizer to mitigate traversal and weird chars
fn sanitize_filename(raw: &str) -> Result<String, ApiError> {
    if raw.is_empty() || raw.len() > 128 {
        return Err(ApiError::validation_error("invalid filename length"));
    }
    if raw.contains('/') || raw.contains('\\') {
        return Err(ApiError::validation_error("path separator detected"));
    }
    if raw.contains("..") {
        return Err(ApiError::validation_error("path traversal detected"));
    }
    let filtered: String = raw.chars().filter(|c| c.is_ascii_graphic()).collect();
    if filtered.is_empty() {
        return Err(ApiError::validation_error("invalid filename characters"));
    }
    Ok(filtered)
}

// ===========================
// AppState
// ===========================

#[derive(Clone)]
pub struct AppState {
    pub ontology_path: PathBuf,
    pub rules_path: PathBuf,
    pub ontology_yaml: Arc<RwLock<String>>,
    pub rules_yaml: Arc<RwLock<String>>,
    pub storage: Arc<LocalFsStorage>,
    pub db_path: PathBuf,
    pub db_pool: Arc<Pool<SqliteConnectionManager>>,
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
            .map_err(ctx_core::ContextError::Io)?;
        let rules_yaml = tokio::fs::read_to_string(&rules_path)
            .await
            .map_err(ctx_core::ContextError::Io)?;

        let storage = Arc::new(LocalFsStorage::new("."));
        let db_path = PathBuf::from("ctxindex.db");

        // Build SQLite connection pool (r2d2)
        let manager = SqliteConnectionManager::file(&db_path);
        let db_pool = Arc::new(
            Pool::builder()
                .max_size(8)
                .build(manager)
                .map_err(|e| ctx_core::ContextError::Other(format!("DB pool build error: {e}")))?,
        );

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

    /// 실제 최근 작업 조회 (SQLite에서)
    pub async fn get_recent_operations(&self) -> Vec<RecentOperation> {
        match self.load_recent_operations_from_db().await {
            Ok(ops) => ops,
            Err(e) => {
                tracing::warn!("Failed to load recent operations: {}", e);
                vec![]
            }
        }
    }

    pub async fn get_dashboard_stats(&self) -> DashboardStats {
        match self.load_stats_from_db().await {
            Ok(stats) => stats,
            Err(e) => {
                tracing::warn!("Failed to load dashboard stats: {}", e);
                DashboardStats {
                    total_classifications: 0,
                    total_compositions: 0,
                    avg_confidence: 0.0,
                    active_namespaces: 0,
                }
            }
        }
    }

    async fn load_recent_operations_from_db(
        &self,
    ) -> Result<Vec<RecentOperation>, Box<dyn std::error::Error>> {
        use ctx_core::data::index::sqlite::SqliteIndexManager;

        let _index = SqliteIndexManager::new(self.db_path.to_string_lossy().as_ref())?;

        // 최근 10개 문서 조회 (classify_log와 JOIN)
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT d.doc_id, d.title, d.valid_from, COALESCE(cl.confidence, 0.0) as confidence
             FROM docs d
             LEFT JOIN classify_log cl ON d.doc_id = cl.doc_id AND d.sha = cl.sha
             WHERE d.valid_to IS NULL AND d.deleted=0
             ORDER BY d.valid_from DESC 
             LIMIT 10",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(RecentOperation {
                id: row.get::<_, String>(0)?,
                operation_type: "classify".to_string(),
                title: row.get::<_, String>(1)?,
                timestamp: format_timestamp(&row.get::<_, String>(2)?),
                confidence: Some(row.get::<_, f32>(3)?),
            })
        })?;

        let mut operations = Vec::new();
        for row in rows {
            operations.push(row?);
        }

        Ok(operations)
    }

    async fn load_stats_from_db(&self) -> Result<DashboardStats, Box<dyn std::error::Error>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;

        let total_classifications: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM docs WHERE valid_to IS NULL AND deleted=0",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let avg_confidence: f64 = conn
            .query_row(
                "SELECT AVG(cl.confidence) FROM classify_log cl 
             INNER JOIN docs d ON cl.doc_id = d.doc_id AND cl.sha = d.sha 
             WHERE d.valid_to IS NULL AND cl.confidence > 0",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let active_namespaces: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT namespace) FROM facet_values",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let total_compositions: i64 = conn
            .query_row("SELECT COUNT(*) FROM compose_log", [], |row| row.get(0))
            .unwrap_or(0);

        Ok(DashboardStats {
            total_classifications: total_classifications as usize,
            total_compositions: total_compositions as usize,
            avg_confidence: (avg_confidence * 100.0) as f32,
            active_namespaces: active_namespaces as usize,
        })
    }
}

// ===========================
// Askama Templates
// ===========================

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    recent_operations: Vec<RecentOperation>,
    stats: DashboardStats,
}

#[derive(Template)]
#[template(path = "classify_form.html")]
struct ClassifyFormTemplate;

#[derive(Template)]
#[template(path = "classify_result.html")]
#[allow(dead_code)]
struct ClassifyResultTemplate {
    facets: BTreeMap<String, Vec<String>>,
    confidence: f32,
    confidence_percent: i32,
    confidence_color: String,
    warnings: Vec<String>,
    errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "compose_form.html")]
struct ComposeFormTemplate;

#[derive(Template)]
#[template(path = "compose_result.html")]
#[allow(dead_code)]
struct ComposeResultTemplate {
    prompt: String,
    trace_id: String,
    metadata: ComposeMetadata,
}

#[derive(Template)]
#[template(path = "compose_builder_result.html")]
#[allow(dead_code)]
struct ComposeBuilderResultTemplate {
    prompt: String,
    metadata: ComposeBuilderMetadata,
}

#[derive(Debug, Serialize)]
struct ComposeBuilderMetadata {
    tokens_used: u32,
    documents_selected: usize,
    confidence: f32,
    confidence_percent: u32,
    rejected_count: usize,
    rationale: String,
    source_documents: Vec<String>,
}

#[derive(Template)]
#[template(path = "studio_rules.html")]
struct StudioRulesTemplate {
    rules_content: String,
}

#[derive(Template)]
#[template(path = "studio_ontology.html")]
struct StudioOntologyTemplate {
    ontology_content: String,
}

// 부분 템플릿들 (HTMX용)
#[derive(Template)]
#[template(path = "contexts_list_partial.html")]
struct ContextsListPartialTemplate {
    documents: Vec<DocSummary>,
}

#[derive(Template)]
#[template(path = "context_detail_partial.html")]
struct ContextDetailPartialTemplate {
    doc: DocDetail,
}

#[derive(Template)]
#[template(path = "db_dashboard_partial.html")]
struct DbDashboardPartialTemplate {
    tables: Vec<DbTableInfo>,
}

#[derive(Template)]
#[template(path = "db_table_partial.html")]
struct DbTablePartialTemplate {
    data: DbTableData,
    has_prev: bool,
    prev_offset: u32,
    has_next: bool,
    next_offset: u32,
}

// ===========================
// Helper Functions
// ===========================

fn is_htmx_request(headers: &HeaderMap) -> bool {
    headers.get("hx-request").is_some()
}

// ===========================
// Router
// ===========================

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Pages (HTMX/Form)
        .route("/", get(page_dashboard))
        .route("/classify", get(page_classify).post(ui_classify))
        .route("/compose", get(page_compose))
        .route(
            "/compose",
            post(
                |state: axum::extract::State<AppState>,
                 form: Form<ComposeFormInput>| async move { ui_compose(state, form).await },
            ),
        )
        .route("/compose/builder", get(page_compose_builder))
        .route(
            "/compose/build",
            post(
                |state: axum::extract::State<AppState>,
                 form: Form<ComposeBuilderForm>| async move { ui_compose_build(state, form).await },
            ),
        )
        .route("/studio/rules", get(page_studio_rules))
        .route("/studio/ontology", get(page_studio_ontology))
        .route("/contexts", get(page_contexts))
        .route("/contexts/:doc_id", get(page_context_detail))
        // DB Dashboard (read-only)
        .route("/db", get(page_db_dashboard))
        .route("/db/table/:name", get(page_db_table))
        .route("/db/query", get(page_db_query).post(ui_db_query))
        // APIs (JSON)
        .route("/v1/classify", post(api_classify_json))
        .route("/v1/validate", post(api_validate_json))
        .route(
            "/v1/compose",
            post(
                |state: axum::extract::State<AppState>,
                 body: axum::Json<ComposeRequest>| async move { api_compose_json(state, body).await },
            ),
        )
        .route("/v1/rules", get(api_get_rules).put(api_put_rules))
        .route("/v1/ontology", get(api_get_ontology).put(api_put_ontology))
        .route("/v1/validate/rules", post(api_validate_rules_yaml))
        .route("/v1/validate/ontology", post(api_validate_ontology_yaml))
        // Webhook APIs
        .route("/v1/hooks/:provider", post(api_webhook_handler))
        // Document management
        .route("/v1/documents", get(api_list_documents))
        .route(
            "/v1/documents/*doc_id",
            axum::routing::delete(api_delete_document),
        )
        .route("/v1/upload", post(api_upload_file))
        // DB management
        .route("/v1/db/clear", axum::routing::delete(api_clear_database))
        .route("/v1/db/tables", get(api_db_tables))
        .route("/v1/db/table/:name", get(api_db_table_rows))
        .route("/healthz", get(health_check))
        .with_state(state)
        .layer(cors_layer_from_env())
        .layer(body_limit_layer_from_env())
        .layer(TraceLayer::new_for_http())
}

// ===========================
// Page Handlers (SSR)
// ===========================

async fn page_dashboard(State(state): State<AppState>) -> impl IntoResponse {
    let recent_operations = state.get_recent_operations().await;
    let stats = state.get_dashboard_stats().await;
    let template = DashboardTemplate {
        recent_operations,
        stats,
    };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template render error".to_string(),
        )
            .into_response(),
    }
}

async fn page_classify() -> impl IntoResponse {
    match ClassifyFormTemplate.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template render error".to_string(),
        )
            .into_response(),
    }
}

async fn page_compose() -> impl IntoResponse {
    match ComposeFormTemplate.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template render error".to_string(),
        )
            .into_response(),
    }
}

#[derive(Template)]
#[template(path = "compose_builder.html")]
struct ComposeBuilderTemplate;

async fn page_compose_builder() -> impl IntoResponse {
    match ComposeBuilderTemplate.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template render error".to_string(),
        )
            .into_response(),
    }
}

async fn page_studio_rules(State(state): State<AppState>) -> impl IntoResponse {
    let rules_content = state.rules_yaml.read().await.clone();
    let template = StudioRulesTemplate { rules_content };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template render error".to_string(),
        )
            .into_response(),
    }
}

async fn page_studio_ontology(State(state): State<AppState>) -> impl IntoResponse {
    let ontology_content = state.ontology_yaml.read().await.clone();
    let template = StudioOntologyTemplate { ontology_content };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template render error".to_string(),
        )
            .into_response(),
    }
}

// ===========================
// Context Pages (SSR)
// ===========================

async fn page_contexts(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let pool = Arc::clone(&state.db_pool);
    let res: CtxResult<Vec<serde_json::Value>> = tokio::task::spawn_blocking(move || {
        let conn = pool.get().map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT d.doc_id, d.title, d.path, d.valid_from, COALESCE(cl.confidence, 0.0) as confidence
                 FROM docs d
                 LEFT JOIN classify_log cl ON d.doc_id = cl.doc_id AND d.sha = cl.sha
                 WHERE d.valid_to IS NULL AND d.deleted=0
                 ORDER BY d.valid_from DESC LIMIT 200",
            )
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "doc_id": row.get::<_, String>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "path": row.get::<_, String>(2)?,
                    "uploaded_at": row.get::<_, String>(3)?,
                    "confidence": row.get::<_, f32>(4)?,
                }))
            })
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut v = Vec::new();
        for r in rows { v.push(r.map_err(|e| ctx_core::ContextError::Other(e.to_string()))?); }
        Ok(v)
    })
    .await
    .unwrap_or_else(|e| Err(ctx_core::ContextError::Other(format!("join error: {e}"))));

    match res {
        Ok(v) => {
            let documents: Vec<DocSummary> = v
                .into_iter()
                .map(|j| DocSummary {
                    doc_id: j["doc_id"].as_str().unwrap_or("").to_string(),
                    title: j["title"].as_str().unwrap_or("").to_string(),
                    path: j["path"].as_str().unwrap_or("").to_string(),
                    uploaded_at: j["uploaded_at"].as_str().unwrap_or("").to_string(),
                    confidence: (j["confidence"].as_f64().unwrap_or(0.0) * 100.0) as f32,
                })
                .collect();

            // HTMX 요청인지 확인
            if is_htmx_request(&headers) {
                // 부분 템플릿 사용
                match (ContextsListPartialTemplate { documents }).render() {
                    Ok(html) => Html(html).into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Template error: {e}"),
                    )
                        .into_response(),
                }
            } else {
                // 전체 페이지 템플릿 사용
                #[derive(Template)]
                #[template(path = "contexts_list.html")]
                struct ContextsTemplate {
                    documents: Vec<DocSummary>,
                }

                match (ContextsTemplate { documents }).render() {
                    Ok(html) => Html(html).into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Template error: {e}"),
                    )
                        .into_response(),
                }
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")).into_response(),
    }
}

async fn page_context_detail(
    State(state): State<AppState>,
    Path(doc_id_encoded): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // URL 디코딩 (간단한 방법)
    let doc_id = doc_id_encoded.replace("%2F", "/");
    let pool = Arc::clone(&state.db_pool);
    let res: CtxResult<serde_json::Value> = tokio::task::spawn_blocking(move || {
        let conn = pool.get().map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        // latest snapshot
        let (title, path): (String, String) = conn
            .query_row(
                "SELECT title, path FROM docs WHERE doc_id=?1 AND valid_to IS NULL",
                [doc_id.clone()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((String::new(), String::new()));
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!("Failed to read file {}: {}", path, e);
                format!("⚠️ 파일을 읽을 수 없습니다: {}\n\n오류: {}", path, e)
            }
        };

        let mut facets_stmt = conn
            .prepare("SELECT namespace,value FROM doc_facets WHERE doc_id=?1 AND sha=(SELECT sha FROM docs WHERE doc_id=?1 AND valid_to IS NULL)")
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut rows = facets_stmt
            .query([doc_id.clone()])
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut facets: BTreeMap<String, Vec<String>> = BTreeMap::new();
        while let Some(r) = rows.next().map_err(|e| ctx_core::ContextError::Other(e.to_string()))? {
            let ns: String = r.get(0).map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
            let val: String = r.get(1).map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
            facets.entry(ns).or_default().push(val);
        }

        let confidence: f32 = conn
            .query_row(
                "SELECT COALESCE(cl.confidence,0.0) FROM classify_log cl WHERE cl.doc_id=?1 AND cl.sha=(SELECT sha FROM docs WHERE doc_id=?1 AND valid_to IS NULL)",
                [doc_id.clone()],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        Ok(serde_json::json!({
            "doc_id": doc_id,
            "title": title,
            "path": path,
            "confidence": confidence,
            "facets": facets,
            "content": content,
        }))
    })
    .await
    .unwrap_or_else(|e| Err(ctx_core::ContextError::Other(format!("join error: {e}"))));

    // Note: context detail template struct is defined inline where used below.

    match res {
        Ok(j) => {
            let facets_map = j["facets"].as_object().cloned().unwrap_or_default();
            let mut facets: Vec<(String, String)> = facets_map
                .into_iter()
                .map(|(k, v)| {
                    let vv = v.as_array().cloned().unwrap_or_default();
                    let s = vv
                        .into_iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                        .join(", ");
                    (k, s)
                })
                .collect();
            facets.sort_by(|a, b| a.0.cmp(&b.0));
            let doc = DocDetail {
                doc_id: j["doc_id"].as_str().unwrap_or("").to_string(),
                title: j["title"].as_str().unwrap_or("").to_string(),
                path: j["path"].as_str().unwrap_or("").to_string(),
                confidence: (j["confidence"].as_f64().unwrap_or(0.0) * 100.0) as f32,
                facets,
                content: j["content"].as_str().unwrap_or("").to_string(),
            };

            // HTMX 요청인지 확인
            if is_htmx_request(&headers) {
                // 부분 템플릿 사용
                match (ContextDetailPartialTemplate { doc }).render() {
                    Ok(html) => Html(html).into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Template error: {e}"),
                    )
                        .into_response(),
                }
            } else {
                // 전체 페이지 템플릿 사용
                #[derive(Template)]
                #[template(path = "context_detail.html")]
                struct ContextDetailTemplate {
                    doc: DocDetail,
                }
                match (ContextDetailTemplate { doc }).render() {
                    Ok(html) => Html(html).into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Template error: {e}"),
                    )
                        .into_response(),
                }
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")).into_response(),
    }
}

// ===========================
// DB Dashboard Handlers
// ===========================

async fn page_db_dashboard(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let db_path = state.db_path.clone();
    let tables: Vec<DbTableInfo> = tokio::task::spawn_blocking(move || -> CtxResult<_> {
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY 1")
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut rows = stmt.query([]).map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut v = Vec::new();
        while let Some(r) = rows.next().map_err(|e| ctx_core::ContextError::Other(e.to_string()))? {
            let name: String = r.get(0).map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
            let cnt: u64 = conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM \"{}\"", name.replace('"', "\"\"")),
                    [],
                    |rr| rr.get(0),
                )
                .unwrap_or(0);
            v.push(DbTableInfo { name, rows: cnt });
        }
        Ok(v)
    })
    .await
    .unwrap_or_else(|e| Err(ctx_core::ContextError::Other(format!("join error: {e}"))))
    .unwrap_or_default();

    // HTMX 요청인지 확인
    if is_htmx_request(&headers) {
        // 부분 템플릿 사용
        match (DbDashboardPartialTemplate { tables }).render() {
            Ok(html) => Html(html).into_response(),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "<div>Template error</div>".to_string(),
            )
                .into_response(),
        }
    } else {
        // 전체 페이지 템플릿 사용
        #[derive(Template)]
        #[template(path = "db_dashboard.html")]
        struct DbDashTmpl {
            tables: Vec<DbTableInfo>,
        }
        let tmpl = DbDashTmpl { tables };
        Html(
            tmpl.render()
                .unwrap_or_else(|_| "<div>Template error</div>".into()),
        )
        .into_response()
    }
}

#[derive(serde::Deserialize)]
struct Page {
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn page_db_table(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(Page { limit, offset }): Query<Page>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let pool = Arc::clone(&state.db_pool);
    let name_clone = name.clone();
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let data: CtxResult<DbTableData> = tokio::task::spawn_blocking(move || {
        let conn = pool.get().map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let safe = name_clone.replace('"', "\"\"");
        let total: u64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM \"{}\"", safe), [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        // columns
        let col_stmt = conn
            .prepare(&format!("SELECT * FROM \"{}\" LIMIT 0", safe))
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let columns = col_stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        drop(col_stmt);

        let sql = format!("SELECT * FROM \"{}\" LIMIT ? OFFSET ?", safe);
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut rows = stmt
            .query((limit as i64, offset as i64))
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut out_rows = Vec::new();
        while let Some(r) = rows
            .next()
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?
        {
            let mut row_vec = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                let v: rusqlite::types::Value =
                    r.get(i).map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
                row_vec.push(match v {
                    rusqlite::types::Value::Null => "NULL".into(),
                    rusqlite::types::Value::Integer(n) => n.to_string(),
                    rusqlite::types::Value::Real(f) => f.to_string(),
                    rusqlite::types::Value::Text(t) => t,
                    rusqlite::types::Value::Blob(_) => "[BLOB]".into(),
                });
            }
            out_rows.push(row_vec);
        }

        Ok(DbTableData {
            name: name_clone,
            columns,
            rows: out_rows,
            limit,
            offset,
            total,
        })
    })
    .await
    .unwrap_or_else(|e| Err(ctx_core::ContextError::Other(format!("join error: {e}"))));

    #[derive(Template)]
    #[template(path = "db_table.html")]
    struct DbTableTmpl {
        data: DbTableData,
        has_prev: bool,
        prev_offset: u32,
        has_next: bool,
        next_offset: u32,
    }

    match data {
        Ok(d) => {
            let has_prev = d.offset > 0;
            let prev_offset = d.offset.saturating_sub(d.limit);
            let has_next = (d.offset as u64 + d.limit as u64) < d.total;
            let next_offset = d.offset + d.limit;

            // HTMX 요청인지 확인
            if is_htmx_request(&headers) {
                // 부분 템플릿 사용
                match (DbTablePartialTemplate {
                    data: d,
                    has_prev,
                    prev_offset,
                    has_next,
                    next_offset,
                })
                .render()
                {
                    Ok(html) => Html(html).into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Template error: {e}"),
                    )
                        .into_response(),
                }
            } else {
                // 전체 페이지 템플릿 사용 (상단 정의 재사용)
                match (DbTableTmpl {
                    data: d,
                    has_prev,
                    prev_offset,
                    has_next,
                    next_offset,
                })
                .render()
                {
                    Ok(html) => Html(html).into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Template error: {e}"),
                    )
                        .into_response(),
                }
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")).into_response(),
    }
}

async fn page_db_query(_state: State<AppState>) -> impl IntoResponse {
    #[derive(Template)]
    #[template(path = "db_query.html")]
    struct DbQueryTmpl;
    match DbQueryTmpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {e}"),
        )
            .into_response(),
    }
}

async fn ui_db_query(
    State(state): State<AppState>,
    Form(form): Form<DbQueryForm>,
) -> impl IntoResponse {
    // read-only guard unless db_write feature enabled
    let sql_trim = form.sql.trim().to_uppercase();
    if [
        "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "REINDEX", "VACUUM", "ATTACH",
        "DETACH", "REPLACE",
    ]
    .iter()
    .any(|kw| sql_trim.starts_with(kw))
        && !cfg!(feature = "db_write")
    {
        return (
            StatusCode::BAD_REQUEST,
            "<div>Write operations are disabled</div>".to_string(),
        )
            .into_response();
    }

    let pool = Arc::clone(&state.db_pool);
    let start = Instant::now();
    let result: CtxResult<DbQueryResult> = tokio::task::spawn_blocking(move || {
        let conn = pool.get().map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut stmt = conn
            .prepare(&form.sql)
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let columns = stmt
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let mut rows_iter = stmt
            .query([])
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut rows = Vec::new();
        let limit = form.limit.unwrap_or(200) as usize;
        while let Some(r) = rows_iter
            .next()
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?
        {
            let mut row_vec = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                let v: rusqlite::types::Value =
                    r.get(i).map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
                row_vec.push(match v {
                    rusqlite::types::Value::Null => "NULL".into(),
                    rusqlite::types::Value::Integer(n) => n.to_string(),
                    rusqlite::types::Value::Real(f) => f.to_string(),
                    rusqlite::types::Value::Text(t) => t,
                    rusqlite::types::Value::Blob(_) => "[BLOB]".into(),
                });
            }
            rows.push(row_vec);
            if rows.len() >= limit {
                break;
            }
        }
        Ok(DbQueryResult {
            columns,
            rows,
            elapsed_ms: start.elapsed().as_millis(),
        })
    })
    .await
    .unwrap_or_else(|e| Err(ctx_core::ContextError::Other(format!("join error: {e}"))));

    match result {
        Ok(r) => {
            #[derive(Template)]
            #[template(path = "db_query_result.html")]
            struct DbQueryResultTmpl {
                res: DbQueryResult,
            }
            match (DbQueryResultTmpl { res: r }).render() {
                Ok(html) => Html(html).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Template error: {e}"),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("<div>DB error: {e}</div>"),
        )
            .into_response(),
    }
}

async fn api_db_tables(State(state): State<AppState>) -> impl IntoResponse {
    let pool = Arc::clone(&state.db_pool);
    let res: CtxResult<Vec<DbTableInfo>> = tokio::task::spawn_blocking(move || {
        let conn = pool.get().map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY 1")
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut rows = stmt.query([]).map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let mut v = Vec::new();
        while let Some(r) = rows.next().map_err(|e| ctx_core::ContextError::Other(e.to_string()))? {
            let name: String = r.get(0).map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
            let cnt: u64 = conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM \"{}\"", name.replace('"', "\"\"")),
                    [],
                    |rr| rr.get(0),
                )
                .unwrap_or(0);
            v.push(DbTableInfo { name, rows: cnt });
        }
        Ok(v)
    })
    .await
    .unwrap_or_else(|e| Err(ctx_core::ContextError::Other(format!("join error: {e}"))));

    match res {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::internal_error(e.to_string())),
        )
            .into_response(),
    }
}

async fn api_db_table_rows(
    _state: State<AppState>,
    _path: Path<String>,
    _page: Query<Page>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiError::new("not implemented", "NOT_IMPL")),
    )
        .into_response()
}

// ===========================
// Internal Logic (공용)
// ===========================

async fn classify_internal(state: &AppState, req: ClassifyRequest) -> CtxResult<ClassifyResponse> {
    let onto_yaml = state.ontology_yaml.read().await.clone();
    let rules_yaml = state.rules_yaml.read().await.clone();

    // 실제 엔진 호출: ClassifierService
    let out = ClassifierService::classify_with_yaml(
        &onto_yaml,
        Some(&rules_yaml),
        &req.title,
        &req.body,
    )?;

    // 결과 정렬 맵으로 변환
    let mut facets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (ns, vals) in out.facets().to_hashmap() {
        let mut v: Vec<_> = vals.into_iter().collect();
        v.sort();
        facets.insert(ns, v);
    }

    Ok(ClassifyResponse {
        facets,
        confidence: out.confidence(),
        warnings: out.warnings().to_vec(),
        errors: out.errors().to_vec(),
    })
}

async fn validate_internal(state: &AppState, req: ValidateRequest) -> CtxResult<serde_json::Value> {
    let onto_yaml = state.ontology_yaml.read().await.clone();
    let onto = OntologyRegistry::from_yaml(&onto_yaml)?;
    let rules = builtin_rules();

    let mut setmap: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    for (ns, vals) in req.facets {
        setmap.insert(ns, vals.into_iter().collect());
    }

    let mut warnings = Vec::new();
    let mut fixes = Vec::new();
    let mut valid = true;

    let has = |ns: &str, v: &str| -> bool {
        let canon = onto.normalize(ns, v).unwrap_or_else(|| v.to_string());
        setmap
            .get(ns)
            .map(|s| s.contains(&canon) || s.contains(v))
            .unwrap_or(false)
    };

    // 금지 조합
    for r in rules.prohibited.iter() {
        if has(&r.a_ns, &r.a_val) && has(&r.b_ns, &r.b_val) {
            valid = false;
            warnings.push(format!(
                "prohibited: {}:{} with {}:{} — {}",
                r.a_ns, r.a_val, r.b_ns, r.b_val, r.reason
            ));
            fixes.push(json!({
                "op": "remove",
                "ns": r.b_ns,
                "value": r.b_val,
                "reason": "prohibited"
            }));
        }
    }

    // 필수 조합
    for r in rules.requires.iter() {
        if has(&r.a_ns, &r.a_val) && !has(&r.b_ns, &r.b_val) {
            warnings.push(format!(
                "requires: {}:{} requires {}:{} — {}",
                r.a_ns, r.a_val, r.b_ns, r.b_val, r.reason
            ));
            fixes.push(json!({
                "op": "add",
                "ns": r.b_ns,
                "value": r.b_val,
                "reason": "requires"
            }));
        }
    }

    Ok(json!({
        "valid": valid,
        "warnings": warnings,
        "fixes": fixes
    }))
}

async fn compose_internal(state: &AppState, req: ComposeRequest) -> CtxResult<serde_json::Value> {
    if req.repo.is_empty() || req.branch.is_empty() || req.commit_sha.is_empty() {
        return Err(ctx_core::ContextError::Other(
            "repo/branch/commit_sha required".into(),
        ));
    }

    // Run non-Send compose pipeline in a blocking thread
    #[derive(Debug)]
    struct ComposeOutcome {
        merged_content: String,
        source_documents: Vec<String>,
        rationale: String,
        tokens_used: u32,
        confidence: f32,
        rejected_count: usize,
    }

    let db_path_str = state.db_path.to_string_lossy().to_string();
    let repo = req.repo.clone();
    let branch = req.branch.clone();
    let commit_sha = req.commit_sha.clone();
    let facets = req.facets.clone();
    let lang = req.lang.clone();
    let maturity = req.maturity.clone();
    let query_text = req.query_text.clone();
    let confidence_threshold = req.confidence_threshold;
    let budget = req.budget.unwrap_or(2000);
    let reserve = req.reserve.unwrap_or(200);

    let outcome: ComposeOutcome =
        tokio::task::spawn_blocking(move || -> CtxResult<ComposeOutcome> {
            let mut query = BuildQuery::new(repo.clone(), branch.clone(), commit_sha.clone());
            for (ns, vals) in &facets {
                query = query.with_facet(ns.clone(), vals.clone());
            }
            if let Some(x) = lang.clone() {
                query = query.with_language(x);
            }
            if let Some(x) = maturity.clone() {
                query = query.with_maturity(x);
            }
            if let Some(x) = query_text.clone() {
                query = query.with_query_text(x);
            }
            if let Some(x) = confidence_threshold {
                query = query.with_confidence_threshold(x);
            }

            let index = SqliteIndexManager::new(&db_path_str)?;
            let candidates = index.load_candidates(&query)?;
            let composer = BuildComposer::new()?;
            let result = composer.compose(candidates, &query, budget, reserve)?;
            Ok(ComposeOutcome {
                merged_content: result.merged_document.content,
                source_documents: result.merged_document.source_documents,
                rationale: result.selection_rationale,
                tokens_used: result.total_tokens_used as u32,
                confidence: result.confidence_score,
                rejected_count: result.rejected_documents.len(),
            })
        })
        .await
        .map_err(|e| ctx_core::ContextError::Other(format!("join error: {e}")))??;

    // Track composition in DB (best-effort)
    {
        let db_path = state.db_path.clone();
        let repo = req.repo.clone();
        let branch = req.branch.clone();
        let commit_sha = req.commit_sha.clone();
        let facets_json = serde_json::to_string(&req.facets).unwrap_or("{}".into());
        let tokens_used = outcome.tokens_used as i64;
        let documents_selected = outcome.source_documents.len() as i64;
        let confidence = outcome.confidence as f64;
        tokio::task::spawn_blocking(move || -> CtxResult<()> {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
            conn.execute(
                "INSERT INTO compose_log(created_at,repo,branch,commit_sha,facets_json,tokens_used,documents_selected,confidence) VALUES(?,?,?,?,?,?,?,?)",
                (
                    chrono::Utc::now().to_rfc3339(),
                    repo,
                    branch,
                    commit_sha,
                    facets_json,
                    tokens_used,
                    documents_selected,
                    confidence,
                ),
            )
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
            Ok(())
        })
        .await
        .ok();
    }

    Ok(json!({
        "merged_content": outcome.merged_content,
        "source_documents": outcome.source_documents,
        "rationale": outcome.rationale,
        "tokens_used": outcome.tokens_used,
        "confidence": outcome.confidence,
        "rejected_count": outcome.rejected_count
    }))
}

// ===========================
// UI Handlers (HTMX/Form)
// ===========================

async fn ui_classify(
    State(state): State<AppState>,
    Form(req): Form<ClassifyRequest>,
) -> impl IntoResponse {
    match classify_internal(&state, req).await {
        Ok(resp) => {
            let confidence_percent = (resp.confidence * 100.0) as i32;
            let confidence_color = if resp.confidence > 0.8 {
                "bg-green-500".to_string()
            } else if resp.confidence > 0.6 {
                "bg-yellow-500".to_string()
            } else {
                "bg-red-500".to_string()
            };

            let tpl = ClassifyResultTemplate {
                facets: resp.facets,
                confidence: resp.confidence,
                confidence_percent,
                confidence_color,
                warnings: resp.warnings,
                errors: resp.errors,
            };
            match tpl.render() {
                Ok(html) => Html(html).into_response(),
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "<div>Template render error</div>".to_string(),
                )
                    .into_response(),
            }
        }
        Err(e) => {
            tracing::error!("Classification failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("<div>Error: {}</div>", e),
            )
                .into_response()
        }
    }
}

// 간단한 폼 입력을 위한 모델
#[derive(serde::Deserialize)]
struct ComposeFormInput {
    repo: String,
    branch: String,
    commit_sha: String,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    framework: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    query_text: Option<String>,
    #[serde(default)]
    confidence_threshold: Option<f32>,
    #[serde(default)]
    budget: Option<usize>,
    #[serde(default)]
    reserve: Option<usize>,
}

// 조합 빌더 폼 입력
#[derive(Debug, Deserialize)]
struct ComposeBuilderForm {
    repo: String,
    branch: String,
    commit_sha: String,
    #[serde(default)]
    query_text: Option<String>,
    #[serde(default)]
    confidence_threshold: Option<f32>,
    #[serde(default)]
    budget: Option<usize>,
    #[serde(default)]
    reserve: Option<usize>,
    #[serde(default)]
    facets_json: Option<String>,
}

impl TryFrom<ComposeFormInput> for ComposeRequest {
    type Error = ctx_core::ContextError;
    fn try_from(v: ComposeFormInput) -> CtxResult<Self> {
        // 선택된 값들로 facets 맵 구성
        let mut facets: BTreeMap<String, Vec<String>> = BTreeMap::new();

        if let Some(platform) = v.platform.filter(|s| !s.is_empty()) {
            facets.insert("platform".to_string(), vec![platform]);
        }
        if let Some(language) = v.language.filter(|s| !s.is_empty()) {
            facets.insert("lang".to_string(), vec![language]);
        }
        if let Some(framework) = v.framework.filter(|s| !s.is_empty()) {
            facets.insert("framework".to_string(), vec![framework]);
        }
        if let Some(action) = v.action.filter(|s| !s.is_empty()) {
            facets.insert("action".to_string(), vec![action]);
        }

        Ok(ComposeRequest {
            repo: v.repo,
            branch: v.branch,
            commit_sha: v.commit_sha,
            facets,
            lang: None,     // 폼에서 language로 처리됨
            maturity: None, // 기본값 사용
            query_text: v.query_text,
            confidence_threshold: v.confidence_threshold,
            budget: v.budget,
            reserve: v.reserve,
        })
    }
}

async fn ui_compose(
    State(state): State<AppState>,
    Form(form): Form<ComposeFormInput>,
) -> impl IntoResponse {
    let req = match ComposeRequest::try_from(form) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("<div>Invalid form: {}</div>", e),
            )
                .into_response();
        }
    };

    match compose_internal(&state, req).await {
        Ok(val) => {
            let tpl = ComposeResultTemplate {
                prompt: val["merged_content"].as_str().unwrap_or("").to_string(),
                trace_id: "trace-123".to_string(),
                metadata: ComposeMetadata {
                    tokens_used: val["tokens_used"].as_u64().unwrap_or(0) as u32,
                    documents_selected: val["source_documents"]
                        .as_array()
                        .map(|a| a.len())
                        .unwrap_or(0),
                    selection_method: "MMR".to_string(),
                },
            };
            match tpl.render() {
                Ok(html) => Html(html).into_response(),
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "<div>Template render error</div>".to_string(),
                )
                    .into_response(),
            }
        }
        Err(e) => {
            tracing::error!("Composition failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("<div>Error: {}</div>", e),
            )
                .into_response()
        }
    }
}

// ===========================
// API Handlers (JSON)
// ===========================

async fn api_classify_json(
    State(state): State<AppState>,
    Json(req): Json<ClassifyRequest>,
) -> impl IntoResponse {
    if let Err(e) = ApiValidator::validate_classify(&req) {
        return (StatusCode::BAD_REQUEST, Json(e)).into_response();
    }
    match classify_internal(&state, req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => {
            tracing::error!("Classification failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError::from(e))).into_response()
        }
    }
}

async fn api_validate_json(
    State(state): State<AppState>,
    Json(req): Json<ValidateRequest>,
) -> impl IntoResponse {
    if let Err(e) = ApiValidator::validate_validate(&req) {
        return (StatusCode::BAD_REQUEST, Json(e)).into_response();
    }
    // Intent gate: simple policy example
    let mut contract = std::collections::HashMap::new();
    contract.insert(
        "category:legacy_edit".to_string(),
        vec!["refactor".to_string(), "migrate".to_string()],
    );
    if let Err(e) = ApiValidator::apply_intent_gate(&req.facets, &contract) {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiError {
                error: e.error,
                code: e.code,
                details: None,
                suggestions: contract.get("category:legacy_edit").cloned(),
            }),
        )
            .into_response();
    }
    match validate_internal(&state, req).await {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => {
            tracing::error!("Validate failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError::from(e))).into_response()
        }
    }
}

async fn api_compose_json(
    State(state): State<AppState>,
    Json(req): Json<ComposeRequest>,
) -> impl IntoResponse {
    if let Err(e) = ApiValidator::validate_compose(&req) {
        return (StatusCode::BAD_REQUEST, Json(e)).into_response();
    }
    // Intent gate (same simple policy)
    let mut contract = std::collections::HashMap::new();
    contract.insert(
        "category:legacy_edit".to_string(),
        vec!["refactor".to_string(), "migrate".to_string()],
    );
    if let Err(e) = ApiValidator::apply_intent_gate(&req.facets, &contract) {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiError {
                error: e.error,
                code: e.code,
                details: None,
                suggestions: contract.get("category:legacy_edit").cloned(),
            }),
        )
            .into_response();
    }
    match compose_internal(&state, req).await {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => {
            tracing::error!("Composition failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError::from(e))).into_response()
        }
    }
}

// ===========================
// Rules/Ontology & Health
// ===========================

async fn api_get_rules(State(state): State<AppState>) -> impl IntoResponse {
    let rules = state.rules_yaml.read().await.clone();
    (StatusCode::OK, Json(json!({ "rules": rules }))).into_response()
}

async fn api_put_rules(
    State(state): State<AppState>,
    Json(req): Json<PutTextRequest>,
) -> impl IntoResponse {
    // Validate rules YAML before saving
    if let Err(e) = rules_loader::load_yaml_rules_from_string(&req.text) {
        tracing::warn!("Invalid rules YAML: {}", e);
        return (
            StatusCode::BAD_REQUEST,
            Json(json!(ApiError::validation_error("Invalid rules format"))),
        )
            .into_response();
    }

    match tokio::fs::write(&state.rules_path, &req.text).await {
        Ok(_) => {
            *state.rules_yaml.write().await = req.text;
            (StatusCode::NO_CONTENT, Json(json!({}))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to write rules file: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!(ApiError::internal_error("Failed to save rules"))),
            )
                .into_response()
        }
    }
}

async fn api_get_ontology(State(state): State<AppState>) -> impl IntoResponse {
    let onto = state.ontology_yaml.read().await.clone();
    (StatusCode::OK, Json(json!({ "ontology": onto }))).into_response()
}

async fn api_put_ontology(
    State(state): State<AppState>,
    Json(req): Json<PutTextRequest>,
) -> impl IntoResponse {
    if let Err(e) = OntologyRegistry::from_yaml(&req.text) {
        tracing::warn!("Invalid ontology YAML: {}", e);
        return (
            StatusCode::BAD_REQUEST,
            Json(json!(ApiError::validation_error("Invalid ontology format"))),
        )
            .into_response();
    }

    match tokio::fs::write(&state.ontology_path, &req.text).await {
        Ok(_) => {
            *state.ontology_yaml.write().await = req.text;
            (StatusCode::NO_CONTENT, Json(json!({}))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to write ontology file: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!(ApiError::internal_error("Failed to save ontology"))),
            )
                .into_response()
        }
    }
}

// Validation endpoints (do not persist)
async fn api_validate_rules_yaml(Json(req): Json<PutTextRequest>) -> impl IntoResponse {
    match rules_loader::load_yaml_rules_from_string(&req.text) {
        Ok(_) => (StatusCode::OK, Json(json!({ "valid": true }))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "valid": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn api_validate_ontology_yaml(Json(req): Json<PutTextRequest>) -> impl IntoResponse {
    match OntologyRegistry::from_yaml(&req.text) {
        Ok(_) => (StatusCode::OK, Json(json!({ "valid": true }))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "valid": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn api_upload_file(
    State(state): State<AppState>,
    Json(req): Json<FileUploadRequest>,
) -> impl IntoResponse {
    tracing::info!("File upload request: {}", req.filename);

    // 1. 파일 내용을 분류
    let classify_req = ClassifyRequest {
        title: req.filename.clone(),
        body: req.content.clone(),
        path: req.path.clone(),
    };

    let classification = match classify_internal(&state, classify_req).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Classification failed: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::internal_error(format!(
                    "Classification failed: {}",
                    e
                ))),
            )
                .into_response();
        }
    };

    // 2. 문서 ID 생성
    let doc_id = generate_doc_id(&req.filename, &req.content);

    // 3. 파일 저장 경로 결정 (파일명 검증 포함)
    let base_path = std::path::PathBuf::from("./documents");
    let filename = match sanitize_filename(&req.filename) {
        Ok(n) => n,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(e)).into_response(),
    };
    let save_path = if let Some(path) = &req.path {
        base_path.join(path).join(&filename)
    } else {
        base_path.join(&filename)
    };

    // 4. 디렉토리 생성
    if let Some(parent) = save_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            tracing::error!("Failed to create directory: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::internal_error("Failed to create directory")),
            )
                .into_response();
        }
    }

    // 5. 파일 저장
    if let Err(e) = tokio::fs::write(&save_path, &req.content).await {
        tracing::error!("Failed to save file: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::internal_error("Failed to save file")),
        )
            .into_response();
    }

    // 6. SQLite에 문서 정보 저장
    if let Err(e) = save_document_to_db(&state, &doc_id, &req, &classification, &save_path).await {
        tracing::error!("Failed to save to database: {}", e);
        // 파일은 저장되었지만 DB 저장 실패 - 경고만 로그
    }

    tracing::info!(
        "File uploaded successfully: {} -> {}",
        req.filename,
        save_path.display()
    );

    let response = FileUploadResponse {
        success: true,
        filename: req.filename,
        doc_id,
        classification,
        saved_path: save_path.to_string_lossy().to_string(),
    };

    (StatusCode::OK, Json(response)).into_response()
}

fn generate_doc_id(filename: &str, content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(filename.as_bytes());
    hasher.update(content.as_bytes());
    let hash = hasher.finalize();
    format!("{:x}", hash)[..16].to_string()
}

async fn save_document_to_db(
    state: &AppState,
    doc_id: &str,
    req: &FileUploadRequest,
    classification: &ClassifyResponse,
    save_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use ctx_core::data::index::sqlite::{DocumentIndex, SqliteIndexManager};
    use std::collections::HashMap;

    // SQLite 매니저 생성
    let index = SqliteIndexManager::new(state.db_path.to_string_lossy().as_ref())?;

    // 패싯을 HashMap으로 변환
    let mut facets = HashMap::new();
    for (ns, values) in &classification.facets {
        facets.insert(ns.clone(), values.clone());
    }

    // 문서 인덱스 생성
    let doc_index = DocumentIndex {
        doc_id: doc_id.to_string(),
        repo: "local".to_string(),
        branch: "main".to_string(),
        path: save_path.to_string_lossy().to_string(),
        sha: "local".to_string(),
        content_hash: generate_doc_id(&req.filename, &req.content),
        title: req.filename.clone(),
        locale: "ko".to_string(),
        trust: 0.8,
        freshness: chrono::Utc::now().to_rfc3339(),
        source_type: "upload".to_string(),
        maturity: "stable".to_string(),
        facets,
        confidence: classification.confidence,
        warnings: classification.warnings.clone(),
        errors: classification.errors.clone(),
        ontology_version: "1.0".to_string(),
        rules_version: "1.0".to_string(),
    };

    // DB에 저장
    index.upsert_document(&doc_index)?;
    Ok(())
}

async fn api_list_documents(State(state): State<AppState>) -> impl IntoResponse {
    match load_documents_from_db(&state).await {
        Ok(documents) => (StatusCode::OK, Json(json!({ "documents": documents }))).into_response(),
        Err(e) => {
            tracing::error!("Failed to load documents: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::internal_error("Failed to load documents")),
            )
                .into_response()
        }
    }
}

async fn load_documents_from_db(
    state: &AppState,
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let conn = rusqlite::Connection::open(&state.db_path)?;
    let mut stmt = conn.prepare(
        "SELECT d.doc_id, d.title, d.path, d.valid_from, COALESCE(cl.confidence, 0.0) as confidence
         FROM docs d
         LEFT JOIN classify_log cl ON d.doc_id = cl.doc_id AND d.sha = cl.sha
         WHERE d.valid_to IS NULL AND d.deleted=0
         ORDER BY d.valid_from DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "doc_id": row.get::<_, String>(0)?,
            "title": row.get::<_, String>(1)?,
            "path": row.get::<_, String>(2)?,
            "uploaded_at": row.get::<_, String>(3)?,
            "confidence": row.get::<_, f32>(4)?
        }))
    })?;

    let mut documents = Vec::new();
    for row in rows {
        documents.push(row?);
    }

    Ok(documents)
}

async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let ontology_exists = state.ontology_path.exists();
    let rules_exists = state.rules_path.exists();

    let mut status = serde_json::Map::new();
    status.insert("status".into(), serde_json::Value::String("ok".into()));
    status.insert(
        "timestamp".into(),
        serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
    );
    status.insert(
        "ontology_file".into(),
        serde_json::Value::Bool(ontology_exists),
    );
    status.insert("rules_file".into(), serde_json::Value::Bool(rules_exists));

    let healthy = ontology_exists && rules_exists;

    if healthy {
        (StatusCode::OK, Json(serde_json::Value::Object(status))).into_response()
    } else {
        status.insert(
            "status".into(),
            serde_json::Value::String("degraded".into()),
        );
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::Value::Object(status)),
        )
            .into_response()
    }
}

async fn api_delete_document(
    State(state): State<AppState>,
    Path(doc_id_encoded): Path<String>,
) -> impl IntoResponse {
    // URL 디코딩 (간단한 방법)
    let doc_id = doc_id_encoded.replace("%2F", "/");
    let db_path = state.db_path.clone();
    let res: CtxResult<()> = tokio::task::spawn_blocking(move || {
        let mut conn =
            rusqlite::Connection::open(&db_path).map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        let tx = conn
            .transaction()
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        // fetch current file path
        let path: Option<String> = tx
            .query_row(
                "SELECT path FROM docs WHERE doc_id=?1 AND valid_to IS NULL AND deleted=0",
                [&doc_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        // mark deleted and close validity window
        tx.execute(
            "UPDATE docs SET deleted=1, valid_to=?2 WHERE doc_id=?1 AND valid_to IS NULL",
            (&doc_id, chrono::Utc::now().to_rfc3339()),
        )
        .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        tx.commit()
            .map_err(|e| ctx_core::ContextError::Other(e.to_string()))?;
        // try to remove file if exists (best-effort)
        if let Some(p) = path {
            let _ = std::fs::remove_file(p);
        }
        Ok(())
    })
    .await
    .unwrap_or_else(|e| Err(ctx_core::ContextError::Other(format!("join error: {e}"))));

    match res {
        Ok(()) => (StatusCode::NO_CONTENT, "").into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::internal_error(e.to_string())),
        )
            .into_response(),
    }
}

// ===========================
// Webhook Handlers
// ===========================

/// Webhook 토큰 검증
fn verify_webhook_token(headers: &HeaderMap) -> bool {
    let expected_token = std::env::var("CTX_HOOK_TOKEN").unwrap_or_default();
    if expected_token.is_empty() {
        tracing::warn!("CTX_HOOK_TOKEN not set, webhook authentication disabled");
        return true; // 토큰이 설정되지 않으면 허용 (개발 환경)
    }

    match headers.get("X-Auth-Token") {
        Some(token) => {
            let token_str = token.to_str().unwrap_or("");
            token_str == expected_token
        }
        None => false,
    }
}

/// Webhook 핸들러
async fn api_webhook_handler(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Json(event): Json<HookEvent>,
) -> impl IntoResponse {
    // 토큰 검증
    if !verify_webhook_token(&headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiError::new(
                "Invalid or missing X-Auth-Token",
                "UNAUTHORIZED",
            )),
        )
            .into_response();
    }

    // HookEvent에서 TaskData 직접 사용
    let task = event.task;

    // 분류 요청 생성
    let classify_req = ClassifyRequest {
        title: task.title.clone(),
        body: task.body.clone(),
        path: task.url.clone(),
    };

    // 분류 실행
    match classify_internal(&state, classify_req).await {
        Ok(classify_resp) => {
            let hook_resp = HookResponse {
                provider: provider.clone(),
                task_id: task.id.clone(),
                suggested_facets: classify_resp.facets,
                confidence: classify_resp.confidence,
                warnings: classify_resp.warnings,
                errors: classify_resp.errors,
            };

            tracing::info!(
                "Webhook processed: provider={}, task_id={}, confidence={}",
                provider,
                task.id,
                classify_resp.confidence
            );

            (StatusCode::OK, Json(hook_resp)).into_response()
        }
        Err(e) => {
            tracing::error!("Webhook classification failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::internal_error(format!(
                    "Classification failed: {}",
                    e
                ))),
            )
                .into_response()
        }
    }
}
async fn ui_compose_build(
    State(state): State<AppState>,
    Form(form): Form<ComposeBuilderForm>,
) -> impl IntoResponse {
    // 패싯 JSON 파싱
    let facets: BTreeMap<String, Vec<String>> = if let Some(facets_json) = &form.facets_json {
        serde_json::from_str(facets_json).unwrap_or_default()
    } else {
        BTreeMap::new()
    };

    // ComposeRequest 생성
    let compose_req = ComposeRequest {
        repo: form.repo,
        branch: form.branch,
        commit_sha: form.commit_sha,
        facets,
        lang: None,
        maturity: None,
        query_text: form.query_text,
        confidence_threshold: form.confidence_threshold,
        budget: form.budget,
        reserve: form.reserve,
    };

    match compose_internal(&state, compose_req).await {
        Ok(result) => {
            let confidence = result
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;
            let metadata = ComposeBuilderMetadata {
                tokens_used: result
                    .get("tokens_used")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                documents_selected: result
                    .get("source_documents")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.len())
                    .unwrap_or(0),
                confidence,
                confidence_percent: (confidence * 100.0) as u32,
                rejected_count: result
                    .get("rejected_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize,
                rationale: result
                    .get("rationale")
                    .and_then(|v| v.as_str())
                    .unwrap_or("선택 근거 없음")
                    .to_string(),
                source_documents: result
                    .get("source_documents")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
            };

            let prompt = result
                .get("merged_content")
                .and_then(|v| v.as_str())
                .unwrap_or("프롬프트 생성 실패")
                .to_string();

            let template = ComposeBuilderResultTemplate { prompt, metadata };
            match template.render() {
                Ok(html) => Html(html).into_response(),
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "<div>Template render error</div>".to_string(),
                )
                    .into_response(),
            }
        }
        Err(e) => {
            tracing::error!("Compose build failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("<div class='bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded'>오류: {}</div>", e),
            )
                .into_response()
        }
    }
}

async fn api_clear_database(State(state): State<AppState>) -> impl IntoResponse {
    let index = match SqliteIndexManager::new(state.db_path.to_string_lossy().as_ref()) {
        Ok(index) => index,
        Err(e) => {
            tracing::error!("Failed to create index manager: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::internal_error("Database connection failed")),
            )
                .into_response();
        }
    };

    match index.clear_all_data() {
        Ok(()) => {
            tracing::warn!("Database cleared - all data deleted");
            (
                StatusCode::OK,
                Json(json!({"success": true, "message": "All data cleared successfully"})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to clear database: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::internal_error("Failed to clear database")),
            )
                .into_response()
        }
    }
}
