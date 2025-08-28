//! MCP (Model Context Protocol) Server Implementation
//!
//! stdio 기반 JSON-RPC 서버로 LLM 에이전트가 분류/조합 기능을 사용할 수 있도록 합니다.

use ctx_core::core::classifier::service::ClassifierService;
use ctx_core::data::index::sqlite::SqliteIndexManager;
use ctx_core::{BuildComposer, BuildQuery, Result as CtxResult};
use crate::app::config::AppConfig;
use std::env;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Write};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcError {
    fn parse_error() -> Self {
        Self {
            code: -32700,
            message: "Parse error".to_string(),
            data: None,
        }
    }

    fn method_not_found() -> Self {
        Self {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }
    }

    fn invalid_params(msg: String) -> Self {
        Self {
            code: -32602,
            message: "Invalid params".to_string(),
            data: Some(json!({"details": msg})),
        }
    }

    fn internal_error(msg: String) -> Self {
        Self {
            code: -32603,
            message: "Internal error".to_string(),
            data: Some(json!({"details": msg})),
        }
    }
}

pub fn run_mcp() -> CtxResult<()> {
    // MCP 서버는 stdio를 통해 통신하므로 stderr로만 로깅
    // 프로덕션에서는 이 로그도 제거하거나 환경변수로 제어 가능
    if std::env::var("MCP_DEBUG").is_ok() {
        eprintln!("Starting MCP server on stdio...");
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = line.map_err(ctx_core::ContextError::Io)?;

        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => handle_request(request),
            Err(_) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: None,
                error: Some(JsonRpcError::parse_error()),
            },
        };

        let response_json = serde_json::to_string(&response)
            .map_err(|e| ctx_core::ContextError::Other(format!("JSON serialization error: {}", e)))?;

        writeln!(stdout, "{}", response_json).map_err(ctx_core::ContextError::Io)?;
        stdout.flush().map_err(ctx_core::ContextError::Io)?;
    }

    Ok(())
}

/// Resolve ontology/rules/db paths with priority:
/// 1) ENV (CTX_ONTOLOGY/CTX_RULES/CTX_DB)
/// 2) ctxset.toml (AppConfig)
/// 3) CWD defaults
fn resolve_paths() -> (PathBuf, PathBuf, PathBuf) {
    let cfg = AppConfig::load_from_current_dir();

    let ontology = env::var_os("CTX_ONTOLOGY")
        .map(PathBuf::from)
        .or_else(|| cfg.as_ref().and_then(|c| c.ontology.clone()))
        .unwrap_or_else(|| PathBuf::from("ontology.yaml"));

    let rules = env::var_os("CTX_RULES")
        .map(PathBuf::from)
        .or_else(|| cfg.as_ref().and_then(|c| c.rules.clone()))
        .unwrap_or_else(|| PathBuf::from("rules.yaml"));

    let db = env::var_os("CTX_DB")
        .map(PathBuf::from)
        .or_else(|| cfg.as_ref().and_then(|c| c.db.clone()))
        .unwrap_or_else(|| PathBuf::from("ctxindex.db"));

    (ontology, rules, db)
}

fn handle_request(request: JsonRpcRequest) -> JsonRpcResponse {
    let id = request.id.clone();

    match request.method.as_str() {
        "ping" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({"status": "ok", "service": "ctxset-mcp"})),
            error: None,
        },

        "classifyText" => handle_classify_text(id, request.params),

        "composePrompt" => handle_compose_prompt(id, request.params),

        "listContexts" => handle_list_contexts(id, request.params),

        "getContext" => handle_get_context(id, request.params),

        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError::method_not_found()),
        },
    }
}

fn handle_classify_text(id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
    let params = match params {
        Some(p) => p,
        None => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError::invalid_params(
                    "Missing parameters".to_string(),
                )),
            };
        }
    };

    let title = params
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let body = params
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if title.is_empty() && body.is_empty() {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError::invalid_params(
                "title or body required".to_string(),
            )),
        };
    }

    match classify_text_internal(&title, &body) {
        Ok(result) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        },
        Err(e) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError::internal_error(e.to_string())),
        },
    }
}

fn handle_compose_prompt(id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
    let params = match params {
        Some(p) => p,
        None => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError::invalid_params(
                    "Missing parameters".to_string(),
                )),
            };
        }
    };

    // 필수 파라미터 추출
    let repo = params
        .get("repo")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    let branch = params
        .get("branch")
        .and_then(|v| v.as_str())
        .unwrap_or("main")
        .to_string();

    let commit_sha = params
        .get("commit_sha")
        .and_then(|v| v.as_str())
        .unwrap_or("HEAD")
        .to_string();

    // facets 파라미터 파싱
    let facets = params
        .get("facets")
        .and_then(|v| v.as_object())
        .map(|obj| {
            let mut facets_map = BTreeMap::new();
            for (key, value) in obj {
                if let Some(arr) = value.as_array() {
                    let values: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();
                    facets_map.insert(key.clone(), values);
                }
            }
            facets_map
        })
        .unwrap_or_default();

    // 파라미터 기본 검증
    if repo.trim().is_empty() || branch.trim().is_empty() || commit_sha.trim().is_empty() {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError::invalid_params(
                "repo, branch, and commit_sha are required".into(),
            )),
        };
    }
    if commit_sha.len() < 6 || !commit_sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError::invalid_params(
                "commit_sha must be hex and >= 6 chars".into(),
            )),
        };
    }

    // facets 구조 검증
    for (k, v) in &facets {
        if k.trim().is_empty() {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError::invalid_params(
                    "facet namespace must not be empty".into(),
                )),
            };
        }
        if v.is_empty() {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError::invalid_params(format!(
                    "facet '{}' must have values",
                    k
                ))),
            };
        }
    }

    // 예산 파라미터 검증
    if let Some(b) = params
        .get("budget")
        .and_then(|v| v.as_u64())
        .map(|u| u as usize)
    {
        let min_b = ctx_core::common::constants::composition::MIN_TOKEN_BUDGET;
        let max_b = ctx_core::common::constants::composition::MAX_TOKEN_BUDGET;
        if b < min_b || b > max_b {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError::invalid_params(format!(
                    "budget must be in [{}..={}]",
                    min_b, max_b
                ))),
            };
        }
    }
    if let (Some(b), Some(r)) = (
        params
            .get("budget")
            .and_then(|v| v.as_u64())
            .map(|u| u as usize),
        params
            .get("reserve")
            .and_then(|v| v.as_u64())
            .map(|u| u as usize),
    ) {
        if r >= b {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError::invalid_params(
                    "reserve must be less than budget".into(),
                )),
            };
        }
    }
    if let Some(ct) = params.get("confidence_threshold").and_then(|v| v.as_f64()) {
        if !(0.0..=1.0).contains(&ct) {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError::invalid_params(
                    "confidence_threshold must be in [0,1]".into(),
                )),
            };
        }
    }

    match compose_prompt_internal(repo, branch, commit_sha, facets, &params) {
        Ok(result) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        },
        Err(e) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError::internal_error(e.to_string())),
        },
    }
}

fn classify_text_internal(title: &str, body: &str) -> CtxResult<Value> {
    // 온톨로지/룰 경로 해석 (ENV/TOML/CWD 순)
    let (ontology_path, rules_path, _db_path) = resolve_paths();

    // 온톨로지와 룰 파일 존재 확인
    if !ontology_path.exists() {
        return Err(ctx_core::ContextError::Other(
            "ontology.yaml not found in current directory".to_string(),
        ));
    }
    if !rules_path.exists() {
        return Err(ctx_core::ContextError::Other(
            "rules.yaml not found in current directory".to_string(),
        ));
    }

    // 온톨로지와 룰 로드
    let ontology_yaml = std::fs::read_to_string(&ontology_path).map_err(ctx_core::ContextError::Io)?;
    let rules_yaml = std::fs::read_to_string(&rules_path).map_err(ctx_core::ContextError::Io)?;

    // 분류 실행
    let result =
        ClassifierService::classify_with_yaml(&ontology_yaml, Some(&rules_yaml), title, body)?;

    // 결과를 정렬된 맵으로 변환
    let mut facets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (ns, vals) in result.facets().to_hashmap() {
        let mut v: Vec<_> = vals.into_iter().collect();
        v.sort();
        facets.insert(ns, v);
    }

    Ok(json!({
        "facets": facets,
        "confidence": result.confidence(),
        "warnings": result.warnings(),
        "errors": result.errors()
    }))
}

fn compose_prompt_internal(
    repo: String,
    branch: String,
    commit_sha: String,
    facets: BTreeMap<String, Vec<String>>,
    params: &Value,
) -> CtxResult<Value> {
    // 쿼리 빌드
    let mut query = BuildQuery::new(repo, branch, commit_sha);
    for (ns, vals) in &facets {
        query = query.with_facet(ns.clone(), vals.clone());
    }

    if let Some(lang) = params.get("lang").and_then(|v| v.as_str()) {
        query = query.with_language(lang.to_string());
    }
    if let Some(maturity) = params.get("maturity").and_then(|v| v.as_str()) {
        query = query.with_maturity(maturity.to_string());
    }
    if let Some(query_text) = params.get("query_text").and_then(|v| v.as_str()) {
        query = query.with_query_text(query_text.to_string());
    }
    if let Some(threshold) = params.get("confidence_threshold").and_then(|v| v.as_f64()) {
        query = query.with_confidence_threshold(threshold as f32);
    }

    // SQLite 데이터베이스 파일 존재 확인 (ENV/TOML/CWD 순)
    let (_onto_path, _rules_path, db_path) = resolve_paths();
    if !db_path.exists() {
        return Err(ctx_core::ContextError::Other(
            "ctxindex.db not found in current directory. Run 'ctxset index-repo' first."
                .to_string(),
        ));
    }

    // SQLite 인덱스 매니저 생성
    let index = SqliteIndexManager::new(db_path.to_string_lossy().as_ref())?;
    let candidates = index.load_candidates(&query)?;

    // 조합 실행
    let composer = BuildComposer::new();
    let budget = params
        .get("budget")
        .and_then(|v| v.as_u64())
        .map(|u| u as usize)
        .unwrap_or(2000);
    let reserve = params
        .get("reserve")
        .and_then(|v| v.as_u64())
        .map(|u| u as usize)
        .unwrap_or(200);

    let result = composer?.compose(candidates, &query, budget, reserve)?;

    Ok(json!({
        "merged_content": result.merged_document.content,
        "source_documents": result.merged_document.source_documents,
        "rationale": result.selection_rationale,
        "tokens_used": result.total_tokens_used,
        "confidence": result.confidence_score,
        "rejected_count": result.rejected_documents.len()
    }))
}

fn handle_list_contexts(id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
    // 파라미터 파싱
    let limit = params
        .as_ref()
        .and_then(|p| p.get("limit"))
        .and_then(|v| v.as_u64())
        .unwrap_or(50) as usize;

    let offset = params
        .as_ref()
        .and_then(|p| p.get("offset"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    match list_contexts_internal(limit, offset) {
        Ok(result) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        },
        Err(e) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError::internal_error(e.to_string())),
        },
    }
}

fn handle_get_context(id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
    let params = match params {
        Some(p) => p,
        None => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError::invalid_params(
                    "Missing parameters".to_string(),
                )),
            };
        }
    };

    let doc_id = match params.get("doc_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError::invalid_params(
                    "doc_id parameter required".to_string(),
                )),
            };
        }
    };

    match get_context_internal(doc_id) {
        Ok(result) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        },
        Err(e) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError::internal_error(e.to_string())),
        },
    }
}

fn list_contexts_internal(limit: usize, offset: usize) -> CtxResult<Value> {
    // SQLite 데이터베이스 파일 존재 확인 (ENV/TOML/CWD 순)
    let (_onto_path, _rules_path, db_path) = resolve_paths();
    if !db_path.exists() {
        return Err(ctx_core::ContextError::Other(
            "ctxindex.db not found in current directory. Run 'ctxset index-repo' first."
                .to_string(),
        ));
    }

    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| ctx_core::ContextError::Other(format!("Database connection failed: {}", e)))?;

    // 전체 개수 조회
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM docs WHERE valid_to IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // 문서 목록 조회
    let mut stmt = conn
        .prepare(
            "SELECT doc_id, title, path, valid_from, 
                    COALESCE((SELECT confidence FROM classify_log cl WHERE cl.doc_id = d.doc_id AND cl.sha = d.sha LIMIT 1), 0.0) as confidence
             FROM docs d 
             WHERE valid_to IS NULL 
             ORDER BY valid_from DESC 
             LIMIT ? OFFSET ?"
        )
        .map_err(|e| ctx_core::ContextError::Other(format!("Query preparation failed: {}", e)))?;

    let rows = stmt
        .query_map([limit as i64, offset as i64], |row| {
            Ok(json!({
                "doc_id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "path": row.get::<_, String>(2)?,
                "uploaded_at": row.get::<_, String>(3)?,
                "confidence": row.get::<_, f64>(4)?
            }))
        })
        .map_err(|e| ctx_core::ContextError::Other(format!("Query execution failed: {}", e)))?;

    let mut contexts = Vec::new();
    for row in rows {
        contexts
            .push(row.map_err(|e| ctx_core::ContextError::Other(format!("Row processing failed: {}", e)))?);
    }

    Ok(json!({
        "contexts": contexts,
        "total": total,
        "limit": limit,
        "offset": offset,
        "has_more": (offset as i64 + limit as i64) < total
    }))
}

fn get_context_internal(doc_id: &str) -> CtxResult<Value> {
    // SQLite 데이터베이스 파일 존재 확인
    if !std::path::Path::new("ctxindex.db").exists() {
        return Err(ctx_core::ContextError::Other(
            "ctxindex.db not found in current directory. Run 'ctxset index-repo' first."
                .to_string(),
        ));
    }

    let conn = rusqlite::Connection::open("ctxindex.db")
        .map_err(|e| ctx_core::ContextError::Other(format!("Database connection failed: {}", e)))?;

    // 문서 기본 정보 조회
    let (title, path, uploaded_at, confidence): (String, String, String, f64) = conn
        .query_row(
            "SELECT d.title, d.path, d.valid_from,
                    COALESCE((SELECT confidence FROM classify_log cl WHERE cl.doc_id = d.doc_id AND cl.sha = d.sha LIMIT 1), 0.0) as confidence
             FROM docs d 
             WHERE d.doc_id = ? AND d.valid_to IS NULL",
            [doc_id],
            |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?
            ))
        )
        .map_err(|e| ctx_core::ContextError::Other(format!("Document not found: {}", e)))?;

    // 패싯 정보 조회
    let mut facet_stmt = conn
        .prepare(
            "SELECT namespace, value 
             FROM facet_values fv
             JOIN docs d ON fv.doc_id = d.doc_id AND fv.sha = d.sha
             WHERE d.doc_id = ? AND d.valid_to IS NULL
             ORDER BY namespace, value",
        )
        .map_err(|e| ctx_core::ContextError::Other(format!("Facet query preparation failed: {}", e)))?;

    let facet_rows = facet_stmt
        .query_map([doc_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| ctx_core::ContextError::Other(format!("Facet query execution failed: {}", e)))?;

    let mut facets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in facet_rows {
        let (namespace, value) =
            row.map_err(|e| ctx_core::ContextError::Other(format!("Facet row processing failed: {}", e)))?;
        facets.entry(namespace).or_default().push(value);
    }

    // 파일 내용 읽기
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| format!("파일을 읽을 수 없습니다: {}", path));

    Ok(json!({
        "doc_id": doc_id,
        "title": title,
        "path": path,
        "uploaded_at": uploaded_at,
        "confidence": confidence,
        "facets": facets,
        "content": content
    }))
}
