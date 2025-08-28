pub mod document;
pub mod v1;

pub use document::FacetDocument;

use crate::{domain::errors::Result, domain::types::ContextDocument};

/// 스키마 검증 결과
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub schema_version: String,
}

/// 검증 오류
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub error_type: ValidationErrorType,
}

/// 검증 경고
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
    pub suggestion: Option<String>,
}

/// 검증 오류 타입
#[derive(Debug, Clone)]
pub enum ValidationErrorType {
    Required,
    Format,
    Range,
    Duplicate,
    Invalid,
    Constraint,
}

/// 스키마 검증기 트레이트
pub trait SchemaValidator {
    fn validate_document(&self, doc: &ContextDocument) -> Result<ValidationResult>;
    fn is_compatible(&self, schema_version: &str) -> bool;
    fn supported_versions(&self) -> Vec<String>;
}
