# File Structure (Lean Architecture)

Version: 2025-08-20

This document provides an overview of the `ctxset` project's file structure, reflecting the implemented lean 6-layer architecture.

## 1. Source Layout (`src/`)

The source code is organized into six primary layers, plus application entry points.

```
src/
├─ lib.rs                // Main library crate, module declarations
├─ main.rs               // CLI application entry point
│
├─ common/               // Shared types, constants, errors, and utilities (Tier 0)
│  ├─ constants/         // Centralized constants for determinism
│  └─ errors.rs          // Project-wide error types (`ContextError`)
│
├─ knowledge/            // Ontology and rules management (Tier 1)
│  ├─ ontology/          // `ontology.yaml` schema and normalization logic
│  └─ rules/            // `rules.yaml` schema, loader, and applier
│
├─ doc/                  // Document parsing, schema definition, and validation (Tier 2)
│  ├─ parse/             // Markdown frontmatter and section extraction
│  ├─ schema/            // Versioned document schemas (e.g., `context.v1`)
│  └─ validate/          // Multi-stage validation pipeline (metadata, schema, structure)
│
├─ core/                 // Core business logic (classifier, composer) (Tier 3)
│  ├─ classifier/        // Facet classification engine and services
│  ├─ composer/          // Prompt composition logic
│  │  ├─ scorer/         // Modular scoring algorithms (keyword, freshness, etc.)
│  │  ├─ selector.rs     // MMR-based document selection
│  │  └─ merger.rs       // Document content merging
│  ├─ dependency/        // Dependency graph analysis for documents
│  └─ determinism.rs     // Determinism engine for consistent outputs
│
├─ data/                 // Data access layer (database, storage) (Tier 4)
│  ├─ index/             // SQLite indexing logic
│  │  ├─ pool.rs         // `r2d2` connection pool for server
│  │  ├─ pooled_sqlite.rs// Pooled version of the index manager
│  │  ├─ sqlite.rs       // Core SQLite interaction logic
│  │  └─ sqlite_schema.sql // Database DDL
│  ├─ optimization/      // Query and index optimization tools
│  ├─ storage/           // Filesystem abstraction (`LocalFsStorage`)
│  ├─ git/               // Git utilities for tracking changes
│  └─ id/                // Stable ID generation for documents
│
└─ app/                  // External interfaces (CLI, Server) (Tier 5)
   ├─ cli/               // Command-Line Interface
   │  ├─ app.rs          // `clap` setup and command routing
   │  └─ commands/       // Implementations for each CLI command
   └─ server/            // HTTP Server (Axum)
      ├─ http.rs         // API/UI route handlers
      ├─ dto.rs          // Data Transfer Objects for API requests/responses
      ├─ validation.rs   // API input validation layer
      └─ providers.rs    // Webhook payload parsers (GitHub, Jira, etc.)
```

## 2. Key Architectural Features

This structure embodies several key architectural decisions:

- **Lean 6-Layer Architecture:** Code is strictly organized into `common`, `knowledge`, `doc`, `core`, `data`, and `app` layers to enforce separation of concerns.
- **Unidirectional Dependency Flow:** Dependencies flow from higher-level abstractions to lower-level ones (`app` → `core` → `doc` → `knowledge` → `common`). The `data` layer is a side-car, only depending on `common`. This is enforced by `tests/arch_guard.rs`.
- **Determinism Engine:** The `core/determinism.rs` module provides the foundation for generating consistent outputs from identical inputs, a core requirement of the project.
- **Pooled & Versioned Database:** The `data/index/` modules implement a sophisticated SQLite backend using connection pooling (`pool.rs`) for performance and a temporal (SCD2) schema (`sqlite_schema.sql`) for versioning document snapshots.
- **Modular Scorer & Composer:** The `core/composer/` logic is highly modular, with individual scorers that can be weighted and combined, and an MMR-based selector to ensure both relevance and diversity in composed prompts.
- **Dual-Interface System:** The `app/` layer provides both a powerful CLI for developers/automation and an Axum-based web server for UI-driven interactions and API access, all sharing the same `core` logic.
- **Extensible Provider System:** The `app/server/providers.rs` module allows for easy extension to support new webhook sources (e.g., GitLab, Linear) by implementing a common `ProviderParser` trait.

## 3. Dependency Rules

- **`common`**: Usable by all other layers. Contains no business logic.
- **`knowledge`**: Depends only on `common`.
- **`doc`**: Depends on `common` and `knowledge`.
- **`core`**: Depends on `common`, `knowledge`, and `doc`.
- **`data`**: Depends only on `common`. It is explicitly forbidden from depending on `core`, `doc`, or `knowledge`.
- **`app`**: The highest layer, depends on all other layers to orchestrate user-facing functionality.

See `docs/ARCHITECTURE.md` for full principles and diagrams.