use crate::common::constants::parsing::HEADER_PREFIX;
use crate::{ContextDocument, ExtractedSection, Result};

/// 섹션 추출기 - extractor/section.rs 기능 구현
pub struct SectionExtractor;

impl SectionExtractor {
    pub fn new() -> Self {
        Self
    }

    /// 문서에서 섹션들을 추출
    pub fn extract_sections(
        &self,
        doc: &ContextDocument,
        content: &str,
    ) -> Result<Vec<ExtractedSection>> {
        let mut sections = Vec::new();

        for section_def in &doc.sections {
            if let Some(section_content) = self.find_section_content(content, &section_def.marker) {
                let tokens = self.estimate_tokens(&section_content);

                sections.push(ExtractedSection {
                    id: section_def.id.clone(),
                    name: section_def.name.clone(),
                    content: section_content,
                    tokens,
                    priority: section_def.priority,
                    score: 0.0, // 기본 점수, 나중에 스코어링에서 계산
                });
            }
        }

        Ok(sections)
    }

    /// 마커를 기준으로 섹션 내용 찾기
    fn find_section_content(&self, content: &str, marker: &str) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();

        // 마커 라인 찾기
        let start_idx = lines.iter().position(|line| line.trim() == marker.trim())?;

        // 다음 헤더까지 또는 파일 끝까지 내용 수집
        let mut end_idx = lines.len();
        for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
            if line.trim().starts_with(HEADER_PREFIX) {
                end_idx = i;
                break;
            }
        }

        if start_idx + 1 < end_idx {
            let section_lines = &lines[start_idx + 1..end_idx];
            Some(section_lines.join("\n").trim().to_string())
        } else {
            None
        }
    }

    /// 간단한 토큰 추정
    fn estimate_tokens(&self, content: &str) -> u32 {
        use crate::common::TOKEN_ESTIMATION_MULTIPLIER;

        let word_count = content.split_whitespace().count();
        ((word_count as f32) * TOKEN_ESTIMATION_MULTIPLIER).ceil() as u32
    }
}

impl Default for SectionExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContextDocument, SectionDef};

    #[test]
    #[allow(clippy::expect_used)] // 테스트 코드에서는 허용
    fn test_extract_sections() {
        let mut doc = ContextDocument::new("TEST".to_string(), "Test Doc".to_string());
        doc.sections = vec![
            SectionDef {
                id: "intro".to_string(),
                name: "Introduction".to_string(),
                marker: "## Introduction".to_string(),
                priority: 90,
                tokens: None,
            },
            SectionDef {
                id: "details".to_string(),
                name: "Details".to_string(),
                marker: "## Details".to_string(),
                priority: 80,
                tokens: None,
            },
        ];

        let content = "# Test Document

## Introduction
This is the introduction section.
It has multiple lines.

## Details
This is the details section.

## Not Defined
This section is not in the document definition.
";

        let extractor = SectionExtractor::new();
        let sections = extractor
            .extract_sections(&doc, content)
            .expect("Section extraction should succeed in test");

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].id, "intro");
        assert!(sections[0].content.contains("This is the introduction"));
        assert_eq!(sections[1].id, "details");
        assert!(sections[1].content.contains("This is the details"));
    }
}
