use super::frontmatter::FrontmatterParser;
use crate::{ContextDocument, ContextError, Result, Storage};
use std::path::Path;

/// 문서 파싱을 위한 공통 유틸리티
/// registry/memory.rs와 category/standard.rs의 중복 로직을 통합
#[derive(Debug, Clone)]
pub struct DocumentParser {
    frontmatter_parser: FrontmatterParser,
}

impl DocumentParser {
    pub fn new() -> Self {
        Self {
            frontmatter_parser: FrontmatterParser::new(),
        }
    }

    /// 파일에서 ContextDocument와 본문을 함께 파싱
    /// 성능 최적화를 위해 파일을 한 번만 읽음
    pub fn parse_file_with_body<S: Storage + ?Sized>(
        &self,
        storage: &S,
        path: &Path,
    ) -> Result<(ContextDocument, String)> {
        let content = storage.read(path)?;
        self.parse_content_with_body(&content, Some(path))
    }

    /// 파일에서 ContextDocument만 파싱
    pub fn parse_file<S: Storage + ?Sized>(
        &self,
        storage: &S,
        path: &Path,
    ) -> Result<ContextDocument> {
        let content = storage.read(path)?;
        self.parse_content(&content, Some(path))
    }

    /// 문자열 내용에서 ContextDocument와 본문을 함께 파싱
    pub fn parse_content_with_body(
        &self,
        content: &str,
        file_path: Option<&Path>,
    ) -> Result<(ContextDocument, String)> {
        // frontmatter 추출
        let (frontmatter, body) = self.frontmatter_parser.extract_frontmatter(content)?;

        // ContextDocument 직접 파싱
        let mut doc = self.parse_frontmatter_to_document(&frontmatter)?;

        // ID가 비어있으면 파일명에서 추출
        if doc.id.trim().is_empty() {
            if let Some(path) = file_path {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    doc.id = file_stem.to_uppercase();
                }
            }
        }

        // 파일 경로 설정
        if let Some(path) = file_path {
            doc.path = Some(path.to_path_buf());
        }

        // 스키마 검증
        doc.validate_schema()?;

        Ok((doc, body))
    }

    /// 문자열 내용에서 ContextDocument만 파싱
    pub fn parse_content(
        &self,
        content: &str,
        file_path: Option<&Path>,
    ) -> Result<ContextDocument> {
        let (doc, _body) = self.parse_content_with_body(content, file_path)?;
        Ok(doc)
    }

    /// frontmatter YAML을 ContextDocument로 파싱
    fn parse_frontmatter_to_document(&self, frontmatter: &str) -> Result<ContextDocument> {
        serde_yaml::from_str(frontmatter)
            .map_err(|e| ContextError::InvalidFrontmatter(format!("YAML parsing failed: {}", e)))
    }
}

impl Default for DocumentParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LocalFsStorage;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_parse_content_with_body() -> Result<()> {
        let parser = DocumentParser::new();
        let content = "---
id: \"TEST-DOC\"
title: \"Test Document\"
version: \"1.0.0\"
schema: \"context.v1\"
type: \"guide\"
tags: [\"test\"]
sections:
  - id: \"intro\"
    name: \"Introduction\"
    marker: \"## Introduction\"
    priority: 90
---

## Introduction
This is a test document.
";

        let (doc, body) = parser.parse_content_with_body(content, None)?;

        assert_eq!(doc.id, "TEST-DOC");
        assert_eq!(doc.title, "Test Document");
        assert_eq!(doc.sections.len(), 1);
        assert!(body.contains("This is a test document"));

        Ok(())
    }

    #[test]
    fn test_parse_file_with_storage() -> Result<()> {
        let temp_dir = TempDir::new().map_err(ContextError::Io)?;
        let storage = LocalFsStorage::new(temp_dir.path());
        let parser = DocumentParser::new();

        let content = "---
id: \"\"
title: \"File Test\"
version: \"1.0.0\"
schema: \"context.v1\"
type: \"guide\"
tags: [\"test\"]
sections:
  - id: \"content\"
    name: \"Content\"
    marker: \"## Content\"
    priority: 80
---

## Content
File content here.
";

        let file_path = PathBuf::from("test_doc.md");
        storage.write(&file_path, content)?;

        let (doc, body) = parser.parse_file_with_body(&storage, &file_path)?;

        // ID가 파일명에서 추출되었는지 확인
        assert_eq!(doc.id, "TEST_DOC");
        assert_eq!(doc.title, "File Test");
        assert!(body.contains("File content here"));

        Ok(())
    }
}
