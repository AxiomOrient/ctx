use super::{
    SchemaValidator, ValidationError, ValidationErrorType, ValidationResult, ValidationWarning,
};
use crate::domain::constants::validation::MAX_PRIORITY;
use crate::domain::errors::Result;
use crate::domain::types::ContextDocument;
use std::collections::HashSet;

/// context.v1 스키마 검증기
#[derive(Debug, Clone)]
pub struct V1SchemaValidator {
    /// 허용되는 문서 타입
    allowed_types: HashSet<String>,
    /// 필수 필드 목록
    #[allow(dead_code)]
    required_fields: HashSet<String>,
}

impl Default for V1SchemaValidator {
    fn default() -> Self {
        let mut allowed_types = HashSet::new();
        allowed_types.insert("guide".to_string());
        allowed_types.insert("persona".to_string());
        allowed_types.insert("workflow".to_string());
        allowed_types.insert("domain".to_string());

        let mut required_fields = HashSet::new();
        required_fields.insert("id".to_string());
        required_fields.insert("title".to_string());
        required_fields.insert("version".to_string());
        required_fields.insert("schema".to_string());
        required_fields.insert("type".to_string());
        required_fields.insert("sections".to_string());

        Self {
            allowed_types,
            required_fields,
        }
    }
}

impl V1SchemaValidator {
    pub fn new() -> Self {
        Self::default()
    }

    /// ID 형식 검증 (대문자, 하이픈, 언더스코어만 허용)
    fn validate_id_format(&self, id: &str) -> std::result::Result<(), ValidationError> {
        if id.trim().is_empty() {
            return Err(ValidationError {
                field: "id".to_string(),
                message: "ID cannot be empty".to_string(),
                error_type: ValidationErrorType::Required,
            });
        }

        // ID 형식: CATEGORY-TOPIC[-VERSION] (대문자, 하이픈, 언더스코어만)
        let valid_chars = id
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '-' || c == '_' || c.is_ascii_digit());
        if !valid_chars {
            return Err(ValidationError {
                field: "id".to_string(),
                message: "ID must contain only uppercase letters, hyphens, underscores, and digits"
                    .to_string(),
                error_type: ValidationErrorType::Format,
            });
        }

        Ok(())
    }

    /// 버전 형식 검증 (Semantic Versioning)
    fn validate_version_format(&self, version: &str) -> std::result::Result<(), ValidationError> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            return Err(ValidationError {
                field: "version".to_string(),
                message: "Version must follow semantic versioning (x.y.z)".to_string(),
                error_type: ValidationErrorType::Format,
            });
        }

        for part in parts {
            if part.parse::<u32>().is_err() {
                return Err(ValidationError {
                    field: "version".to_string(),
                    message: "Version parts must be numeric".to_string(),
                    error_type: ValidationErrorType::Format,
                });
            }
        }

        Ok(())
    }

    /// 섹션 정의 검증
    fn validate_sections(&self, doc: &ContextDocument) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        if doc.sections.is_empty() {
            errors.push(ValidationError {
                field: "sections".to_string(),
                message: "At least one section must be defined".to_string(),
                error_type: ValidationErrorType::Required,
            });
            return errors;
        }

        // 섹션 ID 중복 검사
        let mut seen_ids = HashSet::new();
        for section in &doc.sections {
            if !seen_ids.insert(&section.id) {
                errors.push(ValidationError {
                    field: format!("sections.{}", section.id),
                    message: format!("Duplicate section ID: '{}'", section.id),
                    error_type: ValidationErrorType::Constraint,
                });
            }

            // 섹션 마커 형식 검증 (## 으로 시작해야 함)
            if !section.marker.starts_with("## ") {
                errors.push(ValidationError {
                    field: format!("sections.{}.marker", section.id),
                    message: format!("Section marker must start with '## ': '{}'", section.marker),
                    error_type: ValidationErrorType::Format,
                });
            }

            // 우선순위 범위 검증
            if u32::from(section.priority) > MAX_PRIORITY {
                errors.push(ValidationError {
                    field: format!("sections.{}.priority", section.id),
                    message: format!(
                        "Priority must be between 0-{}, got: {}",
                        MAX_PRIORITY, section.priority
                    ),
                    error_type: ValidationErrorType::Constraint,
                });
            }
        }

        errors
    }

    /// 경고 생성
    fn generate_warnings(&self, doc: &ContextDocument) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        // 태그가 없는 경우 경고
        if doc.tags.is_empty() {
            warnings.push(ValidationWarning {
                field: "tags".to_string(),
                message: "No tags specified. Tags help with searchability".to_string(),
                suggestion: Some("Add relevant tags like [rust, backend, api]".to_string()),
            });
        }

        // 추정 토큰 수가 없는 경우 경고
        if doc.estimated_tokens.is_none() {
            warnings.push(ValidationWarning {
                field: "estimated_tokens".to_string(),
                message: "No token estimate provided".to_string(),
                suggestion: Some("Add estimated_tokens field for better optimization".to_string()),
            });
        }

        // 작성자가 없는 경우 경고
        if doc.author.is_none() {
            warnings.push(ValidationWarning {
                field: "author".to_string(),
                message: "No author specified".to_string(),
                suggestion: Some("Add author field for better tracking".to_string()),
            });
        }

        // 업데이트 날짜가 없는 경우 경고
        if doc.updated.is_none() {
            warnings.push(ValidationWarning {
                field: "updated".to_string(),
                message: "No update timestamp".to_string(),
                suggestion: Some("Add updated field with ISO 8601 format".to_string()),
            });
        }

        warnings
    }
}

impl SchemaValidator for V1SchemaValidator {
    fn validate_document(&self, doc: &ContextDocument) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // 필수 필드 검증
        if doc.id.trim().is_empty() {
            errors.push(ValidationError {
                field: "id".to_string(),
                message: "ID is required".to_string(),
                error_type: ValidationErrorType::Required,
            });
        } else if let Err(validation_error) = self.validate_id_format(&doc.id) {
            errors.push(validation_error);
        }

        if doc.title.trim().is_empty() {
            errors.push(ValidationError {
                field: "title".to_string(),
                message: "Title is required".to_string(),
                error_type: ValidationErrorType::Required,
            });
        }

        // 버전 형식 검증
        if let Err(validation_error) = self.validate_version_format(&doc.version) {
            errors.push(validation_error);
        }

        // 스키마 버전 검증
        if !self.is_compatible(&doc.schema) {
            errors.push(ValidationError {
                field: "schema".to_string(),
                message: format!("Unsupported schema version: '{}'", doc.schema),
                error_type: ValidationErrorType::Constraint,
            });
        }

        // 문서 타입 검증
        if !self.allowed_types.contains(&doc.r#type) {
            errors.push(ValidationError {
                field: "type".to_string(),
                message: format!(
                    "Invalid document type: '{}'. Allowed: {:?}",
                    doc.r#type, self.allowed_types
                ),
                error_type: ValidationErrorType::Constraint,
            });
        }

        // 섹션 검증
        errors.extend(self.validate_sections(doc));

        // 경고 생성
        let warnings = self.generate_warnings(doc);

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            schema_version: doc.schema.clone(),
        })
    }

    fn is_compatible(&self, schema_version: &str) -> bool {
        matches!(schema_version, "context.v1")
    }

    fn supported_versions(&self) -> Vec<String> {
        vec!["context.v1".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{ContextDocument, SectionDef};

    fn create_valid_document() -> ContextDocument {
        let mut doc = ContextDocument::new_test("RUST-API-GUIDE", "Rust API Development Guide");
        doc.domain = Some("rust".to_string());
        doc.tags = vec!["rust".to_string(), "api".to_string()];
        doc.author = Some("test".to_string());
        doc.estimated_tokens = Some(800);
        doc.facets.insert("language".to_string(), vec!["rust".to_string()]);
        doc.facets.insert("type".to_string(), vec!["guide".to_string()]);
        doc.sections = vec![SectionDef {
            id: "intro".to_string(),
            name: "Introduction".to_string(),
            marker: "## Introduction".to_string(),
            priority: 90,
            tokens: Some(150),
        }];
        doc
    }

    #[test]
    fn test_valid_document() -> crate::domain::errors::Result<()> {
        let validator = V1SchemaValidator::new();
        let doc = create_valid_document();

        let result = validator.validate_document(&doc)?;
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
        Ok(())
    }

    #[test]
    fn test_invalid_id_format() -> crate::domain::errors::Result<()> {
        let validator = V1SchemaValidator::new();
        let mut doc = create_valid_document();
        doc.id = "invalid-id-format".to_string(); // 소문자 포함

        let result = validator.validate_document(&doc)?;
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.field == "id"));
        Ok(())
    }

    #[test]
    fn test_invalid_version_format() -> crate::domain::errors::Result<()> {
        let validator = V1SchemaValidator::new();
        let mut doc = create_valid_document();
        doc.version = "1.0".to_string(); // 잘못된 형식

        let result = validator.validate_document(&doc)?;
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.field == "version"));
        Ok(())
    }

    #[test]
    fn test_duplicate_section_ids() -> crate::domain::errors::Result<()> {
        let validator = V1SchemaValidator::new();
        let mut doc = create_valid_document();
        doc.sections.push(SectionDef {
            id: "intro".to_string(), // 중복 ID
            name: "Another Intro".to_string(),
            marker: "## Another Intro".to_string(),
            priority: 80,
            tokens: Some(100),
        });

        let result = validator.validate_document(&doc)?;
        assert!(!result.is_valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.field.contains("sections.intro"))
        );
        Ok(())
    }

    #[test]
    fn test_warnings_generation() -> crate::domain::errors::Result<()> {
        let validator = V1SchemaValidator::new();
        let mut doc = create_valid_document();
        doc.tags.clear(); // 태그 제거
        doc.author = None; // 작성자 제거

        let result = validator.validate_document(&doc)?;
        assert!(result.is_valid); // 경고는 있지만 유효함
        assert!(!result.warnings.is_empty());
        assert!(result.warnings.iter().any(|w| w.field == "tags"));
        assert!(result.warnings.iter().any(|w| w.field == "author"));
        Ok(())
    }
}
