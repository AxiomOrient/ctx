use regex::Regex;
use std::collections::HashSet;

/// 마크다운에서 제목과 본문 추출
pub fn extract_title_and_body(content: &str) -> (Option<String>, String) {
    let lines: Vec<&str> = content.lines().collect();

    // 첫 번째 # 헤딩을 찾기
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            let title = trimmed.trim_start_matches("# ").trim().to_string();
            let body = lines[(i + 1)..].join("\n");
            return (Some(title), body);
        }
    }

    // 헤딩이 없으면 전체를 본문으로
    (None, content.to_string())
}

/// 마크다운에서 링크 추출
pub fn extract_links(content: &str) -> Vec<String> {
    use once_cell::sync::Lazy;

    // 정규식 패턴을 상수로 정의하여 가독성 향상
    const LINK_PATTERN: &str = r"\[([^\]]+)\]\(([^)]+)\)";

    static LINK_REGEX: Lazy<Option<Regex>> = Lazy::new(|| Regex::new(LINK_PATTERN).ok());

    let mut links = Vec::new();

    // 정규식 컴파일에 실패한 경우 빈 벡터 반환 (graceful degradation)
    if let Some(regex) = LINK_REGEX.as_ref() {
        for cap in regex.captures_iter(content) {
            if let Some(url) = cap.get(2) {
                links.push(url.as_str().to_string());
            }
        }
    }

    links
}

/// 마크다운에서 코드 블록 언어 추출
pub fn extract_code_languages(content: &str) -> Vec<String> {
    use once_cell::sync::Lazy;

    // 정규식 패턴을 상수로 정의하여 가독성 향상
    const CODE_PATTERN: &str = r"```(\w+)";

    static CODE_REGEX: Lazy<Option<Regex>> = Lazy::new(|| Regex::new(CODE_PATTERN).ok());

    let mut languages = HashSet::new();

    // 정규식 컴파일에 실패한 경우 빈 벡터 반환 (graceful degradation)
    if let Some(regex) = CODE_REGEX.as_ref() {
        for cap in regex.captures_iter(content) {
            if let Some(lang) = cap.get(1) {
                languages.insert(lang.as_str().to_string());
            }
        }
    }

    languages.into_iter().collect()
}

/// 문자열을 안전한 파일명으로 변환
pub fn sanitize_filename(name: &str) -> String {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut result = String::new();

    for ch in name.chars() {
        if invalid_chars.contains(&ch) || ch.is_control() {
            result.push('_');
        } else {
            result.push(ch);
        }
    }

    // 길이 제한
    if result.len() > 100 {
        result.truncate(100);
    }

    result
}

/// 텍스트에서 태그 추출 (#tag 형식)
pub fn extract_hashtags(content: &str) -> Vec<String> {
    use std::sync::OnceLock;

    // 허용 문자: 유니코드 글자/숫자 + '_' '-' (일반적인 태그에 충분)
    // 링크나 단어 중간의 '#' 오탐을 줄이기 위해, '#' 바로 앞이 영숫자/밑줄/하이픈이 아닌 경우만 허용
    // 캡처 그룹 2가 실제 태그
    static TAG_REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    let regex_opt =
        TAG_REGEX.get_or_init(|| Regex::new(r"(?u)(^|[^A-Za-z0-9_-])#([\p{L}\p{N}_-]+)").ok());

    let mut tags = HashSet::new();
    if let Some(regex) = regex_opt.as_ref() {
        for cap in regex.captures_iter(content) {
            if let Some(tag) = cap.get(2) {
                tags.insert(tag.as_str().to_string());
            }
        }
    }

    tags.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title_and_body() {
        let content = r#"# Main Title

This is the body content.

## Sub heading
More content."#;

        let (title, body) = extract_title_and_body(content);
        assert_eq!(title, Some("Main Title".to_string()));
        assert!(body.contains("This is the body content"));
    }

    #[test]
    fn test_extract_links() {
        let content = "Check out [Google](https://google.com) and [GitHub](https://github.com)";
        let links = extract_links(content);
        assert_eq!(links.len(), 2);
        assert!(links.contains(&"https://google.com".to_string()));
        assert!(links.contains(&"https://github.com".to_string()));
    }

    #[test]
    fn test_extract_code_languages() {
        let content = r#"
```rust
fn main() {}
```

```python
print("hello")
```

```rust
// more rust
```
"#;
        let languages = extract_code_languages(content);
        assert!(languages.contains(&"rust".to_string()));
        assert!(languages.contains(&"python".to_string()));
        assert_eq!(languages.len(), 2); // rust should be deduplicated
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("hello<world>"), "hello_world_");
        assert_eq!(sanitize_filename("normal_name"), "normal_name");

        // Test length limit
        let long_name = "a".repeat(150);
        let sanitized = sanitize_filename(&long_name);
        assert!(sanitized.len() <= 100);
    }

    #[test]
    fn test_extract_hashtags() {
        let content = "This is #rust and #programming content with #ai.";
        let tags = extract_hashtags(content);
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"programming".to_string()));
        assert!(tags.contains(&"ai".to_string()));
        assert_eq!(tags.len(), 3);

        // 문장부호가 뒤따르는 경우 처리
        let content2 = "Edge cases: #rust, #swift. #go!";
        let tags2 = extract_hashtags(content2);
        assert!(tags2.contains(&"rust".to_string()));
        assert!(tags2.contains(&"swift".to_string()));
        assert!(tags2.contains(&"go".to_string()));

        // 유니코드 태그 지원 (간단 검증)
        let content3 = "한글태그: #테스트 와 혼합-태그: #ai-ml";
        let tags3 = extract_hashtags(content3);
        assert!(tags3.contains(&"테스트".to_string()));
        assert!(tags3.contains(&"ai-ml".to_string()));
    }
}
