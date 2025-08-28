use crate::{domain::{errors::{ContextError, Result}, types::ContextDocument}};
use chrono::Utc;
use std::path::Path;

/// 문서 변환을 위한 트레이트
pub trait Transformer {
    /// 비표준 마크다운을 표준 contexts 형식으로 변환
    fn transform(&self, content: &str, hint: Option<TransformHint>) -> Result<String>;

    /// 변환 가능 여부 확인
    fn can_transform(&self, content: &str) -> bool;
}

/// 변환 힌트
#[derive(Debug, Clone)]
pub struct TransformHint {
    pub suggested_id: Option<String>,
    pub suggested_type: Option<String>,
    pub suggested_domain: Option<String>,
    pub suggested_tags: Vec<String>,
    pub file_path: Option<String>,
}

/// 표준 변환기 구현
#[derive(Debug, Default)]
pub struct StandardTransformer {
    pub default_type: String,
    pub default_schema: String,
}

impl StandardTransformer {
    pub fn new() -> Self {
        Self {
            default_type: "guide".to_string(),
            default_schema: "context.v1".to_string(),
        }
    }

    /// 파일 경로에서 ID 생성
    fn generate_id_from_path(&self, path: &str) -> String {
        Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_uppercase().replace(['-', '_', ' '], "-"))
            .unwrap_or_else(|| format!("UNTITLED-{}", Utc::now().timestamp()))
    }

    /// 내용에서 제목 추출
    fn extract_title(&self, content: &str) -> String {
        const MAX_TITLE_LENGTH: usize = 50;
        const TITLE_SUFFIX: &str = "...";
        const DEFAULT_TITLE: &str = "Untitled Document";
        const HEADING_PREFIX: &str = "# ";
        const CODE_BLOCK_PREFIX: &str = "```";

        // 첫 번째 # 헤딩을 찾아서 제목으로 사용
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(stripped) = trimmed.strip_prefix(HEADING_PREFIX) {
                return stripped.trim().to_string();
            }
        }

        // 헤딩이 없으면 첫 번째 비어있지 않은 줄을 제목으로 사용
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with(CODE_BLOCK_PREFIX) {
                return trimmed.chars().take(MAX_TITLE_LENGTH).collect::<String>() + TITLE_SUFFIX;
            }
        }

        DEFAULT_TITLE.to_string()
    }

    /// 내용에서 섹션 자동 감지
    fn detect_sections(&self, content: &str) -> Vec<crate::domain::types::SectionDef> {
        const INITIAL_PRIORITY: u8 = 90;
        const MIN_PRIORITY: u8 = 80;
        const PRIORITY_DECREMENT: u8 = 1;
        const SECTION_HEADING_PREFIX: &str = "## ";
        const DEFAULT_SECTION_ID: &str = "content";
        const DEFAULT_SECTION_NAME: &str = "Content";
        const DEFAULT_SECTION_MARKER: &str = "## Content";
        const ID_REPLACEMENT_CHARS: [char; 3] = [' ', '-', '_'];
        const ID_SEPARATOR: &str = "_";

        let mut sections = Vec::new();
        let mut priority = INITIAL_PRIORITY;

        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(stripped) = trimmed.strip_prefix(SECTION_HEADING_PREFIX) {
                let title = stripped.trim();
                let id = title
                    .to_lowercase()
                    .replace(ID_REPLACEMENT_CHARS, ID_SEPARATOR)
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<String>();

                sections.push(crate::domain::types::SectionDef {
                    id,
                    name: title.to_string(),
                    marker: trimmed.to_string(),
                    priority,
                    tokens: None,
                });

                // 우선순위 감소 (최소값까지)
                if priority > MIN_PRIORITY {
                    priority = priority.saturating_sub(PRIORITY_DECREMENT);
                }
            }
        }

        // 기본 섹션이 없으면 추가
        if sections.is_empty() {
            sections.push(crate::domain::types::SectionDef {
                id: DEFAULT_SECTION_ID.to_string(),
                name: DEFAULT_SECTION_NAME.to_string(),
                marker: DEFAULT_SECTION_MARKER.to_string(),
                priority: INITIAL_PRIORITY,
                tokens: None,
            });
        }

        sections
    }

    /// 기술 키워드 매핑 테이블 반환
    /// 향후 설정 파일이나 외부 소스에서 로드할 수 있도록 분리
    fn get_tech_keywords() -> &'static [(&'static str, &'static str)] {
        const TECH_KEYWORDS: &[(&str, &str)] = &[
            ("rust", "rust"),
            ("python", "python"),
            ("javascript", "javascript"),
            ("typescript", "typescript"),
            ("react", "react"),
            ("vue", "vue"),
            ("angular", "angular"),
            ("node", "nodejs"),
            ("api", "api"),
            ("rest", "rest"),
            ("graphql", "graphql"),
            ("database", "database"),
            ("sql", "sql"),
            ("nosql", "nosql"),
            ("docker", "docker"),
            ("kubernetes", "kubernetes"),
            ("aws", "aws"),
            ("azure", "azure"),
            ("gcp", "gcp"),
        ];
        TECH_KEYWORDS
    }

    /// 내용에서 태그 추론
    fn infer_tags(&self, content: &str, hint: &Option<TransformHint>) -> Vec<String> {
        let mut tags = Vec::new();

        // 힌트에서 태그 가져오기
        if let Some(hint) = hint {
            tags.extend_from_slice(&hint.suggested_tags);
        }

        // 내용에서 기술 스택 키워드 감지
        let content_lower = content.to_lowercase();
        let tech_keywords = Self::get_tech_keywords();

        for (keyword, tag) in tech_keywords {
            if content_lower.contains(keyword) {
                let tag_string = tag.to_string();
                if !tags.contains(&tag_string) {
                    tags.push(tag_string);
                }
            }
        }

        // 기본 태그
        if tags.is_empty() {
            const DEFAULT_TAG: &str = "general";
            tags.push(DEFAULT_TAG.to_string());
        }

        tags
    }
}

impl Transformer for StandardTransformer {
    fn can_transform(&self, content: &str) -> bool {
        // frontmatter가 없거나 불완전한 경우 변환 가능
        if !content.trim_start().starts_with("---") {
            return true;
        }

        // frontmatter가 있지만 필수 필드가 없는 경우
        let parser = crate::doc::parse::frontmatter::FrontmatterParser::new();
        if let Ok((frontmatter, _)) = parser.extract_frontmatter(content) {
            let doc_result: std::result::Result<ContextDocument, _> =
                serde_yaml::from_str(&frontmatter);
            if doc_result.is_err() {
                return true;
            }

            if let Ok(doc) = doc_result {
                // 필수 필드 검사
                return doc.id.trim().is_empty()
                    || doc.title.trim().is_empty()
                    || doc.sections.is_empty();
            }
        }

        false
    }

    fn transform(&self, content: &str, hint: Option<TransformHint>) -> Result<String> {
        let parser = crate::doc::parse::frontmatter::FrontmatterParser::new();
        let (existing_frontmatter, body) = if content.trim_start().starts_with("---") {
            parser.extract_frontmatter(content)?
        } else {
            (String::new(), content.to_string())
        };

        // 기존 frontmatter에서 정보 추출 (있다면)
        let existing_doc: Option<ContextDocument> = if !existing_frontmatter.is_empty() {
            serde_yaml::from_str(&existing_frontmatter).ok()
        } else {
            None
        };

        // 새로운 문서 메타데이터 생성
        let id = hint
            .as_ref()
            .and_then(|h| h.suggested_id.clone())
            .or_else(|| existing_doc.as_ref().map(|d| d.id.clone()))
            .or_else(|| {
                hint.as_ref()
                    .and_then(|h| h.file_path.as_ref().map(|p| self.generate_id_from_path(p)))
            })
            .unwrap_or_else(|| format!("UNTITLED-{}", Utc::now().timestamp()));

        let title = existing_doc
            .as_ref()
            .map(|d| d.title.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| self.extract_title(&body));

        let doc_type = hint
            .as_ref()
            .and_then(|h| h.suggested_type.clone())
            .or_else(|| existing_doc.as_ref().map(|d| d.r#type.clone()))
            .unwrap_or_else(|| self.default_type.clone());

        let domain = hint
            .as_ref()
            .and_then(|h| h.suggested_domain.clone())
            .or_else(|| existing_doc.as_ref().and_then(|d| d.domain.clone()));

        let tags = self.infer_tags(&body, &hint);
        let sections = self.detect_sections(&body);

        // 새로운 frontmatter 생성
        let new_doc = ContextDocument {
            id,
            title,
            version: existing_doc
                .as_ref()
                .map(|d| d.version.clone())
                .unwrap_or_else(|| "1.0.0".to_string()),
            schema: self.default_schema.clone(),
            r#type: doc_type,
            domain,
            tags,
            facets: existing_doc
                .as_ref()
                .map(|d| d.facets.clone())
                .unwrap_or_default(),
            locale: existing_doc.as_ref().and_then(|d| d.locale.clone()),
            trust: existing_doc.as_ref().and_then(|d| d.trust),
            freshness: existing_doc.as_ref().and_then(|d| d.freshness.clone()),
            aliases: existing_doc.as_ref().and_then(|d| d.aliases.clone()),
            conflicts: existing_doc.as_ref().and_then(|d| d.conflicts.clone()),
            dependencies: existing_doc
                .as_ref()
                .map(|d| d.dependencies.clone())
                .unwrap_or_default(),
            author: existing_doc.as_ref().and_then(|d| d.author.clone()),
            updated: Some(Utc::now().to_rfc3339()),
            estimated_tokens: existing_doc.as_ref().and_then(|d| d.estimated_tokens),
            sections,
            path: None,
        };

        // YAML 직렬화
        let frontmatter_yaml = serde_yaml::to_string(&new_doc).map_err(|e| {
            ContextError::AssemblyError(format!("Failed to serialize frontmatter: {}", e))
        })?;

        // 최종 문서 조립
        let result = format!("---\n{}---\n\n{}", frontmatter_yaml, body.trim());

        Ok(result)
    }
}

// extract_frontmatter_simple 함수 제거됨 - FrontmatterParser 사용

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_transform_no_frontmatter() {
        let transformer = StandardTransformer::new();
        let content = "# My Document\n\nThis is content.";
        assert!(transformer.can_transform(content));
    }

    #[test]
    fn test_can_transform_incomplete_frontmatter() {
        let transformer = StandardTransformer::new();
        let content = "---\ntitle: Test\n---\n\nContent";
        assert!(transformer.can_transform(content));
    }

    #[test]
    fn test_transform_basic() -> crate::domain::errors::Result<()> {
        let transformer = StandardTransformer::new();
        let content =
            "# Rust Guide\n\n## Introduction\nThis is about Rust.\n\n## Examples\nSome examples.";

        let hint = TransformHint {
            suggested_id: Some("RUST-GUIDE".to_string()),
            suggested_type: Some("guide".to_string()),
            suggested_domain: Some("rust".to_string()),
            suggested_tags: vec!["rust".to_string(), "programming".to_string()],
            file_path: None,
        };

        let result = transformer.transform(content, Some(hint))?;

        assert!(result.contains("id: RUST-GUIDE"));
        assert!(result.contains("title: Rust Guide"));
        assert!(result.contains("type: guide"));
        assert!(result.contains("domain: rust"));
        assert!(result.contains("- rust"));
        assert!(result.contains("- programming"));
        assert!(result.contains("## Introduction"));
        assert!(result.contains("## Examples"));
        Ok(())
    }

    #[test]
    fn test_detect_sections() {
        let transformer = StandardTransformer::new();
        let content = "# Title\n\n## Section 1\nContent 1\n\n## Section 2\nContent 2";
        let sections = transformer.detect_sections(content);

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "Section 1");
        assert_eq!(sections[0].marker, "## Section 1");
        assert_eq!(sections[1].name, "Section 2");
        assert_eq!(sections[1].marker, "## Section 2");
    }

    #[test]
    fn test_infer_tags() {
        let transformer = StandardTransformer::new();
        let content = "This document covers Rust programming and API development with Docker.";
        let tags = transformer.infer_tags(content, &None);

        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"api".to_string()));
        assert!(tags.contains(&"docker".to_string()));
    }
}
