pub mod constants;
pub mod errors;
pub mod rule_application;
pub mod types;

pub use errors::{ContextError, Result};
pub use types::{ContextDocument, SectionDef, ExtractedSection, ContextMetadata};
