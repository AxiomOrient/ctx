#![deny(unused_must_use, unreachable_patterns)]
#![deny(clippy::all)]

//! MCP Server (rmcp SDK only)
//! - Custom server removed; rmcp SDK is the single path.
//! - Response schema is fixed via public types and schemars.
//! - Error codes standardized via `CtxErrorCode` and helpers.

use crate::domain::errors::ContextError;
use crate::domain::types::{ComposeInput, WorkInput};
use crate::services;
use rmcp::{
    ErrorData as McpError,
    RoleServer, ServerHandler, ServiceExt,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::stdio,
};
use rmcp::schemars as schemars;
use serde::{Deserialize, Serialize};

/// Standardized error codes for ctx MCP server.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CtxErrorCode {
    ERuleViolation,
    ENotFound,
    EBudgetExceeded,
    EInvalidInput,
    EInternal,
}

impl CtxErrorCode {
    fn to_mcp_invalid(self, message: impl Into<String>) -> McpError {
        McpError::invalid_params(message.into(), None)
    }
    fn to_mcp_not_found(self, message: impl Into<String>) -> McpError {
        McpError::resource_not_found(message.into(), None)
    }
    fn to_mcp_internal(self, message: impl Into<String>) -> McpError {
        McpError::internal_error(message.into(), None)
    }
}

#[derive(Clone, Default)]
pub struct CtxServer {
    #[allow(dead_code)]
    tool_router: ToolRouter<CtxServer>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
struct ComposeArgs {
    #[serde(default)]
    query: String,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
struct WorkArgs {
    #[serde(default)]
    task: String,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    max_tokens: Option<u32>,
}

#[tool_router]
impl CtxServer {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }

    /// Compose prompt bundle from files and a query
    #[tool(description = "Compose prompt bundle from files and a query")]
    async fn compose_prompt(
        &self,
        Parameters(args): Parameters<ComposeArgs>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let input = ComposeInput {
            query: args.query,
            tags: args.tags,
            max_tokens: args.max_tokens,
            exclude_sources: None,
            priority_sources: None,
        };
        let bundle = services::compose_prompt(input)
            .map_err(|e| CtxErrorCode::EInternal.to_mcp_internal(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::json(bundle)?]))
    }

    /// Compose and (optionally) send to AI provider
    #[tool(description = "Compose and (optionally) send to AI provider")]
    async fn work(
        &self,
        Parameters(args): Parameters<WorkArgs>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let wi = WorkInput {
            task: args.task,
            context: args.context,
            ai_provider: args.provider,
            max_tokens: args.max_tokens,
            temperature: args.temperature,
        };
        let out = services::work(wi)
            .await
            .map_err(|e| CtxErrorCode::EInternal.to_mcp_internal(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }
}

#[tool_handler]
impl ServerHandler for CtxServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            server_info: Implementation {
                name: "ctx".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some(
                "Context composition and AI work tools; exposes documents as resources and a compose prompt".to_string(),
            ),
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListResourcesResult, McpError> {
        let resources = list_documents().map_err(|e| CtxErrorCode::EInternal.to_mcp_internal(e.to_string()))?;
        Ok(ListResourcesResult { resources, next_cursor: None })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ReadResourceResult, McpError> {
        let (_mime, body) = read_document(&uri)
            .map_err(|e| CtxErrorCode::ENotFound.to_mcp_not_found(e.to_string()))?;
        Ok(ReadResourceResult { contents: vec![ResourceContents::text(body, uri)] })
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            prompts: vec![Prompt::new(
                "compose_builder",
                Some("Template for composing context"),
                Some(vec![PromptArgument {
                    name: "task".to_string(),
                    description: Some("Task description".to_string()),
                    required: Some(true),
                }]),
            )],
            next_cursor: None,
        })
    }

    async fn get_prompt(
        &self,
        GetPromptRequestParam { name, arguments }: GetPromptRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<GetPromptResult, McpError> {
        if name != "compose_builder" {
            return Err(CtxErrorCode::EInvalidInput.to_mcp_invalid("Unknown prompt"));
        }
        let task = arguments
            .as_ref()
            .and_then(|m| m.get("task"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let template = default_compose_template();
        let content = template.replace("{{task}}", task).replace("{{context}}", "");

        Ok(GetPromptResult {
            description: Some("Compose builder prompt".into()),
            messages: vec![PromptMessage { role: PromptMessageRole::User, content: PromptMessageContent::text(content) }],
        })
    }
}

fn docs_root() -> std::path::PathBuf { std::path::PathBuf::from("documents") }

fn list_documents() -> crate::domain::errors::Result<Vec<Resource>> {
    use crate::drivers::storage::{local::LocalFsStorage, Storage};
    let storage = LocalFsStorage::new(std::path::PathBuf::from("."));
    let mut items = Vec::new();
    for path in storage.list_files(&docs_root(), "*.md")? {
        let id = path.to_string_lossy().to_string();
        let raw = RawResource { uri: format!("doc://{}", id), name: id.clone(), description: Some("Markdown document".into()), mime_type: Some("text/markdown".into()), size: None };
        items.push(raw.no_annotation());
    }
    Ok(items)
}

fn read_document(uri: &str) -> crate::domain::errors::Result<(String, String)> {
    use crate::drivers::storage::{local::LocalFsStorage, Storage};
    let storage = LocalFsStorage::new(std::path::PathBuf::from("."));
    let rel = uri.strip_prefix("doc://").unwrap_or(uri);
    let content = storage.read(std::path::Path::new(rel))?;
    Ok(("text/markdown".to_string(), content))
}

fn default_compose_template() -> String {
    "# Context\n\n{{context}}\n\n# Task\n\n{{task}}\n".to_string()
}

pub async fn run_mcp_server() -> crate::domain::errors::Result<()> {
    let svc = CtxServer::new()
        .serve(stdio())
        .await
        .map_err(|e| ContextError::ServerError(e.to_string()))?;
    let _ = svc
        .waiting()
        .await
        .map_err(|e| ContextError::ServerError(e.to_string()))?;
    Ok(())
}
