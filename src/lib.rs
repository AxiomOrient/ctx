//! ctx - AI Context Management Library
//!
//! A simple yet precise library for AI context generation following PLAN.md architecture.
//! Single binary with multiple modes: CLI/HTTP/MCP stdio

// PLAN.md compliant single binary architecture
pub mod app; // Entry point (mode switcher: CLI/HTTP/MCP)
pub mod config;
pub mod doc; // Document parsing and validation
pub mod domain; // Domain models (documents, facets, scores), errors
pub mod drivers; // Storage/HTTP/MCP/AI drivers
pub mod knowledge; // Ontology and rules system for classification
pub mod pipeline; // Single prompt pipeline (7 stages)
pub mod services; // Use cases (facade): classify_text, compose_prompt, work
pub mod util; // Token counting, hashing (determinism), common helpers // Config loader

// Public API surface - minimal and clean
pub use domain::errors::{ContextError, Result};
pub use domain::types::{AiPrompt, AiResponse, ComposeInput, PromptBundle, WorkInput, WorkOutput};

// Core pipeline functions
pub use services::{classify_text, compose_prompt, work};
