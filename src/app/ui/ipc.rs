//! IPC signatures and data types for Tauri UI (feature = "ui_ipc").
//!
//! This module exposes stable request/response types and functions for:
//! - Workspace selection and reindex
//! - Document listing/reading/updating (atomic save)
//! - Ontology/Rules viewing/updating
//! - Compose/classify entry points backed by core services
//! - Settings get/set
//!
//! Rendering policy (P1): HTML is produced server-side for security/determinism (comrak+syntect+ammonia).
//! At this stage we provide a plain HTML field; renderer integration can be added under a separate feature.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use once_cell::sync::Lazy;
use tokio::sync::RwLock;

use crate::domain::errors::{ContextError, Result};
use crate::domain::types::PromptBundle;
use crate::drivers::storage::{local::LocalFsStorage, Storage};
use crate::services;
use serde_json::json;

static UI_STATE: Lazy<RwLock<UiState>> = Lazy::new(|| RwLock::new(UiState::default()));

#[derive(Debug, Default)]
struct UiState {
    workspace_root: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceSummary {
    pub root: String,
    pub doc_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexSummary {
    pub total: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocMeta {
    pub id: String,
    pub path: String,
    pub tags: Vec<String>,
    pub sections: usize,
    pub mtime: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocReadResult {
    pub frontmatter: crate::domain::types::ContextDocument,
    pub sections: Vec<crate::domain::types::SectionDef>,
    pub html: String,
    pub raw_markdown: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SaveResult {
    pub ok: bool,
    pub updated: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

pub fn to_ui_error(err: ContextError) -> UiError {
    match err {
        ContextError::ParseError { line, message } => UiError { code: "E_PARSE_ERROR".into(), message, data: Some(json!({"line": line})) },
        ContextError::InvalidFrontmatter(msg) => UiError { code: "E_INVALID_INPUT".into(), message: msg, data: None },
        ContextError::MarkerNotFound { marker } => UiError { code: "E_NOT_FOUND".into(), message: format!("Marker '{}' not found", marker), data: None },
        ContextError::SectionNotFound { section_id, available } => UiError { code: "E_NOT_FOUND".into(), message: format!("Section '{}' not found", section_id), data: Some(json!({"available": available})) },
        ContextError::TokenBudgetExceeded { budget, excess } => UiError { code: "E_BUDGET_EXCEEDED".into(), message: format!("Budget {} exceeded by {}", budget, excess), data: Some(json!({"budget": budget, "excess": excess})) },
        ContextError::ConfigError(msg) => UiError { code: "E_CONFIG".into(), message: msg, data: None },
        ContextError::Io(e) => UiError { code: "E_IO".into(), message: e.to_string(), data: None },
        ContextError::YamlError(e) => UiError { code: "E_SERIALIZE".into(), message: e.to_string(), data: None },
        ContextError::JsonError(e) => UiError { code: "E_SERIALIZE".into(), message: e.to_string(), data: None },
        ContextError::ServerError(msg) => UiError { code: "E_INTERNAL".into(), message: msg, data: None },
        ContextError::ClassificationError(msg) => UiError { code: "E_CLASSIFY".into(), message: msg, data: None },
        ContextError::CompositionError(msg) => UiError { code: "E_COMPOSE".into(), message: msg, data: None },
        ContextError::DatabaseError(e) => UiError { code: "E_DB".into(), message: e.to_string(), data: None },
        ContextError::OptimizationError(msg) => UiError { code: "E_OPTIMIZE".into(), message: msg, data: None },
        ContextError::AssemblyError(msg) => UiError { code: "E_ASSEMBLY".into(), message: msg, data: None },
        ContextError::Other(msg) => UiError { code: "E_INTERNAL".into(), message: msg, data: None },
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ComposeRequest {
    pub query: String,
    pub sections: Option<Vec<String>>,
    pub budget: Option<u32>,
    pub lambda: Option<f32>,
    pub facets: Option<std::collections::HashMap<String, Vec<String>>>,
    pub template: Option<String>,
}

async fn resolve_workspace_root() -> Result<PathBuf> {
    let state = UI_STATE.read().await;
    let Some(root) = state.workspace_root.clone() else {
        return Err(ContextError::ConfigError("workspace not selected".into()));
    };
    Ok(root)
}

pub async fn select_workspace(path: String) -> Result<WorkspaceSummary> {
    let root = PathBuf::from(path.clone());
    if !root.exists() || !root.is_dir() {
        return Err(ContextError::ConfigError("invalid workspace root".into()));
    }
    {
        let mut state = UI_STATE.write().await;
        state.workspace_root = Some(root.clone());
    }
    let storage = LocalFsStorage::new(root.clone());
    let files = storage.list_files(Path::new("."), "*.md")?;
    Ok(WorkspaceSummary { root: path, doc_count: files.len() })
}

pub async fn reindex() -> Result<IndexSummary> {
    let root = resolve_workspace_root().await?;
    let storage = LocalFsStorage::new(root);
    let files = storage.list_files(Path::new("."), "*.md")?;
    Ok(IndexSummary { total: files.len(), errors: 0 })
}

pub async fn list_documents(_filter: Option<String>) -> Result<Vec<DocMeta>> {
    let root = resolve_workspace_root().await?;
    let storage = LocalFsStorage::new(root.clone());
    let files = storage.list_files(Path::new("."), "*.md")?;
    let mut out = Vec::new();
    let parser = crate::doc::parse::DocumentParser::new();
    for p in files {
        let doc = parser.parse_file(&storage, &p).unwrap_or_default();
        let meta = DocMeta {
            id: if doc.id.is_empty() { p.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string() } else { doc.id },
            path: p.to_string_lossy().to_string(),
            tags: doc.tags,
            sections: doc.sections.len(),
            mtime: std::fs::metadata(root.join(&p)).ok().and_then(|m| m.modified().ok()).and_then(to_unix_ts),
        };
        out.push(meta);
    }
    Ok(out)
}

fn to_unix_ts(t: SystemTime) -> Option<i64> { t.duration_since(SystemTime::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64) }

pub async fn read_document(id_or_relpath: String) -> Result<DocReadResult> {
    let root = resolve_workspace_root().await?;
    let storage = LocalFsStorage::new(root.clone());
    let path = PathBuf::from(&id_or_relpath);
    let target = if path.extension().is_some() { path } else { PathBuf::from(format!("{}.md", id_or_relpath)) };
    let parser = crate::doc::parse::DocumentParser::new();
    let (doc, body) = parser.parse_file_with_body(&storage, &target)?;
    // P1: Render HTML on Rust-side for security & determinism when enabled.
    #[cfg(feature = "ui_render_rust")]
    let html = crate::app::ui::render::render_markdown_to_safe_html(&body);
    #[cfg(not(feature = "ui_render_rust"))]
    let html = html_escape::encode_text(&body).to_string();
    let raw_markdown = std::fs::read_to_string(root.join(&target)).map_err(ContextError::Io)?;
    Ok(DocReadResult { frontmatter: doc.clone(), sections: doc.sections.clone(), html, raw_markdown })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum UpdatePayload {
    Full { content: String },
    Patch { find: String, replace: String },
}

pub async fn update_document(id_or_relpath: String, payload: UpdatePayload) -> Result<SaveResult> {
    let root = resolve_workspace_root().await?;
    let _storage = LocalFsStorage::new(root.clone());
    let path = PathBuf::from(&id_or_relpath);
    let target = if path.extension().is_some() { root.join(path) } else { root.join(format!("{}.md", id_or_relpath)) };
    let orig = std::fs::read_to_string(&target).map_err(ContextError::Io)?;
    let next = match payload {
        UpdatePayload::Full { content } => content,
        UpdatePayload::Patch { find, replace } => orig.replace(&find, &replace),
    };
    atomic_write(&target, next.as_bytes())?;
    Ok(SaveResult { ok: true, updated: true, message: None })
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    use std::fs::{self, File};
    use std::io::Write;
    let parent = path.parent().ok_or_else(|| ContextError::ConfigError("invalid path".into()))?;
    let tmp = parent.join(format!(".{}.tmp", path.file_name().and_then(|s| s.to_str()).unwrap_or("file")));
    {
        let mut f = File::create(&tmp).map_err(ContextError::Io)?;
        f.write_all(data).map_err(ContextError::Io)?;
        f.sync_all().map_err(ContextError::Io)?;
    }
    fs::rename(&tmp, path).map_err(ContextError::Io)?;
    Ok(())
}

pub async fn read_ontology() -> Result<String> {
    let root = resolve_workspace_root().await?;
    let path = root.join("knowledge/ontology.yaml");
    std::fs::read_to_string(path).map_err(ContextError::Io)
}

pub async fn update_ontology(content: String) -> Result<SaveResult> {
    let root = resolve_workspace_root().await?;
    let path = root.join("knowledge/ontology.yaml");
    atomic_write(&path, content.as_bytes())?;
    Ok(SaveResult { ok: true, updated: true, message: None })
}

pub async fn read_rules() -> Result<String> {
    let root = resolve_workspace_root().await?;
    let path = root.join("knowledge/rules.yaml");
    std::fs::read_to_string(path).map_err(ContextError::Io)
}

pub async fn update_rules(content: String) -> Result<SaveResult> {
    let root = resolve_workspace_root().await?;
    let path = root.join("knowledge/rules.yaml");
    atomic_write(&path, content.as_bytes())?;
    Ok(SaveResult { ok: true, updated: true, message: None })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Settings {
    pub theme: Option<String>,
    pub auto_reindex: Option<String>,
    pub default_budget: Option<u32>,
    pub default_lambda: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_paths: Option<Vec<String>>, // Absolute paths under workspace root
}

pub async fn get_settings() -> Result<Settings> {
    load_ui_settings().await
}

pub async fn set_settings(_partial: Settings) -> Result<Settings> {
    // Merge partial with existing, validate watch_paths scope, then persist
    let mut current = load_ui_settings().await.unwrap_or_default();
    if let Some(v) = _partial.theme { current.theme = Some(v); }
    if let Some(v) = _partial.auto_reindex { current.auto_reindex = Some(v); }
    if let Some(v) = _partial.default_budget { current.default_budget = Some(v); }
    if let Some(v) = _partial.default_lambda { current.default_lambda = Some(v); }
    if let Some(paths) = _partial.watch_paths {
        validate_watch_paths(&paths).await?;
        current.watch_paths = Some(normalize_paths(paths).await?);
    }
    save_ui_settings(&current).await?;
    Ok(current)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClassifyInput { pub text: String }

pub async fn classify(input: ClassifyInput) -> Result<Vec<String>> {
    services::classify_text(&input.text)
}

pub async fn compose(req: ComposeRequest) -> Result<PromptBundle> {
    let mut input = crate::domain::types::ComposeInput::new(req.query);
    if let Some(b) = req.budget { input = input.with_max_tokens(b); }
    if let Some(sections) = req.sections { let _ = sections; /* reserved for P2 */ }
    services::compose_prompt(input)
}

async fn settings_path() -> Result<std::path::PathBuf> {
    let root = resolve_workspace_root().await?;
    let dir = root.join(".ctx");
    std::fs::create_dir_all(&dir).map_err(ContextError::Io)?;
    Ok(dir.join("ui-settings.json"))
}

async fn load_ui_settings() -> Result<Settings> {
    let p = settings_path().await?;
    if !p.exists() { return Ok(Settings::default()); }
    let s = std::fs::read_to_string(&p).map_err(ContextError::Io)?;
    let cfg: Settings = serde_json::from_str(&s).map_err(ContextError::JsonError)?;
    Ok(cfg)
}

async fn save_ui_settings(s: &Settings) -> Result<()> {
    let p = settings_path().await?;
    let data = serde_json::to_vec_pretty(s).map_err(ContextError::JsonError)?;
    atomic_write(&p, &data)
}

async fn validate_watch_paths(paths: &Vec<String>) -> Result<()> {
    let root = resolve_workspace_root().await?;
    for p in paths {
        let pb = std::path::PathBuf::from(p);
        if !pb.exists() || !pb.is_dir() {
            return Err(ContextError::ConfigError(format!("invalid watch path: {}", p)));
        }
        let abs = if pb.is_absolute() { pb.clone() } else { root.join(&pb) };
        let abs = std::fs::canonicalize(&abs).map_err(ContextError::Io)?;
        let root_abs = std::fs::canonicalize(&root).map_err(ContextError::Io)?;
        if !abs.starts_with(&root_abs) {
            return Err(ContextError::ConfigError(format!("watch path must be under workspace root: {}", abs.display())));
        }
    }
    Ok(())
}

async fn normalize_paths(paths: Vec<String>) -> Result<Vec<String>> {
    let root = resolve_workspace_root().await?;
    let mut out = Vec::new();
    for p in paths {
        let pb = std::path::PathBuf::from(&p);
        let abs = if pb.is_absolute() { pb } else { root.join(pb) };
        let can = std::fs::canonicalize(&abs).map_err(ContextError::Io)?;
        out.push(can.to_string_lossy().to_string());
    }
    out.sort();
    out.dedup();
    Ok(out)
}
