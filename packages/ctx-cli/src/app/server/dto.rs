use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 공통 API 에러 응답
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<String>>,
}

impl ApiError {
    pub fn new(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: code.into(),
            details: None,
            suggestions: None,
        }
    }
    pub fn internal_error(error: impl Into<String>) -> Self {
        Self::new(error, "INTERNAL_ERROR")
    }
    pub fn validation_error(error: impl Into<String>) -> Self {
        Self::new(error, "VALIDATION_ERROR")
    }
}

impl From<ctx_core::ContextError> for ApiError {
    fn from(err: ctx_core::ContextError) -> Self {
        let msg = err.user_friendly_message();
        match err {
            ctx_core::ContextError::TokenBudgetExceeded { budget, excess } => ApiError {
                error: msg,
                code: "TOKEN_BUDGET_EXCEEDED".into(),
                details: Some(serde_json::json!({ "budget": budget, "excess": excess })),
                suggestions: Some(vec!["Increase budget".into(), "Lower content size".into()]),
            },
            _ => ApiError::internal_error(msg),
        }
    }
}

/* ===========================
Requests
=========================== */

/// 분류 요청(JSON/폼 공용)
#[derive(Debug, Deserialize)]
pub struct ClassifyRequest {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub path: Option<String>,
}

/// 검증 요청(JSON)
#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    pub facets: BTreeMap<String, Vec<String>>,
}

/// 조합 요청(JSON)
#[derive(Debug, Deserialize)]
pub struct ComposeRequest {
    pub repo: String,
    pub branch: String,
    pub commit_sha: String,
    pub facets: BTreeMap<String, Vec<String>>,

    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub maturity: Option<String>,
    #[serde(default)]
    pub query_text: Option<String>,
    #[serde(default)]
    pub confidence_threshold: Option<f32>,
    #[serde(default)]
    pub budget: Option<usize>,
    #[serde(default)]
    pub reserve: Option<usize>,
}

/// 텍스트 PUT(JSON)
#[derive(Debug, Deserialize)]
pub struct PutTextRequest {
    pub text: String,
}

/// 파일 업로드 요청
#[derive(Debug, Deserialize)]
pub struct FileUploadRequest {
    pub filename: String,
    pub content: String,
    #[serde(default)]
    pub path: Option<String>,
}

/// 파일 업로드 응답
#[derive(Debug, Serialize)]
pub struct FileUploadResponse {
    pub success: bool,
    pub filename: String,
    pub doc_id: String,
    pub classification: ClassifyResponse,
    pub saved_path: String,
}

/// Webhook 이벤트 요청
#[derive(Debug, Deserialize)]
pub struct HookEvent {
    pub task: TaskData,
}

/// 태스크 데이터 (provider 공통)
#[derive(Debug, Deserialize)]
pub struct TaskData {
    pub id: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub assignees: Vec<String>,
}

/// Webhook 응답
#[derive(Debug, Serialize)]
pub struct HookResponse {
    pub provider: String,
    pub task_id: String,
    pub suggested_facets: BTreeMap<String, Vec<String>>,
    pub confidence: f32,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

/* ===========================
Responses
=========================== */

/// 분류 응답
#[derive(Debug, Serialize, Deserialize)]
pub struct ClassifyResponse {
    pub facets: BTreeMap<String, Vec<String>>,
    pub confidence: f32,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// 조합 응답(SSR 템플릿에서 사용)
#[derive(Debug, Serialize)]
pub struct ComposeResponse {
    pub prompt: String,
    #[serde(default)]
    pub trace_id: Option<String>,
    pub metadata: ComposeMetadata,
}

/// 조합 메타데이터(SSR 템플릿에서 사용)
#[derive(Debug, Serialize)]
pub struct ComposeMetadata {
    pub tokens_used: u32,
    pub documents_selected: usize,
    pub selection_method: String,
}

/// 일반 성공 응답
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

/* ===========================
Dashboard DTOs (SSR)
=========================== */

#[derive(Debug, Serialize)]
pub struct RecentOperation {
    pub id: String,
    /// "classify" | "compose"
    pub operation_type: String,
    pub title: String,
    pub timestamp: String,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub total_classifications: usize,
    pub total_compositions: usize,
    pub avg_confidence: f32,
    pub active_namespaces: usize,
}

#[derive(Debug, Serialize)]
pub struct DashboardData {
    pub recent_operations: Vec<RecentOperation>,
    pub stats: DashboardStats,
}

/* ===========================
DB Dashboard DTOs
=========================== */

#[derive(Serialize)]
pub struct DbTableInfo {
    pub name: String,
    pub rows: u64,
}

#[derive(Serialize)]
pub struct DbTableData {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>, // normalized as strings for template
    pub limit: u32,
    pub offset: u32,
    pub total: u64,
}

#[derive(Deserialize)]
pub struct DbQueryForm {
    pub sql: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct DbQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub elapsed_ms: u128,
}

// Webhook DTOs are defined above (HookEvent with TaskData, HookResponse)

/* ===========================
Context DTOs (SSR)
=========================== */

#[derive(Debug, Serialize)]
pub struct DocSummary {
    pub doc_id: String,
    pub title: String,
    pub path: String,
    pub uploaded_at: String,
    pub confidence: f32,
}

#[derive(Debug, Serialize)]
pub struct DocDetail {
    pub doc_id: String,
    pub title: String,
    pub path: String,
    pub confidence: f32,
    pub facets: Vec<(String, String)>,
    pub content: String,
}
