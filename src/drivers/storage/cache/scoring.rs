//! 점수 계산 캐싱
//!
//! 문서 점수 계산 결과를 캐싱하여 성능을 최적화합니다.

use super::{CacheKeyGenerator, CacheManager};
use crate::domain::constants::scoring::{
    DEFAULT_FACET_COVERAGE_WEIGHT as FACET_COVERAGE_WEIGHT,
    DEFAULT_FRESHNESS_WEIGHT as FRESHNESS_WEIGHT, DEFAULT_KEYWORD_WEIGHT as KEYWORD_WEIGHT,
};
use crate::domain::errors::ContextError;

// 유사성 가중치 (determinism constants에서 가져와야 하지만 임시로 정의)
const SIMILARITY_WEIGHT: f32 = 0.5;
use std::collections::HashMap;
use std::time::Instant;

/// 점수 계산 캐싱 매니저
pub struct ScoringCacheManager {
    cache_manager: &'static CacheManager,
}

impl ScoringCacheManager {
    /// 새로운 점수 캐싱 매니저 생성
    pub fn new() -> Self {
        Self {
            cache_manager: super::global_cache_manager(),
        }
    }

    /// 문서 점수 조회 (캐시 우선)
    pub fn get_score(
        &self,
        doc_content: &str,
        doc_metadata: &HashMap<String, String>,
        query_text: &str,
        query_facets: &[String],
        query_metadata: &HashMap<String, String>,
    ) -> Option<f32> {
        let doc_hash = CacheKeyGenerator::document_hash(doc_content, doc_metadata);
        let query_hash = CacheKeyGenerator::query_hash(query_text, query_facets, query_metadata);

        self.cache_manager.get_score(doc_hash, query_hash)
    }

    /// 문서 점수 저장
    pub fn put_score(
        &self,
        doc_content: &str,
        doc_metadata: &HashMap<String, String>,
        query_text: &str,
        query_facets: &[String],
        query_metadata: &HashMap<String, String>,
        score: f32,
    ) {
        let doc_hash = CacheKeyGenerator::document_hash(doc_content, doc_metadata);
        let query_hash = CacheKeyGenerator::query_hash(query_text, query_facets, query_metadata);

        self.cache_manager.put_score(doc_hash, query_hash, score);
    }

    /// 해시로 직접 점수 조회
    pub fn get_score_by_hash(&self, doc_hash: u64, query_hash: u64) -> Option<f32> {
        self.cache_manager.get_score(doc_hash, query_hash)
    }

    /// 해시로 직접 점수 저장
    pub fn put_score_by_hash(&self, doc_hash: u64, query_hash: u64, score: f32) {
        self.cache_manager.put_score(doc_hash, query_hash, score);
    }

    /// 배치 점수 계산 및 캐싱
    pub fn compute_and_cache_batch_scores<F>(
        &self,
        documents: &[(String, HashMap<String, String>)],
        query_text: &str,
        query_facets: &[String],
        query_metadata: &HashMap<String, String>,
        scoring_fn: F,
    ) -> Result<Vec<ScoreResult>, ContextError>
    where
        F: Fn(
            &str,
            &HashMap<String, String>,
            &str,
            &[String],
            &HashMap<String, String>,
        ) -> Result<f32, ContextError>,
    {
        let query_hash = CacheKeyGenerator::query_hash(query_text, query_facets, query_metadata);
        let mut results = Vec::with_capacity(documents.len());

        for (doc_content, doc_metadata) in documents {
            let doc_hash = CacheKeyGenerator::document_hash(doc_content, doc_metadata);

            let score_result =
                if let Some(cached_score) = self.cache_manager.get_score(doc_hash, query_hash) {
                    ScoreResult::cached(cached_score)
                } else {
                    let start_time = Instant::now();
                    let computed_score = scoring_fn(
                        doc_content,
                        doc_metadata,
                        query_text,
                        query_facets,
                        query_metadata,
                    )?;
                    let computation_time = start_time.elapsed().as_millis() as u64;

                    // 캐시에 저장
                    self.cache_manager
                        .put_score(doc_hash, query_hash, computed_score);

                    ScoreResult::computed(computed_score, computation_time)
                };

            results.push(score_result);
        }

        Ok(results)
    }

    /// 가중 점수 계산 및 캐싱
    pub fn compute_weighted_scores<F>(
        &self,
        documents: &[(String, HashMap<String, String>)],
        query_text: &str,
        query_facets: &[String],
        query_metadata: &HashMap<String, String>,
        scoring_components: &[ScoringComponent],
        component_fn: F,
    ) -> Result<Vec<WeightedScoreResult>, ContextError>
    where
        F: Fn(
            &str,
            &HashMap<String, String>,
            &str,
            &[String],
            &HashMap<String, String>,
            &ScoringComponent,
        ) -> Result<f32, ContextError>,
    {
        let mut results = Vec::with_capacity(documents.len());

        for (doc_content, doc_metadata) in documents {
            let mut component_scores = HashMap::new();
            let mut total_score = 0.0;
            let mut was_cached = true;
            let mut total_computation_time = 0u64;

            for component in scoring_components {
                let component_key = format!(
                    "{}_{}",
                    CacheKeyGenerator::document_hash(doc_content, doc_metadata),
                    component.cache_key()
                );
                let component_hash = CacheKeyGenerator::generate_key(&[&component_key]);
                let query_hash =
                    CacheKeyGenerator::query_hash(query_text, query_facets, query_metadata);

                let component_score = if let Some(cached) =
                    self.cache_manager.get_score(component_hash, query_hash)
                {
                    cached
                } else {
                    was_cached = false;
                    let start_time = Instant::now();
                    let computed = component_fn(
                        doc_content,
                        doc_metadata,
                        query_text,
                        query_facets,
                        query_metadata,
                        component,
                    )?;
                    let computation_time = start_time.elapsed().as_millis() as u64;
                    total_computation_time += computation_time;

                    // 컴포넌트별 점수 캐싱
                    self.cache_manager
                        .put_score(component_hash, query_hash, computed);
                    computed
                };

                component_scores.insert(component.name.clone(), component_score);
                total_score += component_score * component.weight;
            }

            let result = WeightedScoreResult {
                total_score,
                component_scores,
                was_cached,
                computation_time_ms: if was_cached {
                    None
                } else {
                    Some(total_computation_time)
                },
            };

            results.push(result);
        }

        Ok(results)
    }

    /// 최고 점수 문서들 찾기 (캐시 활용)
    pub fn find_top_scoring_documents<F>(
        &self,
        documents: &[(String, HashMap<String, String>)],
        query_text: &str,
        query_facets: &[String],
        query_metadata: &HashMap<String, String>,
        scoring_fn: F,
        top_k: usize,
    ) -> Result<Vec<(usize, f32)>, ContextError>
    where
        F: Fn(
            &str,
            &HashMap<String, String>,
            &str,
            &[String],
            &HashMap<String, String>,
        ) -> Result<f32, ContextError>,
    {
        let scores = self.compute_and_cache_batch_scores(
            documents,
            query_text,
            query_facets,
            query_metadata,
            scoring_fn,
        )?;

        let mut indexed_scores: Vec<(usize, f32)> = scores
            .into_iter()
            .enumerate()
            .map(|(idx, score_result)| (idx, score_result.score))
            .collect();

        // 점수 내림차순 정렬
        indexed_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 상위 k개만 반환
        indexed_scores.truncate(top_k);
        Ok(indexed_scores)
    }

    /// 점수 분포 분석
    pub fn analyze_score_distribution<F>(
        &self,
        documents: &[(String, HashMap<String, String>)],
        query_text: &str,
        query_facets: &[String],
        query_metadata: &HashMap<String, String>,
        scoring_fn: F,
    ) -> Result<ScoreDistribution, ContextError>
    where
        F: Fn(
            &str,
            &HashMap<String, String>,
            &str,
            &[String],
            &HashMap<String, String>,
        ) -> Result<f32, ContextError>,
    {
        let scores = self.compute_and_cache_batch_scores(
            documents,
            query_text,
            query_facets,
            query_metadata,
            scoring_fn,
        )?;

        let score_values: Vec<f32> = scores.iter().map(|s| s.score).collect();

        if score_values.is_empty() {
            return Ok(ScoreDistribution::default());
        }

        let min_score = score_values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_score = score_values
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let mean_score = score_values.iter().sum::<f32>() / score_values.len() as f32;

        // 표준편차 계산
        let variance = score_values
            .iter()
            .map(|score| (score - mean_score).powi(2))
            .sum::<f32>()
            / score_values.len() as f32;
        let std_deviation = variance.sqrt();

        // 분위수 계산을 위해 정렬
        let mut sorted_scores = score_values;
        sorted_scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let median = if sorted_scores.len() % 2 == 0 {
            let mid = sorted_scores.len() / 2;
            (sorted_scores[mid - 1] + sorted_scores[mid]) / 2.0
        } else {
            sorted_scores[sorted_scores.len() / 2]
        };

        let cache_hit_count = scores.iter().filter(|s| s.was_cached).count();
        let cache_hit_rate = cache_hit_count as f64 / scores.len() as f64;

        Ok(ScoreDistribution {
            min_score,
            max_score,
            mean_score,
            median_score: median,
            std_deviation,
            total_documents: scores.len(),
            cache_hit_rate,
        })
    }

    /// 캐시 통계 조회
    pub fn get_cache_stats(&self) -> super::CacheStats {
        self.cache_manager.get_all_stats().score
    }
}

impl Default for ScoringCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 점수 계산 결과
#[derive(Debug, Clone)]
pub struct ScoreResult {
    pub score: f32,
    pub was_cached: bool,
    pub computation_time_ms: Option<u64>,
}

impl ScoreResult {
    pub fn cached(score: f32) -> Self {
        Self {
            score,
            was_cached: true,
            computation_time_ms: None,
        }
    }

    pub fn computed(score: f32, computation_time_ms: u64) -> Self {
        Self {
            score,
            was_cached: false,
            computation_time_ms: Some(computation_time_ms),
        }
    }
}

/// 가중 점수 계산 결과
#[derive(Debug, Clone)]
pub struct WeightedScoreResult {
    pub total_score: f32,
    pub component_scores: HashMap<String, f32>,
    pub was_cached: bool,
    pub computation_time_ms: Option<u64>,
}

/// 점수 계산 컴포넌트
#[derive(Debug, Clone)]
pub struct ScoringComponent {
    pub name: String,
    pub weight: f32,
    pub component_type: ScoringComponentType,
}

impl ScoringComponent {
    pub fn new(name: String, weight: f32, component_type: ScoringComponentType) -> Self {
        Self {
            name,
            weight,
            component_type,
        }
    }

    pub fn cache_key(&self) -> String {
        format!("{}_{:?}", self.name, self.component_type)
    }

    /// 기본 점수 계산 컴포넌트들
    pub fn default_components() -> Vec<Self> {
        vec![
            Self::new(
                "keyword".to_string(),
                KEYWORD_WEIGHT,
                ScoringComponentType::Keyword,
            ),
            Self::new(
                "freshness".to_string(),
                FRESHNESS_WEIGHT,
                ScoringComponentType::Freshness,
            ),
            Self::new(
                "facet_coverage".to_string(),
                FACET_COVERAGE_WEIGHT,
                ScoringComponentType::FacetCoverage,
            ),
            Self::new(
                "similarity".to_string(),
                SIMILARITY_WEIGHT,
                ScoringComponentType::Similarity,
            ),
        ]
    }
}

/// 점수 계산 컴포넌트 타입
#[derive(Debug, Clone, PartialEq)]
pub enum ScoringComponentType {
    Keyword,
    Freshness,
    FacetCoverage,
    Similarity,
    Custom(String),
}

/// 점수 분포 통계
#[derive(Debug, Clone, Default)]
pub struct ScoreDistribution {
    pub min_score: f32,
    pub max_score: f32,
    pub mean_score: f32,
    pub median_score: f32,
    pub std_deviation: f32,
    pub total_documents: usize,
    pub cache_hit_rate: f64,
}

impl ScoreDistribution {
    /// 점수가 평균 + n * 표준편차보다 높은지 확인
    pub fn is_above_threshold(&self, score: f32, std_deviations: f32) -> bool {
        score > (self.mean_score + std_deviations * self.std_deviation)
    }

    /// 점수의 z-score 계산
    pub fn z_score(&self, score: f32) -> f32 {
        if self.std_deviation == 0.0 {
            0.0
        } else {
            (score - self.mean_score) / self.std_deviation
        }
    }

    /// 점수의 백분위수 계산 (근사치)
    pub fn percentile_rank(&self, score: f32) -> f32 {
        if score <= self.min_score {
            0.0
        } else if score >= self.max_score {
            100.0
        } else {
            // 선형 보간으로 근사 계산
            ((score - self.min_score) / (self.max_score - self.min_score)) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_scoring_fn(
        _doc_content: &str,
        _doc_metadata: &HashMap<String, String>,
        _query_text: &str,
        _query_facets: &[String],
        _query_metadata: &HashMap<String, String>,
    ) -> Result<f32, ContextError> {
        Ok(0.75) // 더미 점수
    }

    fn dummy_component_fn(
        _doc_content: &str,
        _doc_metadata: &HashMap<String, String>,
        _query_text: &str,
        _query_facets: &[String],
        _query_metadata: &HashMap<String, String>,
        component: &ScoringComponent,
    ) -> Result<f32, ContextError> {
        // 컴포넌트 타입에 따라 다른 점수
        match component.component_type {
            ScoringComponentType::Keyword => Ok(0.8),
            ScoringComponentType::Freshness => Ok(0.6),
            ScoringComponentType::FacetCoverage => Ok(0.9),
            ScoringComponentType::Similarity => Ok(0.7),
            ScoringComponentType::Custom(_) => Ok(0.5),
        }
    }

    #[test]
    fn test_score_cache_basic() {
        let cache_manager = ScoringCacheManager::new();

        let doc_metadata = HashMap::new();
        let query_facets = vec!["rust".to_string()];
        let query_metadata = HashMap::new();

        // 캐시에 없음
        assert_eq!(
            cache_manager.get_score(
                "doc",
                &doc_metadata,
                "query",
                &query_facets,
                &query_metadata
            ),
            None
        );

        // 캐시에 저장
        cache_manager.put_score(
            "doc",
            &doc_metadata,
            "query",
            &query_facets,
            &query_metadata,
            0.85,
        );

        // 캐시에서 조회
        assert_eq!(
            cache_manager.get_score(
                "doc",
                &doc_metadata,
                "query",
                &query_facets,
                &query_metadata
            ),
            Some(0.85)
        );
    }

    #[test]
    fn test_batch_score_computation() {
        let cache_manager = ScoringCacheManager::new();

        let documents = vec![
            ("document 1".to_string(), HashMap::new()),
            ("document 2".to_string(), HashMap::new()),
        ];

        let query_facets = vec!["test".to_string()];
        let query_metadata = HashMap::new();

        let results = cache_manager
            .compute_and_cache_batch_scores(
                &documents,
                "test query",
                &query_facets,
                &query_metadata,
                dummy_scoring_fn,
            )
            .expect("Failed to compute scores");

        assert_eq!(results.len(), 2);
        for result in &results {
            assert_eq!(result.score, 0.75);
            assert!(!result.was_cached); // 첫 번째 계산이므로 캐시되지 않음
        }

        // 다시 계산하면 캐시에서 가져옴
        let results2 = cache_manager
            .compute_and_cache_batch_scores(
                &documents,
                "test query",
                &query_facets,
                &query_metadata,
                dummy_scoring_fn,
            )
            .expect("Failed to compute scores");

        for result in &results2 {
            assert_eq!(result.score, 0.75);
            assert!(result.was_cached); // 이번에는 캐시에서 가져옴
        }
    }

    #[test]
    fn test_weighted_scores() {
        let cache_manager = ScoringCacheManager::new();

        let documents = vec![("test document".to_string(), HashMap::new())];

        let components = ScoringComponent::default_components();
        let query_facets = vec!["test".to_string()];
        let query_metadata = HashMap::new();

        let results = cache_manager
            .compute_weighted_scores(
                &documents,
                "test query",
                &query_facets,
                &query_metadata,
                &components,
                dummy_component_fn,
            )
            .expect("Failed to compute weighted scores");

        assert_eq!(results.len(), 1);
        assert!(!results[0].was_cached);
        assert_eq!(results[0].component_scores.len(), 4);

        // 가중 점수 계산 확인
        let expected_score = 0.8 * KEYWORD_WEIGHT
            + 0.6 * FRESHNESS_WEIGHT
            + 0.9 * FACET_COVERAGE_WEIGHT
            + 0.7 * SIMILARITY_WEIGHT;
        assert!((results[0].total_score - expected_score).abs() < 0.001);
    }

    #[test]
    fn test_score_distribution() {
        let cache_manager = ScoringCacheManager::new();

        // 다양한 점수를 반환하는 더미 함수
        fn varying_scores(
            doc_content: &str,
            _doc_metadata: &HashMap<String, String>,
            _query_text: &str,
            _query_facets: &[String],
            _query_metadata: &HashMap<String, String>,
        ) -> Result<f32, ContextError> {
            // 문서 내용에 따라 다른 점수
            match doc_content {
                "high" => Ok(0.9),
                "medium" => Ok(0.5),
                "low" => Ok(0.1),
                _ => Ok(0.0),
            }
        }

        let documents = vec![
            ("high".to_string(), HashMap::new()),
            ("medium".to_string(), HashMap::new()),
            ("low".to_string(), HashMap::new()),
        ];

        let query_facets = vec![];
        let query_metadata = HashMap::new();

        let distribution = cache_manager
            .analyze_score_distribution(
                &documents,
                "test query",
                &query_facets,
                &query_metadata,
                varying_scores,
            )
            .expect("Failed to analyze distribution");

        assert_eq!(distribution.min_score, 0.1);
        assert_eq!(distribution.max_score, 0.9);
        assert_eq!(distribution.total_documents, 3);
        assert!((distribution.mean_score - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_top_scoring_documents() {
        let cache_manager = ScoringCacheManager::new();

        fn indexed_scores(
            doc_content: &str,
            _doc_metadata: &HashMap<String, String>,
            _query_text: &str,
            _query_facets: &[String],
            _query_metadata: &HashMap<String, String>,
        ) -> Result<f32, ContextError> {
            // 문서 내용의 길이에 따른 점수
            Ok(doc_content.len() as f32 / 10.0)
        }

        let documents = vec![
            ("short".to_string(), HashMap::new()),         // 5/10 = 0.5
            ("medium length".to_string(), HashMap::new()), // 13/10 = 1.3
            ("a".to_string(), HashMap::new()),             // 1/10 = 0.1
            ("very long document".to_string(), HashMap::new()), // 18/10 = 1.8
        ];

        let query_facets = vec![];
        let query_metadata = HashMap::new();

        let top_docs = cache_manager
            .find_top_scoring_documents(
                &documents,
                "test query",
                &query_facets,
                &query_metadata,
                indexed_scores,
                2, // 상위 2개
            )
            .expect("Failed to find top documents");

        assert_eq!(top_docs.len(), 2);
        assert_eq!(top_docs[0].0, 3); // "very long document" (index 3)
        assert_eq!(top_docs[1].0, 1); // "medium length" (index 1)
        assert!(top_docs[0].1 > top_docs[1].1); // 점수 내림차순
    }
}
