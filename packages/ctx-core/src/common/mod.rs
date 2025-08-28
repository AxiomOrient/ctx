use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

pub mod constants;
pub mod errors;
pub mod rule_application;
pub mod utils;

// Re-export commonly used constants (개별적으로 중복 방지)
pub use constants::composition::{
    COMPOSITION_CONFIDENCE_WEIGHTS, COMPOSITION_MMR_LAMBDA, COMPOSITION_TOKEN_BUDGET,
    DEFAULT_TOKEN_RESERVE, DIVERSITY_SIMILARITY_THRESHOLD, GREEDY_SELECTION_THRESHOLD,
    MAX_CANDIDATES_TO_SCORE, MAX_DOCUMENT_TOKENS, MAX_MERGED_SECTIONS, MAX_SOURCE_DOCS_IN_METADATA,
    MAX_TOKEN_BUDGET, MIN_TOKEN_BUDGET, SECTION_SEPARATOR,
};
pub use constants::defaults::{
    DEFAULT_CACHE_SIZE,
    DEFAULT_CLASSIFICATION_CONFIDENCE,
    DEFAULT_LOCALE,
    DEFAULT_SCHEMA_VERSION,
    DEFAULT_TOKEN_BUDGET as DEFAULTS_TOKEN_BUDGET, // 별칭으로 중복 방지
    DEFAULT_TRUST,
    EXACT_ALGORITHM_THRESHOLD,
    HIGH_CONFIDENCE_THRESHOLD,
    MEDIUM_CONFIDENCE_THRESHOLD,
    SUMMARY_MAX_LENGTH,
    TOKEN_ESTIMATION_MULTIPLIER,
};
pub use constants::determinism::{
    FACET_SIMILARITY_WEIGHT, SIMILARITY_THRESHOLD as DETERMINISM_SIMILARITY_THRESHOLD,
    STRUCT_SIMILARITY_WEIGHT, TEXT_SIMILARITY_WEIGHT,
};
pub use constants::graph::*;
pub use constants::parsing::*;
pub use constants::scoring::{
    DEFAULT_AXES_MATCH_WEIGHT, DEFAULT_CONFIDENCE_THRESHOLD, DEFAULT_CONFIDENCE_WEIGHT,
    DEFAULT_FACET_COVERAGE_WEIGHT, DEFAULT_FRESHNESS_TAU_DAYS, DEFAULT_FRESHNESS_WEIGHT,
    DEFAULT_KEYWORD_WEIGHT, DEFAULT_MMR_LAMBDA as SCORING_MMR_LAMBDA, DEFAULT_TRUST_WEIGHT,
    KNAPSACK_BUDGET_THRESHOLD,
};
pub use constants::validation::SIMILARITY_THRESHOLD as VALIDATION_SIMILARITY_THRESHOLD;

/// 컨텍스트 메타데이터 (기존 호환성 유지)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextMetadata {
    pub title: String,
    pub version: String,
    #[serde(default = "default_priority")]
    pub priority: u8,
    pub estimated_tokens: Option<u32>,
    pub sections: Vec<SectionDef>,
}

/// 확장된 컨텍스트 문서 (contexts/ 폴더 관리용)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextDocument {
    pub id: String, // 전역 고유 ID
    pub title: String,
    pub version: String,
    #[serde(default = "default_schema")]
    pub schema: String, // "context.v1"
    pub r#type: String, // guide|persona|workflow|domain
    #[serde(default)]
    pub domain: Option<String>, // rust, python, ai 등
    pub tags: Vec<String>,

    // 새로운 패싯 시스템 (v2 스키마)
    #[serde(default)]
    pub facets: HashMap<String, Vec<String>>, // namespace -> values
    #[serde(default)]
    pub locale: Option<String>, // 출력 언어
    #[serde(default)]
    pub trust: Option<f32>, // 신뢰도 (0.0-1.0)
    #[serde(default)]
    pub freshness: Option<String>, // ISO 날짜
    #[serde(default)]
    pub aliases: Option<Vec<String>>, // 별칭들
    #[serde(default)]
    pub conflicts: Option<Vec<String>>, // 충돌하는 문서 ID들

    #[serde(default)]
    pub dependencies: Vec<String>, // 다른 문서 ID 참조
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
    #[serde(default)]
    pub estimated_tokens: Option<u32>,
    pub sections: Vec<SectionDef>,

    // 런타임 정보 (직렬화 제외)
    #[serde(skip)]
    pub path: Option<PathBuf>, // 파일 경로
}

/// 검색 쿼리
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub tags: Option<Vec<String>>,
    pub types: Option<Vec<String>>,
    pub domains: Option<Vec<String>>,
    pub text_match: Option<String>,
    pub updated_after: Option<DateTime<Utc>>,
    pub author: Option<String>,
}

/// 인덱스 엔트리
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexEntry {
    pub doc: ContextDocument,
    pub path: PathBuf,
    pub summary: String, // 첫 번째 섹션 요약
    pub last_checked: DateTime<Utc>,
}

/// 섹션 정의
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SectionDef {
    pub id: String,
    pub name: String,
    pub marker: String,
    #[serde(default = "default_priority")]
    pub priority: u8,
    pub tokens: Option<u32>,
}

/// 추출된 섹션
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedSection {
    pub id: String,
    pub name: String,
    pub content: String,
    pub tokens: u32,
    pub priority: u8,
    pub score: f32,
}

/// 조립된 프롬프트
#[derive(Debug, Clone, serde::Serialize)]
pub struct AssembledPrompt {
    pub content: String,
    pub sections_used: Vec<String>,
    pub total_tokens: u32,
    pub generated_at: DateTime<Utc>,
    pub metadata: AssemblyMetadata,
}

/// 조립 메타데이터
#[derive(Debug, Clone, serde::Serialize)]
pub struct AssemblyMetadata {
    pub source_files: Vec<String>,
    pub token_budget: u32,
    pub optimization_method: OptimizationMethod,
    pub cache_hit: bool,
}

/// 최적화 방법
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationMethod {
    Exact,  // DP 정확 해법
    Greedy, // 그리디 근사
}

/// 조립 요청
#[derive(Debug, Clone)]
pub struct AssemblyRequest {
    pub content: String,
    pub token_budget: u32,
    pub section_filter: Option<Vec<String>>,
}

/// 빌드 쿼리 - 단일 진실 원천
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildQuery {
    pub repo: String,
    pub branch: String,
    pub commit_sha: String,
    pub required_facets: Vec<(String, Vec<String>)>, // namespace -> allowed values
    pub lang: Option<String>,
    pub maturity: Option<String>,
    pub axes: Option<Vec<String>>,  // artifact types
    pub query_text: Option<String>, // 키워드 검색
    pub include_sources: Option<bool>,
    pub confidence_threshold: f32,
}

/// 문서 후보 - 단일 진실 원천 (상위 호환)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCandidate {
    pub doc_id: String,
    pub sha: String,
    pub title: String,
    pub locale: String,
    pub trust: f32,
    pub freshness: String, // ISO date
    pub confidence: f32,
    pub path: String,
    pub facets: HashMap<String, Vec<String>>,
    pub tokens: usize,
    pub content: Option<String>, // 실제 문서 내용 (필요시 로드)
}

/// 토큰 계산 트레이트
pub trait Tokenizer {
    fn estimate_tokens(&self, content: &str) -> u32;
}

/// 단순 토크나이저 구현
pub struct SimpleTokenizer;

impl Tokenizer for SimpleTokenizer {
    fn estimate_tokens(&self, content: &str) -> u32 {
        // 단어 수 기반 간단 추정
        let word_count = content.split_whitespace().count();
        ((word_count as f32) * crate::common::constants::defaults::TOKEN_ESTIMATION_MULTIPLIER)
            .ceil() as u32
    }
}

fn default_priority() -> u8 {
    crate::common::constants::parsing::DEFAULT_PRIORITY
}

fn default_schema() -> String {
    crate::common::constants::defaults::DEFAULT_SCHEMA_VERSION.to_string()
}

impl Default for ContextMetadata {
    fn default() -> Self {
        Self {
            title: "Untitled".to_string(),
            version: "1.0.0".to_string(),
            priority: crate::common::constants::parsing::DEFAULT_PRIORITY,
            estimated_tokens: None,
            sections: Vec::new(),
        }
    }
}

impl Default for AssemblyRequest {
    fn default() -> Self {
        Self {
            content: String::new(),
            token_budget: crate::common::constants::defaults::DEFAULT_TOKEN_BUDGET,
            section_filter: None,
        }
    }
}

impl BuildQuery {
    pub fn new(repo: String, branch: String, commit_sha: String) -> Self {
        Self {
            repo,
            branch,
            commit_sha,
            required_facets: Vec::new(),
            lang: None,
            maturity: None,
            axes: None,
            query_text: None,
            include_sources: Some(true),
            confidence_threshold: crate::common::constants::scoring::DEFAULT_CONFIDENCE_THRESHOLD,
        }
    }

    /// 패싯 요구사항 추가
    pub fn with_facet(mut self, namespace: String, values: Vec<String>) -> Self {
        self.required_facets.push((namespace, values));
        self
    }

    /// 언어 필터 설정
    pub fn with_language(mut self, lang: String) -> Self {
        self.lang = Some(lang);
        self
    }

    /// 성숙도 필터 설정
    pub fn with_maturity(mut self, maturity: String) -> Self {
        self.maturity = Some(maturity);
        self
    }

    /// 아티팩트 타입 설정
    pub fn with_axes(mut self, axes: Vec<String>) -> Self {
        self.axes = Some(axes);
        self
    }

    /// 키워드 검색 설정
    pub fn with_query_text(mut self, text: String) -> Self {
        self.query_text = Some(text);
        self
    }

    /// 신뢰도 임계값 설정
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
    }
}

impl DocumentCandidate {
    /// 특정 패싯 값을 가지고 있는지 확인
    pub fn has_facet(&self, namespace: &str, value: &str) -> bool {
        self.facets
            .get(namespace)
            .map(|values| values.contains(&value.to_string()))
            .unwrap_or(false)
    }

    /// 패싯 집합을 문자열 집합으로 변환 (유사도 계산용)
    pub fn facet_set(&self) -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        for (namespace, values) in &self.facets {
            for value in values {
                set.insert(format!("{}:{}", namespace, value));
            }
        }
        set
    }

    /// 문서 내용 로드 (지연 로딩)
    pub fn load_content(&mut self) -> crate::Result<&str> {
        if self.content.is_none() {
            let content = std::fs::read_to_string(&self.path).map_err(crate::ContextError::Io)?;
            self.content = Some(content);
        }
        self.content
            .as_ref()
            .ok_or_else(|| crate::ContextError::Other("Content not loaded".to_string()))
            .map(|s| s.as_str())
    }

    /// 신선도를 일 단위로 계산
    pub fn days_since_freshness(&self) -> Option<i64> {
        use chrono::{DateTime, Utc};

        if let Ok(freshness_date) = DateTime::parse_from_rfc3339(&self.freshness) {
            let now = Utc::now();
            let duration = now.signed_duration_since(freshness_date.with_timezone(&Utc));
            Some(duration.num_days())
        } else {
            None
        }
    }
}

impl From<ContextMetadata> for ContextDocument {
    fn from(metadata: ContextMetadata) -> Self {
        Self {
            id: format!("UNTITLED-{}", chrono::Utc::now().timestamp()),
            title: metadata.title,
            version: metadata.version,
            schema: crate::common::constants::defaults::DEFAULT_SCHEMA_VERSION.to_string(),
            r#type: "guide".to_string(),
            domain: None,
            tags: Vec::new(),
            facets: HashMap::new(),
            locale: None,
            trust: None,
            freshness: None,
            aliases: None,
            conflicts: None,
            dependencies: Vec::new(),
            author: None,
            updated: Some(chrono::Utc::now()),
            estimated_tokens: metadata.estimated_tokens,
            sections: metadata.sections,
            path: None,
        }
    }
}

/// 조합 결과 - 완전한 프롬프트 조합 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionResult {
    /// 병합된 최종 문서
    pub merged_document: MergedDocument,

    /// 선택 근거 설명
    pub selection_rationale: String,

    /// 사용된 총 토큰 수
    pub total_tokens_used: usize,

    /// 조합 신뢰도 점수 (0.0-1.0)
    pub confidence_score: f32,

    /// 선택에서 제외된 문서들
    pub rejected_documents: Vec<DocumentCandidate>,

    /// 실행 ID (결정성 보장용)
    pub execution_id: String,

    /// 유사성 재사용 여부 및 점수
    pub similarity_used: Option<f32>,

    /// 조합 생성 시각
    pub created_at: DateTime<Utc>,
}

/// 병합된 문서 - 여러 문서가 조합된 최종 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedDocument {
    /// 병합된 최종 내용
    pub content: String,

    /// 소스 문서 ID 목록
    pub source_documents: Vec<String>,

    /// 최종 토큰 수
    pub tokens: usize,

    /// 병합된 섹션들
    pub sections: Vec<MergedSection>,
}

/// 병합된 섹션 - 개별 문서에서 추출된 섹션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedSection {
    /// 섹션 제목
    pub title: String,

    /// 섹션 내용
    pub content: String,

    /// 소스 문서 ID
    pub source_doc_id: String,

    /// 섹션 신뢰도
    pub confidence: f32,
}

/// 실행 컨텍스트 - 결정성 계산에 사용되는 버전 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// 룰 버전
    pub rules_version: String,

    /// 레시피 버전
    pub recipe_version: String,

    /// 커밋 SHA
    pub commit_sha: String,

    /// 상수 버전
    pub constants_version: String,

    /// 실행 시각
    pub timestamp: DateTime<Utc>,
}

/// 정규화된 입력 - 결정성 계산을 위한 표준화된 입력
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalInput {
    /// 정규화된 텍스트
    pub normalized_text: String,

    /// 정렬된 패싯 맵
    pub facets: BTreeMap<String, Vec<String>>,

    /// 추가 메타데이터
    pub metadata: HashMap<String, String>,
}

/// 점수 세부사항 (기존 코드와의 호환성을 위해 유지)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub keyword_score: f32,
    pub freshness_score: f32,
    pub trust_score: f32,
    pub confidence_score: f32,
    pub facet_coverage_score: f32,
    pub axes_match_score: f32,
}

/// 점수가 매겨진 후보 문서
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredCandidate {
    /// 원본 문서 후보
    pub candidate: DocumentCandidate,

    /// 기본 점수 (가중 조합)
    pub base_score: f32,

    /// 개별 점수들
    pub keyword_score: f32,
    pub freshness_score: f32,
    pub trust_score: f32,
    pub confidence_score: f32,
    pub facet_coverage_score: f32,
    pub axes_match_score: f32,

    /// MMR에서 사용되는 최종 점수
    pub mmr_score: Option<f32>,

    /// 점수 세부사항 (기존 코드와의 호환성을 위해 유지)
    pub score_breakdown: ScoreBreakdown,
}

/// 점수 계산 가중치
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub keyword: f32,
    pub freshness: f32,
    pub trust: f32,
    pub confidence: f32,
    pub facet_coverage: f32,
    pub axes_match: f32,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            keyword: crate::common::constants::scoring::DEFAULT_KEYWORD_WEIGHT,
            freshness: crate::common::constants::scoring::DEFAULT_FRESHNESS_WEIGHT,
            trust: crate::common::constants::scoring::DEFAULT_TRUST_WEIGHT,
            confidence: crate::common::constants::scoring::DEFAULT_CONFIDENCE_WEIGHT,
            facet_coverage: crate::common::constants::scoring::DEFAULT_FACET_COVERAGE_WEIGHT,
            axes_match: crate::common::constants::scoring::DEFAULT_AXES_MATCH_WEIGHT,
        }
    }
}

impl MergedDocument {
    /// 빈 병합 문서 생성
    pub fn empty() -> Self {
        Self {
            content:
                "# 조건에 맞는 문서를 찾을 수 없습니다\n\n지정된 조건에 부합하는 문서가 없습니다."
                    .to_string(),
            source_documents: vec![],
            tokens: 20, // 대략적인 토큰 수
            sections: vec![],
        }
    }

    /// 문서가 비어있는지 확인
    pub fn is_empty(&self) -> bool {
        self.source_documents.is_empty()
    }
}

impl CompositionResult {
    /// 새로운 조합 결과 생성
    pub fn new(
        merged_document: MergedDocument,
        execution_id: String,
        confidence_score: f32,
    ) -> Self {
        Self {
            total_tokens_used: merged_document.tokens,
            merged_document,
            selection_rationale: "기본 선택 전략 적용".to_string(),
            confidence_score,
            rejected_documents: Vec::new(),
            execution_id,
            similarity_used: None,
            created_at: Utc::now(),
        }
    }

    /// 유사성을 사용한 결과인지 확인
    pub fn is_similarity_based(&self) -> bool {
        self.similarity_used.is_some()
    }
}

impl ExecutionContext {
    /// 현재 버전으로 새 컨텍스트 생성
    pub fn current(commit_sha: String) -> Self {
        Self {
            rules_version: crate::common::constants::RULES_VERSION.to_string(),
            recipe_version: crate::common::constants::RECIPE_VERSION.to_string(),
            commit_sha,
            constants_version: crate::common::constants::CONSTANTS_VERSION.to_string(),
            timestamp: Utc::now(),
        }
    }
}

impl CanonicalInput {
    /// 새로운 정규화된 입력 생성
    pub fn new(text: String, facets: HashMap<String, Vec<String>>) -> Self {
        // 패싯을 정렬된 BTreeMap으로 변환
        let sorted_facets: BTreeMap<String, Vec<String>> = facets
            .into_iter()
            .map(|(k, mut v)| {
                v.sort();
                (k, v)
            })
            .collect();

        Self {
            normalized_text: text,
            facets: sorted_facets,
            metadata: HashMap::new(),
        }
    }
}

impl ScoredCandidate {
    /// 새로운 점수 매겨진 후보 생성
    pub fn new(candidate: DocumentCandidate) -> Self {
        Self {
            candidate,
            base_score: 0.0,
            keyword_score: 0.0,
            freshness_score: 0.0,
            trust_score: 0.0,
            confidence_score: 0.0,
            facet_coverage_score: 0.0,
            axes_match_score: 0.0,
            mmr_score: None,
            score_breakdown: ScoreBreakdown {
                keyword_score: 0.0,
                freshness_score: 0.0,
                trust_score: 0.0,
                confidence_score: 0.0,
                facet_coverage_score: 0.0,
                axes_match_score: 0.0,
            },
        }
    }

    /// 가중 점수 계산
    pub fn calculate_base_score(&mut self, weights: &ScoringWeights) {
        self.base_score = weights.keyword * self.keyword_score
            + weights.freshness * self.freshness_score
            + weights.trust * self.trust_score
            + weights.confidence * self.confidence_score
            + weights.facet_coverage * self.facet_coverage_score
            + weights.axes_match * self.axes_match_score;

        // 호환성을 위해 score_breakdown도 업데이트
        self.score_breakdown.keyword_score = self.keyword_score;
        self.score_breakdown.freshness_score = self.freshness_score;
        self.score_breakdown.trust_score = self.trust_score;
        self.score_breakdown.confidence_score = self.confidence_score;
        self.score_breakdown.facet_coverage_score = self.facet_coverage_score;
        self.score_breakdown.axes_match_score = self.axes_match_score;
    }
}

impl ContextDocument {
    /// 기본 ContextDocument 생성
    pub fn new(id: String, title: String) -> Self {
        Self {
            id,
            title,
            version: "1.0.0".to_string(),
            schema: crate::common::constants::defaults::DEFAULT_SCHEMA_VERSION.to_string(),
            r#type: "guide".to_string(),
            domain: None,
            tags: Vec::new(),
            facets: HashMap::new(),
            locale: None,
            trust: None,
            freshness: None,
            aliases: None,
            conflicts: None,
            dependencies: Vec::new(),
            author: None,
            updated: Some(chrono::Utc::now()),
            estimated_tokens: None,
            sections: Vec::new(),
            path: None,
        }
    }

    /// 테스트용 기본 ContextDocument 생성 (모든 필드 포함)
    pub fn new_test(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            version: "1.0.0".to_string(),
            schema: crate::common::constants::defaults::DEFAULT_SCHEMA_VERSION.to_string(),
            r#type: "guide".to_string(),
            domain: None,
            tags: Vec::new(),
            facets: HashMap::new(),
            locale: None,
            trust: None,
            freshness: None,
            aliases: None,
            conflicts: None,
            dependencies: Vec::new(),
            author: None,
            updated: Some(chrono::Utc::now()),
            estimated_tokens: None,
            sections: Vec::new(),
            path: None,
        }
    }

    /// 스키마 검증
    pub fn validate_schema(&self) -> crate::Result<()> {
        if self.id.trim().is_empty() {
            return Err(crate::ContextError::InvalidFrontmatter(
                "ID cannot be empty".to_string(),
            ));
        }

        if self.title.trim().is_empty() {
            return Err(crate::ContextError::InvalidFrontmatter(
                "Title cannot be empty".to_string(),
            ));
        }

        if self.sections.is_empty() {
            return Err(crate::ContextError::InvalidFrontmatter(
                "At least one section must be defined".to_string(),
            ));
        }

        // 섹션 ID 중복 검사
        let mut seen_ids = std::collections::HashSet::new();
        for section in &self.sections {
            if !seen_ids.insert(&section.id) {
                return Err(crate::ContextError::InvalidFrontmatter(format!(
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
    use chrono::Utc;

    #[test]
    fn test_composition_result_serialization() -> crate::Result<()> {
        let merged_doc = MergedDocument {
            content: "Test content".to_string(),
            source_documents: vec!["doc1".to_string(), "doc2".to_string()],
            tokens: 100,
            sections: vec![MergedSection {
                title: "Test Section".to_string(),
                content: "Section content".to_string(),
                source_doc_id: "doc1".to_string(),
                confidence: 0.9,
            }],
        };

        let composition_result = CompositionResult {
            merged_document: merged_doc,
            selection_rationale: "Selected based on relevance".to_string(),
            total_tokens_used: 100,
            confidence_score: 0.85,
            rejected_documents: vec![],
            execution_id: "test-exec-id".to_string(),
            similarity_used: Some(0.9),
            created_at: Utc::now(),
        };

        // Test serialization
        let json = serde_json::to_string(&composition_result)?;
        assert!(!json.is_empty());

        // Test deserialization
        let deserialized: CompositionResult = serde_json::from_str(&json)?;
        assert_eq!(deserialized.execution_id, "test-exec-id");
        assert_eq!(deserialized.total_tokens_used, 100);
        assert_eq!(deserialized.confidence_score, 0.85);
        Ok(())
    }

    #[test]
    fn test_execution_context_creation() {
        let context = ExecutionContext {
            rules_version: "1.0.0".to_string(),
            recipe_version: "1.0.0".to_string(),
            commit_sha: "abc123".to_string(),
            constants_version: "1.0.0".to_string(),
            timestamp: Utc::now(),
        };

        assert_eq!(context.rules_version, "1.0.0");
        assert_eq!(context.commit_sha, "abc123");
    }

    #[test]
    fn test_canonical_input_normalization() {
        let mut facets = BTreeMap::new();
        facets.insert(
            "language".to_string(),
            vec!["rust".to_string(), "python".to_string()],
        );
        facets.insert("framework".to_string(), vec!["tokio".to_string()]);

        let canonical_input = CanonicalInput {
            normalized_text: "normalized test text".to_string(),
            facets,
            metadata: HashMap::new(),
        };

        // Test that facets are properly structured
        assert!(canonical_input.facets.contains_key("language"));
        assert!(canonical_input.facets.contains_key("framework"));
        assert_eq!(canonical_input.facets["language"].len(), 2);
        assert_eq!(canonical_input.facets["framework"].len(), 1);
    }

    #[test]
    fn test_merged_document_empty() {
        let empty_doc = MergedDocument {
            content: "# No Documents Found\n\nNo documents matched the specified criteria."
                .to_string(),
            source_documents: vec![],
            tokens: 10,
            sections: vec![],
        };

        assert!(empty_doc.source_documents.is_empty());
        assert!(empty_doc.sections.is_empty());
        assert_eq!(empty_doc.tokens, 10);
        assert!(empty_doc.content.contains("No Documents Found"));
    }

    #[test]
    fn test_merged_section_confidence() {
        let section = MergedSection {
            title: "Test Section".to_string(),
            content: "Test content".to_string(),
            source_doc_id: "doc-123".to_string(),
            confidence: 0.95,
        };

        assert_eq!(section.confidence, 0.95);
        assert_eq!(section.source_doc_id, "doc-123");
        assert!(!section.title.is_empty());
        assert!(!section.content.is_empty());
    }
}
