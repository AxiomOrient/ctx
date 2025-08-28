//! CLI 공통 유틸리티 함수들
//!
//! 모든 CLI 명령어에서 공통으로 사용되는 헬퍼 함수들을 제공합니다.



/// 출력 포맷 처리 헬퍼 - 모든 명령어에서 공통 사용
///
/// # Arguments
/// * `data` - 직렬화할 데이터
/// * `format` - 출력 형식 ("json", "yaml", "human")
///
/// # Returns
/// * 포맷된 문자열 또는 에러
pub fn format_output<T: serde::Serialize>(data: &T, format: &str) -> ctx_core::Result<String> {
    match format {
        "json" => serde_json::to_string_pretty(data)
            .map_err(|e| ctx_core::ContextError::AssemblyError(format!("JSON serialization failed: {}", e))),
        "yaml" => serde_yaml::to_string(data)
            .map_err(|e| ctx_core::ContextError::AssemblyError(format!("YAML serialization failed: {}", e))),
        "human" => Ok(String::new()), // human 포맷은 각 함수에서 직접 처리
        _ => Err(ctx_core::ContextError::AssemblyError(format!(
            "Unsupported format: {}. Use 'json', 'yaml', or 'human'",
            format
        ))),
    }
}

/// 태그 문자열을 Vec<String>으로 파싱
///
/// # Arguments  
/// * `tags_str` - 쉼표로 구분된 태그 문자열 ("tag1,tag2,tag3")
///
/// # Returns
/// * 파싱된 태그 벡터 또는 None
///
/// # Example
/// ```
/// use crate::app::cli::utils::parse_tags;
///
/// let tags = parse_tags(Some("rust,backend,api".to_string()));
/// assert_eq!(tags, Some(vec!["rust".to_string(), "backend".to_string(), "api".to_string()]));
/// ```
pub fn parse_tags(tags_str: Option<String>) -> Option<Vec<String>> {
    tags_str.map(|s| s.split(',').map(|tag| tag.trim().to_string()).collect())
}



#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_format_output_json() -> ctx_core::Result<()> {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        let result = format_output(&data, "json")?;
        assert!(result.contains("\"name\": \"test\""));
        assert!(result.contains("\"value\": 42"));
        Ok(())
    }

    #[test]
    fn test_format_output_yaml() -> ctx_core::Result<()> {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        let result = format_output(&data, "yaml")?;
        assert!(result.contains("name: test"));
        assert!(result.contains("value: 42"));
        Ok(())
    }

    #[test]
    fn test_format_output_invalid() {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        let result = format_output(&data, "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_tags_some() {
        let result = parse_tags(Some("rust,backend,api".to_string()));
        assert_eq!(
            result,
            Some(vec![
                "rust".to_string(),
                "backend".to_string(),
                "api".to_string()
            ])
        );
    }

    #[test]
    fn test_parse_tags_with_spaces() {
        let result = parse_tags(Some(" rust , backend , api ".to_string()));
        assert_eq!(
            result,
            Some(vec![
                "rust".to_string(),
                "backend".to_string(),
                "api".to_string()
            ])
        );
    }

    #[test]
    fn test_parse_tags_none() {
        let result = parse_tags(None);
        assert_eq!(result, None);
    }
}
