//! Tauri v2 application bootstrap and command bindings.
//! Follows RULES: clear names, single-responsibility handlers, secure defaults.

// use tauri::Manager; // not used currently
use tauri::{Wry, AppHandle, Emitter};
use std::sync::Mutex;
use std::collections::HashMap;
use once_cell::sync::Lazy;
use notify::{RecommendedWatcher, Config, Event, RecursiveMode, Watcher};
use serde_json::json;

// Re-exported IPC types for Tauri commands' signatures
use crate::app::ui::ipc::{
    ClassifyInput, ComposeRequest, DocReadResult, DocMeta, IndexSummary, SaveResult, Settings,
    WorkspaceSummary, UiError, to_ui_error,
};

// Workspace
#[tauri::command]
async fn select_workspace(path: String, app: AppHandle<Wry>) -> Result<WorkspaceSummary, UiError> {
    let ws = crate::app::ui::ipc::select_workspace(path.clone()).await.map_err(to_ui_error)?;
    // 기본 감시 경로: documents/ (없으면 루트)
    let root = std::path::PathBuf::from(&ws.root);
    let docs = root.join("documents");
    let watch_base = if docs.exists() { docs } else { root };
    set_watch_paths_internal(&app, vec![watch_base])
        .map_err(|e| UiError { code: "E_WATCH".into(), message: e, data: None })?;
    Ok(ws)
}

#[tauri::command]
async fn reindex() -> Result<IndexSummary, UiError> {
    crate::app::ui::ipc::reindex().await.map_err(to_ui_error)
}

// Docs
#[tauri::command]
async fn list_documents(filter: Option<String>) -> Result<Vec<DocMeta>, UiError> {
    crate::app::ui::ipc::list_documents(filter).await.map_err(to_ui_error)
}

#[tauri::command]
async fn read_document(id: String) -> Result<DocReadResult, UiError> {
    crate::app::ui::ipc::read_document(id).await.map_err(to_ui_error)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct UpdateDocumentPayload {
    id: String,
    payload: crate::app::ui::ipc::UpdatePayload,
}

#[tauri::command(rename_all = "snake_case")]
async fn update_document(input: UpdateDocumentPayload) -> Result<SaveResult, UiError> {
    crate::app::ui::ipc::update_document(input.id, input.payload).await.map_err(to_ui_error)
}

// Knowledge
#[tauri::command]
async fn read_ontology() -> Result<String, UiError> {
    crate::app::ui::ipc::read_ontology().await.map_err(to_ui_error)
}

#[tauri::command]
async fn update_ontology(content: String) -> Result<SaveResult, UiError> {
    crate::app::ui::ipc::update_ontology(content).await.map_err(to_ui_error)
}

#[tauri::command]
async fn read_rules() -> Result<String, UiError> {
    crate::app::ui::ipc::read_rules().await.map_err(to_ui_error)
}

#[tauri::command]
async fn update_rules(content: String) -> Result<SaveResult, UiError> {
    crate::app::ui::ipc::update_rules(content).await.map_err(to_ui_error)
}

// Settings
#[tauri::command]
async fn get_settings() -> Result<Settings, UiError> {
    crate::app::ui::ipc::get_settings().await.map_err(to_ui_error)
}

#[tauri::command]
async fn set_settings(partial: Settings) -> Result<Settings, UiError> {
    crate::app::ui::ipc::set_settings(partial).await.map_err(to_ui_error)
}

// Compose / Classify
#[tauri::command]
async fn compose(req: ComposeRequest) -> Result<crate::domain::types::PromptBundle, UiError> {
    crate::app::ui::ipc::compose(req).await.map_err(to_ui_error)
}

#[tauri::command]
async fn classify(input: ClassifyInput) -> Result<Vec<String>, UiError> {
    crate::app::ui::ipc::classify(input).await.map_err(to_ui_error)
}

/// Launch Tauri UI. In dev, frontend loads from local assets; in prod, it uses packaged assets.
pub async fn run_tauri_ui() -> Result<(), crate::domain::errors::ContextError> {
    let context: tauri::Context<Wry> = tauri::generate_context!();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            select_workspace,
            set_watch_paths,
            reindex,
            list_documents,
            read_document,
            update_document,
            read_ontology,
            update_ontology,
            read_rules,
            update_rules,
            get_settings,
            set_settings,
            compose,
            classify,
        ])
        .setup(|_app| Ok(()))
        .run(context)
        .map_err(|e| crate::domain::errors::ContextError::ServerError(e.to_string()))
}

static FS_WATCHERS: Lazy<Mutex<HashMap<String, RecommendedWatcher>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[tauri::command]
async fn set_watch_paths(paths: Vec<String>, app: AppHandle<Wry>) -> Result<Vec<String>, UiError> {
    // Persist to settings
    let mut current = crate::app::ui::ipc::get_settings().await.map_err(to_ui_error)?;
    current.watch_paths = Some(paths.clone());
    let saved = crate::app::ui::ipc::set_settings(current).await.map_err(to_ui_error)?;
    let bases: Vec<std::path::PathBuf> = saved
        .watch_paths
        .unwrap_or_default()
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect();
    set_watch_paths_internal(&app, bases)
        .map_err(|e| UiError { code: "E_WATCH".into(), message: e, data: None })
}

fn set_watch_paths_internal(app: &AppHandle<Wry>, bases: Vec<std::path::PathBuf>) -> Result<Vec<String>, String> {
    // Clear existing watchers
    {
        let mut map = FS_WATCHERS.lock().map_err(|_| "watchers lock poisoned")?;
        map.clear();
    }
    let mut ok_paths = Vec::new();
    for base in bases {
        if !base.exists() || !base.is_dir() {
            return Err(format!("invalid watch path: {}", base.display()));
        }
        let app_for_cb = app.clone();
        let base_str = base.to_string_lossy().to_string();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let paths: Vec<String> = event
                    .paths
                    .iter()
                    .filter_map(|p| p.to_str().map(|s| s.to_string()))
                    .collect();
                let md_changed = paths.iter().any(|p| p.ends_with(".md"));
                if md_changed {
                    let _ = app_for_cb.emit("workspace://change", json!({
                        "paths": paths,
                        "kind": format!("{:?}", event.kind),
                        "base": base_str,
                    }));
                }
            }
        }).map_err(|e| e.to_string())?;
        watcher.configure(Config::default()).map_err(|e| e.to_string())?;
        watcher.watch(&base, RecursiveMode::Recursive).map_err(|e| e.to_string())?;
        let mut map = FS_WATCHERS.lock().map_err(|_| "watchers lock poisoned")?;
        map.insert(base.to_string_lossy().to_string(), watcher);
        ok_paths.push(base.to_string_lossy().to_string());
    }
    Ok(ok_paths)
}
