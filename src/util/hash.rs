//! 결정성 엔진 - 유사-안정 캐싱 시스템
//!
//! 이 모듈은 동일한 입력에 대해 동일한 결과를, 유사한 입력에 대해 유사한 결과를
//! 제공하는 결정성 엔진을 구현합니다.

use crate::domain::constants::determinism::regex_patterns;
use crate::domain::constants::determinism::*;
use crate::domain::errors::ContextError;
use regex::Regex;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 정규화된 입력을 나타내는 구조체
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalInput {
    /// 정규화된 텍스트 내용
    pub text: String,
    /// 정규화된 패싯 리스트 (정렬됨)
    pub facets: Vec<String>,
    /// 정규화된 메타데이터
    pub metadata: Vec<(String, String)>,
    /// 정규화 버전 (호환성 추적)
    pub normalization_version: String,
}

/// 실행 컨텍스트 정보
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// 고유 실행 ID
    pub execution_id: String,
    /// 정규화된 입력의 해시
    pub canonical_hash: String,
    /// 버전 정보
    pub version_hash: String,
    /// 생성 시각 (타임스탬프)
    pub created_at: i64,
}

/// 결정성 엔진 - execution ID 생성 및 정규화 처리
pub struct DeterminismEngine {
    /// 공백 정규화 정규식
    whitespace_regex: Regex,
    /// 마크다운 헤더 정규식
    header_regex: Regex,
    /// 코드 펜스 정규식
    code_fence_regex: Regex,
    /// URL 정규화 정규식
    url_regex: Regex,
    /// 현재 정규화 버전
    normalization_version: String,
}

impl DeterminismEngine {
    /// 새로운 DeterminismEngine 인스턴스 생성
    pub fn new() -> Result<Self, ContextError> {
        let whitespace_regex = Regex::new(regex_patterns::WHITESPACE_NORMALIZE)
            .map_err(|e| ContextError::ConfigError(format!("Invalid whitespace regex: {}", e)))?;

        let header_regex = Regex::new(regex_patterns::MARKDOWN_HEADERS)
            .map_err(|e| ContextError::ConfigError(format!("Invalid header regex: {}", e)))?;

        let code_fence_regex = Regex::new(regex_patterns::CODE_FENCES)
            .map_err(|e| ContextError::ConfigError(format!("Invalid code fence regex: {}", e)))?;

        let url_regex = Regex::new(regex_patterns::TRAILING_SLASHES)
            .map_err(|e| ContextError::ConfigError(format!("Invalid URL regex: {}", e)))?;

        Ok(Self {
            whitespace_regex,
            header_regex,
            code_fence_regex,
            url_regex,
            normalization_version: "1.0".to_string(),
        })
    }

    /// 입력 텍스트와 메타데이터를 정규화하여 CanonicalInput 생성
    pub fn normalize_input(
        &self,
        text: &str,
        facets: &[String],
        metadata: &[(String, String)],
    ) -> Result<CanonicalInput, ContextError> {
        // 입력 크기 제한 확인
        if text.len() > MAX_INPUT_SIZE_BYTES {
            return Err(ContextError::Other(format!(
                "Input size {} exceeds maximum allowed size {}",
                text.len(),
                MAX_INPUT_SIZE_BYTES
            )));
        }

        // 텍스트 정규화
        let normalized_text = self.normalize_text(text)?;

        // 패싯 정규화 (정렬 및 중복 제거)
        let mut normalized_facets: Vec<String> = facets
            .iter()
            .map(|f| f.trim().to_lowercase())
            .filter(|f| !f.is_empty())
            .collect();
        normalized_facets.sort_unstable();
        normalized_facets.dedup();

        // 메타데이터 정규화 (키-값 정렬)
        let mut normalized_metadata: Vec<(String, String)> = metadata
            .iter()
            .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
            .filter(|(k, _)| !k.is_empty())
            .collect();
        normalized_metadata.sort_unstable_by(|a, b| a.0.cmp(&b.0));

        Ok(CanonicalInput {
            text: normalized_text,
            facets: normalized_facets,
            metadata: normalized_metadata,
            normalization_version: self.normalization_version.clone(),
        })
    }

    /// 텍스트 내용 정규화
    fn normalize_text(&self, text: &str) -> Result<String, ContextError> {
        let mut normalized = text.to_string();

        // 공백 정규화
        if NORMALIZE_WHITESPACE {
            normalized = self
                .whitespace_regex
                .replace_all(&normalized, " ")
                .to_string();
        }

        // 마크다운 헤더 정규화
        if NORMALIZE_MARKDOWN_HEADERS {
            normalized = self.header_regex.replace_all(&normalized, "# ").to_string();
        }

        // 코드 펜스 정규화
        if NORMALIZE_CODE_FENCES {
            normalized = self
                .code_fence_regex
                .replace_all(&normalized, "```\n")
                .to_string();
        }

        // URL 정규화
        if NORMALIZE_URLS {
            normalized = self
                .url_regex
                .replace_all(&normalized, |caps: &regex::Captures| {
                    let url = caps.get(0).map_or("", |m| m.as_str());
                    url.trim_end_matches('/').to_string()
                })
                .to_string();
        }

        // 길이 제한 적용
        if normalized.len() > MAX_NORMALIZED_TEXT_LENGTH {
            normalized.truncate(MAX_NORMALIZED_TEXT_LENGTH);
        }

        normalized = normalized.trim().to_string();

        Ok(normalized)
    }

    /// CanonicalInput에서 실행 ID 생성
    pub fn generate_execution_id(&self, canonical_input: &CanonicalInput) -> String {
        let mut hasher = DefaultHasher::new();

        // 텍스트 해시
        canonical_input.text.hash(&mut hasher);

        // 패싯 해시
        for facet in &canonical_input.facets {
            facet.hash(&mut hasher);
        }

        // 메타데이터 해시
        for (key, value) in &canonical_input.metadata {
            key.hash(&mut hasher);
            value.hash(&mut hasher);
        }

        // 정규화 버전 해시
        canonical_input.normalization_version.hash(&mut hasher);

        let hash = hasher.finish();
        format!("{:016x}", hash)[..EXECUTION_ID_LENGTH].to_string()
    }

    /// CanonicalInput의 내용 해시 생성
    pub fn generate_canonical_hash(&self, canonical_input: &CanonicalInput) -> String {
        let mut hasher = DefaultHasher::new();
        canonical_input.hash(&mut hasher);
        let hash = hasher.finish();
        format!("{:016x}", hash)[..CANONICAL_INPUT_HASH_LENGTH].to_string()
    }

    /// 시스템 버전 해시 생성 (상수 및 알고리즘 버전 기반)
    pub fn generate_version_hash(&self) -> String {
        let mut hasher = DefaultHasher::new();

        // 중요한 상수들 해시
        SIMILARITY_THRESHOLD.to_bits().hash(&mut hasher);
        FACET_SIMILARITY_WEIGHT.to_bits().hash(&mut hasher);
        TEXT_SIMILARITY_WEIGHT.to_bits().hash(&mut hasher);
        STRUCT_SIMILARITY_WEIGHT.to_bits().hash(&mut hasher);
        TRIGRAM_SIMILARITY_THRESHOLD.to_bits().hash(&mut hasher);

        // 정규화 설정 해시
        NORMALIZE_WHITESPACE.hash(&mut hasher);
        NORMALIZE_MARKDOWN_HEADERS.hash(&mut hasher);
        NORMALIZE_CODE_FENCES.hash(&mut hasher);
        NORMALIZE_URLS.hash(&mut hasher);

        // 정규화 버전 해시
        self.normalization_version.hash(&mut hasher);

        let hash = hasher.finish();
        format!("{:016x}", hash)[..VERSION_HASH_LENGTH].to_string()
    }

    /// 완전한 ExecutionContext 생성
    pub fn create_execution_context(
        &self,
        text: &str,
        facets: &[String],
        metadata: &[(String, String)],
    ) -> Result<ExecutionContext, ContextError> {
        let canonical_input = self.normalize_input(text, facets, metadata)?;
        let execution_id = self.generate_execution_id(&canonical_input);
        let canonical_hash = self.generate_canonical_hash(&canonical_input);
        let version_hash = self.generate_version_hash();

        // 현재 시각을 Unix 타임스탬프로
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| ContextError::Other(format!("Failed to get timestamp: {}", e)))?
            .as_secs() as i64;

        Ok(ExecutionContext {
            execution_id,
            canonical_hash,
            version_hash,
            created_at,
        })
    }
}

impl DeterminismEngine {
    /// 기본 엔진 생성 - Default trait 대신 명시적 메서드 사용
    /// 이 방법이 Result를 반환하므로 더 안전함
    pub fn try_default() -> Result<Self, ContextError> {
        Self::new()
    }
}

// Hash trait을 CanonicalInput에 구현
impl Hash for CanonicalInput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        for facet in &self.facets {
            facet.hash(state);
        }
        for (key, value) in &self.metadata {
            key.hash(state);
            value.hash(state);
        }
        self.normalization_version.hash(state);
    }
}

pub fn deterministic_hash(input: &str) -> String {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{:016x}", hash)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_determinism_engine_creation() {
        let engine = DeterminismEngine::new();
        assert!(engine.is_ok());
    }

    #[test]
    fn test_text_normalization() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");

        let input = "  This   is   \n\n  test   text  ";
        let normalized = engine
            .normalize_text(input)
            .expect("Failed to normalize text for test");
        assert_eq!(normalized, "This is test text");
    }

    #[test]
    fn test_markdown_header_normalization() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");

        let input = "## Header\n### Another Header\n#### Small Header";
        let normalized = engine
            .normalize_text(input)
            .expect("Failed to normalize text for test");
        assert!(normalized.contains("# Header"));
        assert!(normalized.contains("# Another Header"));
        assert!(normalized.contains("# Small Header"));
    }

    #[test]
    fn test_canonical_input_creation() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");

        let text = "Test content";
        let facets = vec!["rust".to_string(), "api".to_string(), "test".to_string()];
        let metadata = vec![
            ("author".to_string(), "test".to_string()),
            ("version".to_string(), "1.0".to_string()),
        ];

        let canonical = engine
            .normalize_input(text, &facets, &metadata)
            .expect("Failed to normalize input for test");

        assert_eq!(canonical.text, "Test content");
        assert_eq!(canonical.facets, vec!["api", "rust", "test"]); // 정렬됨
        assert_eq!(canonical.metadata.len(), 2);
        assert_eq!(canonical.normalization_version, "1.0");
    }

    #[test]
    fn test_execution_id_generation() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");

        let text = "Test content";
        let facets = vec!["rust".to_string()];
        let metadata = vec![];

        let canonical = engine
            .normalize_input(text, &facets, &metadata)
            .expect("Failed to normalize input for test");
        let execution_id = engine.generate_execution_id(&canonical);

        assert_eq!(execution_id.len(), EXECUTION_ID_LENGTH);
        assert!(execution_id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_execution_id_deterministic() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");

        let text = "Test content";
        let facets = vec!["rust".to_string()];
        let metadata = vec![];

        let canonical1 = engine
            .normalize_input(text, &facets, &metadata)
            .expect("Failed to normalize input for test");
        let canonical2 = engine
            .normalize_input(text, &facets, &metadata)
            .expect("Failed to normalize input for test");

        let id1 = engine.generate_execution_id(&canonical1);
        let id2 = engine.generate_execution_id(&canonical2);

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_execution_id_different_for_different_inputs() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");

        let canonical1 = engine
            .normalize_input("content1", &[], &[])
            .expect("Failed to normalize input for test");
        let canonical2 = engine
            .normalize_input("content2", &[], &[])
            .expect("Failed to normalize input for test");

        let id1 = engine.generate_execution_id(&canonical1);
        let id2 = engine.generate_execution_id(&canonical2);

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_version_hash_generation() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");
        let version_hash = engine.generate_version_hash();

        assert_eq!(version_hash.len(), VERSION_HASH_LENGTH);
        assert!(version_hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_execution_context_creation() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");

        let context = engine
            .create_execution_context(
                "test content",
                &["rust".to_string()],
                &[("key".to_string(), "value".to_string())],
            )
            .expect("Failed to create execution context for test");

        assert_eq!(context.execution_id.len(), EXECUTION_ID_LENGTH);
        assert_eq!(context.canonical_hash.len(), CANONICAL_INPUT_HASH_LENGTH);
        assert_eq!(context.version_hash.len(), VERSION_HASH_LENGTH);
        assert!(context.created_at > 0);
    }

    #[test]
    fn test_input_size_limit() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");

        let large_text = "x".repeat(MAX_INPUT_SIZE_BYTES + 1);
        let result = engine.normalize_input(&large_text, &[], &[]);

        assert!(result.is_err());
        if let Err(ContextError::Other(msg)) = result {
            assert!(msg.contains("exceeds maximum allowed size"));
        } else {
            panic!("Expected Other error");
        }
    }

    #[test]
    fn test_facets_normalization_and_deduplication() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");

        let facets = vec![
            "Rust".to_string(),
            "API".to_string(),
            "rust".to_string(),     // 중복
            "".to_string(),         // 빈 문자열
            "  test  ".to_string(), // 공백 포함
        ];

        let canonical = engine
            .normalize_input("test", &facets, &[])
            .expect("Failed to normalize input for test");

        assert_eq!(canonical.facets, vec!["api", "rust", "test"]);
    }

    #[test]
    fn test_metadata_normalization() {
        let engine = DeterminismEngine::new().expect("Failed to create engine for test");

        let metadata = vec![
            ("Author".to_string(), "John Doe".to_string()),
            ("VERSION".to_string(), "1.0".to_string()),
            ("".to_string(), "should be filtered".to_string()),
            ("  key  ".to_string(), "  value  ".to_string()),
        ];

        let canonical = engine
            .normalize_input("test", &[], &metadata)
            .expect("Failed to normalize input for test");

        // 빈 키는 필터링되고, 키는 소문자로, 값은 트림됨, 정렬됨
        assert_eq!(canonical.metadata.len(), 3);
        assert_eq!(
            canonical.metadata[0],
            ("author".to_string(), "John Doe".to_string())
        );
        assert_eq!(
            canonical.metadata[1],
            ("key".to_string(), "value".to_string())
        );
        assert_eq!(
            canonical.metadata[2],
            ("version".to_string(), "1.0".to_string())
        );
    }
}
