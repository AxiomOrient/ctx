use crate::knowledge::ontology::OntologyRegistry;
use std::collections::HashMap;

/// 토큰/동의어/패스 정규화기
pub struct Normalizer<'a> {
    ontology: &'a OntologyRegistry,
    synonym_cache: HashMap<String, String>,
}

impl<'a> Normalizer<'a> {
    pub fn new(ontology: &'a OntologyRegistry) -> Self {
        Self {
            ontology,
            synonym_cache: HashMap::new(),
        }
    }

    /// 토큰 정규화 (소문자, 공백 제거 등)
    pub fn normalize_token(&self, token: &str) -> String {
        token.to_lowercase().trim().replace(['-', '_'], "")
    }

    /// 네임스페이스-값 쌍 정규화
    pub fn normalize_facet(&mut self, namespace: &str, value: &str) -> String {
        let cache_key = format!("{}:{}", namespace, value);

        if let Some(cached) = self.synonym_cache.get(&cache_key) {
            return cached.clone();
        }

        let normalized_value = self.normalize_token(value);

        // 온톨로지를 통한 정규화
        let canonical = self
            .ontology
            .normalize(namespace, &normalized_value)
            .unwrap_or(normalized_value);

        // 캐시에 저장
        self.synonym_cache.insert(cache_key, canonical.clone());

        canonical
    }

    /// 경로 정규화 (파일 경로에서 의미있는 부분 추출)
    pub fn normalize_path(&self, path: &str) -> Vec<String> {
        path.split(['/', '\\'])
            .filter(|part| !part.is_empty())
            .map(|part| self.normalize_token(part))
            .collect()
    }

    /// URL 호스트 정규화
    pub fn normalize_host(&self, url: &str) -> Option<String> {
        // 간단한 호스트 추출 시도 (프로토콜 포함/미포함 모두 지원)
        if let Some(start) = url.find("://") {
            let after_protocol = &url[start + 3..];
            if let Some(end) = after_protocol.find('/') {
                return Some(self.normalize_token(&after_protocol[..end]));
            } else {
                return Some(self.normalize_token(after_protocol));
            }
        }
        // 프로토콜이 없는 경우, 첫 슬래시 전까지를 호스트로 간주
        if let Some(end) = url.find('/') {
            return Some(self.normalize_token(&url[..end]));
        }
        if !url.is_empty() {
            return Some(self.normalize_token(url));
        }

        None
    }

    /// 코드 언어 정규화
    pub fn normalize_language(&self, lang: &str) -> String {
        let normalized = self.normalize_token(lang);

        // 일반적인 언어 별칭 처리
        match normalized.as_str() {
            "js" => "javascript".to_string(),
            "ts" => "typescript".to_string(),
            "py" => "python".to_string(),
            "rb" => "ruby".to_string(),
            "sh" | "bash" => "shell".to_string(),
            "yml" => "yaml".to_string(),
            _ => normalized,
        }
    }

    /// 캐시 통계
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.synonym_cache.len(), self.synonym_cache.capacity())
    }

    /// 캐시 클리어
    pub fn clear_cache(&mut self) {
        self.synonym_cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::ontology::{OntologyRegistry, schema::*};
    use std::collections::HashMap;

    #[allow(clippy::unwrap_used)] // 테스트 코드에서는 허용
    fn create_test_ontology() -> OntologyRegistry {
        let mut namespaces = HashMap::new();

        let mut tech_values = HashMap::new();
        tech_values.insert(
            "swiftui".to_string(),
            OntoValue {
                synonyms: vec!["swift-ui".to_string(), "SwiftUI".to_string()],
                parents: vec![],
                deprecated: false,
                description: None,
            },
        );

        namespaces.insert(
            "tech".to_string(),
            Namespace {
                values: tech_values,
            },
        );

        let ontology = Ontology { namespaces };
        OntologyRegistry::from_ontology(ontology).unwrap()
    }

    #[test]
    fn test_normalize_token() {
        let ontology = create_test_ontology();
        let normalizer = Normalizer::new(&ontology);

        assert_eq!(normalizer.normalize_token("SwiftUI"), "swiftui");
        assert_eq!(normalizer.normalize_token("Swift-UI"), "swiftui");
        assert_eq!(normalizer.normalize_token("swift_ui"), "swiftui");
        assert_eq!(normalizer.normalize_token(" SwiftUI "), "swiftui");
    }

    #[test]
    fn test_normalize_language() {
        let ontology = create_test_ontology();
        let normalizer = Normalizer::new(&ontology);

        assert_eq!(normalizer.normalize_language("js"), "javascript");
        assert_eq!(normalizer.normalize_language("ts"), "typescript");
        assert_eq!(normalizer.normalize_language("py"), "python");
        assert_eq!(normalizer.normalize_language("Swift"), "swift");
    }

    #[test]
    fn test_normalize_host() {
        let ontology = create_test_ontology();
        let normalizer = Normalizer::new(&ontology);

        assert_eq!(
            normalizer.normalize_host("https://developer.apple.com/documentation"),
            Some("developer.apple.com".to_string())
        );
        assert_eq!(
            normalizer.normalize_host("http://github.com/user/repo"),
            Some("github.com".to_string())
        );
    }

    #[test]
    fn test_normalize_path() {
        let ontology = create_test_ontology();
        let normalizer = Normalizer::new(&ontology);

        let path_parts = normalizer.normalize_path("src/iOS/SwiftUI/Views");
        assert_eq!(path_parts, vec!["src", "ios", "swiftui", "views"]);
    }
}
