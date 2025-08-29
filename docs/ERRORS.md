# Error Codes (UI / IPC / MCP)

This document lists the standardized error codes and payload shape used across the UI IPC (Tauri), HTTP, and MCP layers.

- Payload shape (JSON):
  - code: SCREAMING_SNAKE_CASE
  - message: short description
  - data: optional JSON with machine-readable details

## Codes

- E_PARSE_ERROR: parsing error with location
  - data: { line: number }
- E_INVALID_INPUT: invalid parameters/frontmatter/arguments
- E_NOT_FOUND: missing marker/section/resource
  - data: optional { available: string[] }
- E_BUDGET_EXCEEDED: token budget exceeded
  - data: { budget: number, excess: number }
- E_CONFIG: configuration error (invalid workspace etc.)
- E_IO: filesystem I/O error
- E_SERIALIZE: serialization/deserialization error (YAML/JSON)
- E_CLASSIFY: classification pipeline error
- E_COMPOSE: composition pipeline error
- E_OPTIMIZE: optimization error
- E_ASSEMBLY: assembly error
- E_DB: database error
- E_INTERNAL: internal/server error
- E_WATCH: filesystem watcher setup/teardown error
- E_EVENT: frontend event hookup error
- E_UNKNOWN: untyped runtime error surfaced to UI

## UI Banner

- Shows `code` and `message`, with a "JSON" toggle to view full payload.
- All UI IPC commands return `Result<T, UiError>` to ensure structured errors.

## MCP

- MCP handlers should map internal errors to rmcp::ErrorData using equivalent codes:
  - NOT_FOUND → resource_not_found
  - INVALID_PARAMS → invalid input
  - INTERNAL_ERROR → internal
- MCP schema snapshot tests should include failures for discovery/access/tool routes to guard stability.
