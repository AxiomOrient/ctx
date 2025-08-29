# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Building and Testing
```bash
# Build the project
cargo build

# Run tests (including architecture guard tests)
cargo test --workspace

# Run tests for a specific module
cargo test <module_name>

# Run clippy linting with strict rules
cargo clippy -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

# Format code
cargo fmt --all --check
```

### Installing and Running
```bash
# Install from source
cargo install --path .

# Run CLI commands
ctx index-repo                          # Index repository incrementally
ctx index-repo --full                   # Full reindex
ctx build --tags "rust,api" --budget 4000  # Build prompt with constraints
ctx search --tags "rust,backend"        # Search documents
ctx validate                            # Validate context documents
ctx validate --fix                      # Auto-fix validation errors
ctx graph                               # Analyze dependency graph
```

### Feature Flags
The project uses Cargo feature flags for optional functionality:
- `cli` (default): CLI commands and utilities
- `server` (default): HTTP server with webhook endpoints
- `db_write`: Database write operations
- `legacy-compat`: Legacy compatibility mode

## Architecture Overview

ctx is a Rust-based AI context management tool with a **lean layered architecture** enforcing strict unidirectional dependencies:

```
common → knowledge → doc → core → data → app
```

### Core Layers
- **`common/`**: Shared types, constants, errors, utilities (used by all layers)
- **`knowledge/`**: Ontology and rules processing (`ontology/`, `rules/`)
- **`doc/`**: Document parsing, schema validation, structure validation (`parse/`, `schema/`, `validate/`)
- **`core/`**: Business logic - classification and composition (`classifier/`, `composer/`, `dependency/`)
- **`data/`**: Data access - SQLite indexing and local storage (`index/`, `storage/`, `git/`)
- **`app/`**: External interfaces - CLI and HTTP server (`cli/`, `server/`)

### Key Architecture Rules
1. **Unidirectional dependencies**: Each layer can only import from layers to its left
2. **Pure computation separation**: I/O operations only in `data/` and `knowledge/loader`
3. **Central constants**: All magic numbers/strings defined in `common/constants/`
4. **Feature-gated compilation**: Server and CLI can be disabled via feature flags
5. **Architecture guards**: Trybuild tests prevent dependency violations

### Core Components
- **Classification Engine**: Uses ontology-based facet analysis to categorize documents
- **Composition Engine**: MMR (Maximal Marginal Relevance) + Knapsack algorithms for optimal context selection
- **Dependency Resolver**: Automatically resolves document dependencies
- **Git Integration**: Incremental indexing based on Git changes
- **SQLite Index**: Persistent metadata storage for fast searches

## External Integrations

### Webhook Server
```bash
# Start webhook server
export CTX_HOOK_TOKEN="your-secret-token"
ctx server --port 3000

# Test endpoints: /v1/hooks/linear, /v1/hooks/jira, /v1/hooks/github
```

### MCP (Model Context Protocol) Server
```bash
# Start MCP server (stdio-based JSON-RPC)
ctx mcp

# Supported methods: ping, classifyText, composePrompt, listContexts, getContext
```

## Code Conventions

### Error Handling
- **Never use `.unwrap()` or `.expect()`** - enforced by clippy rules
- Use `Result<T, ContextError>` for all fallible operations
- Errors are centralized in `common/errors.rs`

### Constants Management
- All magic numbers/strings go in `common/constants/` modules:
  - `scoring.rs`: Algorithm weights and thresholds
  - `parsing.rs`: Document parsing constants
  - `validation.rs`: Validation thresholds
  - `defaults.rs`: Default values
  - `graph.rs`: Graph analysis constants

### Module Visibility
- Use `pub(crate)` for internal APIs
- Only re-export public APIs through `lib.rs`
- External APIs are minimal and carefully curated

### Testing
- Unit tests alongside implementation files
- Integration tests in `tests/` directory
- Architecture guard tests prevent dependency violations
- Use `tempfile` for test isolation

## Key Files and Their Purposes

- `ontology.yaml`: Document classification schema with facets and rules
- `rules.yaml`: Custom classification and validation rules
- `ctxindex.db`: SQLite database for document metadata and search
- `src/lib.rs`: Public API surface with controlled re-exports
- `docs/ARCHITECTURE.md`: Detailed architectural documentation
- `scripts/test_*.sh`: Integration testing scripts

## Development Workflow

1. **Before making changes**: Run `cargo test --workspace` to ensure architecture guards pass
2. **Adding new features**: Respect layer boundaries and update appropriate README files in each module
3. **Adding constants**: Place in appropriate `common/constants/` module
4. **Adding dependencies**: Consider if they belong in feature flags
5. **Testing integrations**: Use provided test scripts in `scripts/`

The codebase prioritizes architectural clarity and maintainability over convenience, with strict enforcement of design principles through automated testing.
