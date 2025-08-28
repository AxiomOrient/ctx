use crate::doc::validate::ValidationStage;
use crate::{ContextDocument, ContextError, Result};

/// 구조 검증 스테이지
pub struct StructureValidationStage;

impl ValidationStage for StructureValidationStage {
    fn name(&self) -> &'static str {
        "structure"
    }

    fn validate(&self, doc: &ContextDocument, body: &str) -> Result<()> {
        // 본문이 비어있지 않은지 검증
        if body.trim().is_empty() {
            return Err(ContextError::InvalidFrontmatter(
                "Document body cannot be empty".to_string(),
            ));
        }

        // 섹션 마커가 실제로 본문에 존재하는지 검증
        for section in &doc.sections {
            if !body.contains(&section.marker) {
                return Err(ContextError::InvalidFrontmatter(format!(
                    "Section marker '{}' not found in document body",
                    section.marker
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SectionDef;

    #[test]
    fn test_structure_validation_stage() {
        let stage = StructureValidationStage;

        // 유효한 문서 (섹션 없음)
        let valid_doc = ContextDocument::new_test("TEST-DOC", "Test Document");
        assert!(
            stage
                .validate(&valid_doc, "## Introduction\nContent")
                .is_ok()
        );

        // 빈 본문
        assert!(stage.validate(&valid_doc, "").is_err());
        assert!(stage.validate(&valid_doc, "   ").is_err());

        // 섹션 마커가 본문에 없는 경우
        let mut doc_with_section = ContextDocument::new_test("TEST-DOC", "Test Document");
        doc_with_section.sections.push(SectionDef {
            id: "missing".to_string(),
            name: "Missing Section".to_string(),
            marker: "## Missing".to_string(),
            priority: 10,
            tokens: Some(100),
        });
        assert!(
            stage
                .validate(&doc_with_section, "## Introduction\nContent")
                .is_err()
        );

        // 섹션 마커가 본문에 있는 경우
        assert!(
            stage
                .validate(&doc_with_section, "## Missing\nContent")
                .is_ok()
        );
    }
}
