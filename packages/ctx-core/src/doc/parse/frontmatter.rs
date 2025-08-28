use crate::common::constants::parsing::FRONTMATTER_DELIMITER;
use crate::{common::ContextMetadata, ContextError, Result};

#[derive(Debug, Default, Clone)]
pub struct FrontmatterParser;

impl FrontmatterParser {
    pub fn new() -> Self {
        Self
    }

    /// Markdown에서 frontmatter와 본문 분리
    pub fn parse(&self, content: &str) -> Result<(ContextMetadata, String)> {
        let (frontmatter, body) = self.extract_frontmatter(content)?;
        let metadata = self.parse_yaml(&frontmatter)?;
        self.validate_metadata(&metadata)?;

        Ok((metadata, body))
    }

    pub fn extract_frontmatter(&self, content: &str) -> Result<(String, String)> {
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() || lines[0].trim() != FRONTMATTER_DELIMITER {
            return Err(ContextError::InvalidFrontmatter(format!(
                "Document must start with '{}'",
                FRONTMATTER_DELIMITER
            )));
        }

        let end_index = lines[1..]
            .iter()
            .position(|line| line.trim() == FRONTMATTER_DELIMITER)
            .ok_or_else(|| {
                ContextError::InvalidFrontmatter(format!(
                    "Frontmatter must end with '{}'",
                    FRONTMATTER_DELIMITER
                ))
            })?;

        let frontmatter = lines[1..=end_index].join("\n");
        let body = lines[end_index + 2..].join("\n");

        Ok((frontmatter, body))
    }

    fn parse_yaml(&self, yaml: &str) -> Result<ContextMetadata> {
        serde_yaml::from_str(yaml)
            .map_err(|e| ContextError::InvalidFrontmatter(format!("YAML parsing failed: {}", e)))
    }

    fn validate_metadata(&self, metadata: &ContextMetadata) -> Result<()> {
        use crate::common::constants::validation::SEMVER_PARTS_COUNT;

        // 제목 검증
        if metadata.title.trim().is_empty() {
            return Err(ContextError::InvalidFrontmatter(
                "Title cannot be empty".to_string(),
            ));
        }

        // 버전 검증 (간단한 semver 체크)
        let version_parts: Vec<&str> = metadata.version.split('.').collect();
        if version_parts.len() != SEMVER_PARTS_COUNT {
            return Err(ContextError::InvalidFrontmatter(
                "Version must be in semver format (e.g., '1.0.0')".to_string(),
            ));
        }

        // 섹션 검증
        if metadata.sections.is_empty() {
            return Err(ContextError::InvalidFrontmatter(
                "At least one section must be defined".to_string(),
            ));
        }

        // 섹션 ID 중복 검사
        let mut seen_ids = std::collections::HashSet::new();
        for section in &metadata.sections {
            if !seen_ids.insert(&section.id) {
                return Err(ContextError::InvalidFrontmatter(format!(
                    "Duplicate section ID: '{}'",
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

    #[test]
    fn test_valid_frontmatter() -> crate::Result<()> {
        let content = "---
title: \"Test Document\"
version: \"1.0.0\"
sections:
  - id: \"intro\"
    name: \"Introduction\"
    marker: \"## Introduction\"
    priority: 90
---

## Introduction
This is the introduction.
";

        let parser = FrontmatterParser::new();
        let result = parser.parse(content);

        let (metadata, body) = result?;
        assert_eq!(metadata.title, "Test Document");
        assert_eq!(metadata.sections.len(), 1);
        assert!(body.contains("This is the introduction"));
        Ok(())
    }

    #[test]
    fn test_invalid_frontmatter() {
        let content = "---
title: \"\"
version: \"1.0.0\"
sections: []
---";

        let parser = FrontmatterParser::new();
        let result = parser.parse(content);

        assert!(result.is_err());
    }
}
