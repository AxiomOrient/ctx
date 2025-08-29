use crate::{domain::{errors::{ContextError, Result}, types::ContextDocument}, doc::validate::ValidationStage};

/// 스키마 검증 스테이지
pub struct SchemaValidationStage;

impl ValidationStage for SchemaValidationStage {
    fn name(&self) -> &'static str {
        "schema"
    }

    fn validate(&self, doc: &ContextDocument, _body: &str) -> Result<()> {
        // 섹션이 최소 하나는 있어야 함
        if doc.sections.is_empty() {
            return Err(ContextError::InvalidFrontmatter(
                "Document must have at least one section".to_string(),
            ));
        }

        // 섹션 ID 중복 검증
        let mut section_ids = std::collections::HashSet::new();
        for section in &doc.sections {
            if !section_ids.insert(&section.id) {
                return Err(ContextError::InvalidFrontmatter(format!(
                    "Duplicate section ID: {}",
                    section.id
                )));
            }

            // 섹션별 우선순위 검증
            if section.priority > 100 {
                return Err(ContextError::InvalidFrontmatter(format!(
                    "Section '{}' priority must be <= 100",
                    section.id
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::SectionDef;

    #[test]
    fn test_schema_validation_stage() {
        let stage = SchemaValidationStage;

        // 섹션이 없는 문서 (실패해야 함)
        let doc_no_sections = ContextDocument::new_test("TEST-DOC", "Test Document");
        assert!(stage.validate(&doc_no_sections, "body").is_err());

        // 섹션이 있는 유효한 문서
        let mut valid_doc = ContextDocument::new_test("TEST-DOC", "Test Document");
        valid_doc.sections.push(SectionDef {
            id: "intro".to_string(),
            name: "Introduction".to_string(),
            marker: "## Introduction".to_string(),
            priority: 10,
            tokens: Some(100),
        });
        assert!(stage.validate(&valid_doc, "body").is_ok());

        // 잘못된 섹션 우선순위
        let mut invalid_doc = ContextDocument::new_test("TEST-DOC", "Test Document");
        invalid_doc.sections.push(SectionDef {
            id: "intro".to_string(),
            name: "Introduction".to_string(),
            marker: "## Introduction".to_string(),
            priority: 150, // 잘못된 우선순위
            tokens: Some(100),
        });
        assert!(stage.validate(&invalid_doc, "body").is_err());
    }
}
