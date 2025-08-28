use crate::doc::validate::ValidationStage;
use crate::{ContextDocument, ContextError, Result};

/// 메타데이터 검증 스테이지
pub struct MetadataValidationStage;

impl ValidationStage for MetadataValidationStage {
    fn name(&self) -> &'static str {
        "metadata"
    }

    fn validate(&self, doc: &ContextDocument, _body: &str) -> Result<()> {
        // ID 검증
        if doc.id.is_empty() {
            return Err(ContextError::InvalidFrontmatter(
                "Document ID cannot be empty".to_string(),
            ));
        }

        // 제목 검증
        if doc.title.is_empty() {
            return Err(ContextError::InvalidFrontmatter(
                "Document title cannot be empty".to_string(),
            ));
        }

        // 버전 검증 (기본 형식)
        if !doc
            .version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err(ContextError::InvalidFrontmatter(
                "Invalid version format".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_validation_stage() {
        let stage = MetadataValidationStage;

        // 유효한 문서
        let valid_doc = ContextDocument::new_test("TEST-DOC", "Test Document");
        assert!(stage.validate(&valid_doc, "body").is_ok());

        // 빈 ID
        let invalid_doc = ContextDocument::new_test("", "Test Document");
        assert!(stage.validate(&invalid_doc, "body").is_err());

        // 빈 제목
        let invalid_doc = ContextDocument::new_test("TEST-DOC", "");
        assert!(stage.validate(&invalid_doc, "body").is_err());
    }
}
