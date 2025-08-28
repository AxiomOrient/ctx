// Lean architecture layers
pub mod common;
pub mod core;
pub mod data;
pub mod doc;
pub mod knowledge;

// Public API surface (selected re-exports)

// Base error/result
pub use common::errors::{ContextError, Result};

// Common data types
pub use common::{
    BuildQuery, ContextDocument, DocumentCandidate, CompositionResult, MergedDocument, SectionDef, ExtractedSection
};

// Core services
pub use core::classifier::service::ClassifierService;
pub use core::classifier::Classification;
pub use core::composer::BuildComposer;

// Data layer
pub use data::index::pooled_sqlite::{PooledSqliteIndexManager, DocumentIndex};
pub use data::storage::{LocalFsStorage, Storage};

// Doc layer
pub use doc::parse::{DocumentParser, FrontmatterParser, SectionExtractor};
pub use doc::schema::v1::V1SchemaValidator;
pub use doc::schema::{SchemaValidator, ValidationResult};

// Knowledge layer
pub use knowledge::ontology::OntologyRegistry;
pub use knowledge::rules::{builtin_rules, RuleSet};