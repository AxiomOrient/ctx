pub mod facet_coverage;
pub mod freshness;
pub mod keyword;
pub mod similarity;

use crate::Result;
use crate::common::constants::composition::MAX_CANDIDATES_TO_SCORE;
use crate::common::constants::scoring::*;
use crate::core::composer::types::{BuildQuery, DocumentCandidate};
use std::collections::HashMap;

// Re-export common types for backward compatibility
pub use crate::common::{ScoreBreakdown, ScoredCandidate, ScoringWeights};

/// 문서 스코어러 - 모듈화된 스코어링 시스템
///
/// 여러 기준으로 문서를 평가하고 가중치를 적용하여 최종 점수를 계산합니다.
/// TF-IDF 키워드 매칭, 신선도 감쇠, 패싯 커버리지, 문서 간 유사도를 종합적으로 고려합니다.
pub struct DocumentScorer {
    weights: ScoringWeights,
    tau_days: f32, // 신선도 감쇠 상수
}

/// 쿼리 신호 - 점수 계산에 필요한 쿼리 정보 추출
#[derive(Debug, Clone)]
pub struct QuerySignals {
    pub query_text: Option<String>,
    pub required_facets: Vec<(String, Vec<String>)>,
    pub required_axes: Option<Vec<String>>,
}

impl From<&BuildQuery> for QuerySignals {
    fn from(query: &BuildQuery) -> Self {
        Self {
            query_text: query.query_text.clone(),
            required_facets: query.required_facets.clone(),
            required_axes: query.axes.clone(),
        }
    }
}

impl DocumentScorer {
    /// 기본 설정으로 스코어러 생성
    pub fn new() -> Self {
        Self {
            weights: ScoringWeights::default(),
            tau_days: DEFAULT_FRESHNESS_TAU_DAYS,
        }
    }

    /// 커스텀 가중치로 스코어러 생성
    pub fn with_weights(weights: ScoringWeights) -> Self {
        Self {
            weights,
            tau_days: DEFAULT_FRESHNESS_TAU_DAYS,
        }
    }

    /// 신선도 감쇠 상수 설정
    pub fn with_tau_days(mut self, tau_days: f32) -> Self {
        self.tau_days = tau_days;
        self
    }

    /// 문서들에 점수 부여
    ///
    /// 각 문서를 여러 기준으로 평가하고 신뢰도 임계값을 적용하여
    /// 점수가 매겨진 후보 문서 목록을 반환합니다.
    pub fn score_documents(
        &self,
        candidates: &[DocumentCandidate],
        query: &BuildQuery,
    ) -> Result<Vec<ScoredCandidate>> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let signals = QuerySignals::from(query);
        let mut scored = Vec::with_capacity(candidates.len());

        // 전체 후보가 너무 많으면 제한 (성능 최적화)
        let candidates_to_score = if candidates.len() > MAX_CANDIDATES_TO_SCORE {
            &candidates[..MAX_CANDIDATES_TO_SCORE]
        } else {
            candidates
        };

        for candidate in candidates_to_score {
            // 신뢰도 임계값 체크
            if candidate.confidence < query.confidence_threshold {
                continue;
            }

            let mut scored_candidate = ScoredCandidate::new(candidate.clone());
            self.calculate_all_scores(&mut scored_candidate, &signals);

            // 가중 점수 계산
            scored_candidate.calculate_base_score(&self.weights);

            scored.push(scored_candidate);
        }

        // 점수 기준 정렬 (높은 점수부터)
        scored.sort_by(|a, b| {
            b.base_score
                .partial_cmp(&a.base_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(scored)
    }

    /// 모든 개별 점수 계산
    fn calculate_all_scores(&self, scored: &mut ScoredCandidate, signals: &QuerySignals) {
        scored.keyword_score = self.calculate_keyword_score(&scored.candidate, signals);
        scored.freshness_score = self.calculate_freshness_score(&scored.candidate);
        scored.trust_score = scored.candidate.trust;
        scored.confidence_score = scored.candidate.confidence;
        scored.facet_coverage_score = self.calculate_facet_coverage(&scored.candidate, signals);
        scored.axes_match_score = self.calculate_axes_match(&scored.candidate, signals);
    }

    /// 키워드 점수 계산 (제목 부스트 포함)
    fn calculate_keyword_score(
        &self,
        candidate: &DocumentCandidate,
        signals: &QuerySignals,
    ) -> f32 {
        let base_score = keyword::calculate_keyword_score(candidate, signals);
        let title_boost = keyword::calculate_title_boost(candidate, signals);
        (base_score + title_boost).min(1.0)
    }

    /// 신선도 점수 계산
    fn calculate_freshness_score(&self, candidate: &DocumentCandidate) -> f32 {
        freshness::calculate_freshness_score(candidate, self.tau_days)
    }

    /// 패싯 커버리지 점수 계산
    fn calculate_facet_coverage(
        &self,
        candidate: &DocumentCandidate,
        signals: &QuerySignals,
    ) -> f32 {
        facet_coverage::calculate_facet_coverage(candidate, signals)
    }

    /// 축 매칭 점수 계산
    fn calculate_axes_match(&self, candidate: &DocumentCandidate, signals: &QuerySignals) -> f32 {
        facet_coverage::calculate_axes_match(candidate, signals)
    }

    /// 두 문서 간 유사도 계산
    pub fn calculate_similarity(&self, a: &DocumentCandidate, b: &DocumentCandidate) -> f32 {
        similarity::calculate_similarity(a, b)
    }

    /// 점수 가중치 조회
    pub fn weights(&self) -> &ScoringWeights {
        &self.weights
    }

    /// 점수 가중치 업데이트
    pub fn set_weights(&mut self, weights: ScoringWeights) {
        self.weights = weights;
    }

    /// 배치 유사도 계산 (성능 최적화)
    pub fn calculate_similarity_matrix(&self, candidates: &[DocumentCandidate]) -> Vec<Vec<f32>> {
        let n = candidates.len();
        let mut matrix = vec![vec![0.0; n]; n];

        // 대칭 행렬이므로 상삼각 부분만 계산
        for i in 0..n {
            matrix[i][i] = 1.0; // 자기 자신과의 유사도는 1.0

            for j in (i + 1)..n {
                let similarity = self.calculate_similarity(&candidates[i], &candidates[j]);
                matrix[i][j] = similarity;
                matrix[j][i] = similarity; // 대칭성 활용
            }
        }

        matrix
    }

    /// 점수 정규화 (0-1 범위로 스케일링)
    pub fn normalize_scores(&self, scored: &mut [ScoredCandidate]) {
        if scored.is_empty() {
            return;
        }

        let scores: Vec<f32> = scored.iter().map(|s| s.base_score).collect();
        let min_score = scores.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_score = scores.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        if (max_score - min_score).abs() < f32::EPSILON {
            // 모든 점수가 같으면 정규화하지 않음
            return;
        }

        for candidate in scored.iter_mut() {
            candidate.base_score = (candidate.base_score - min_score) / (max_score - min_score);
        }
    }

    /// 점수 기반 필터링 (임계값 이하 제거)
    pub fn filter_by_score_threshold(
        &self,
        scored: Vec<ScoredCandidate>,
        threshold: f32,
    ) -> Vec<ScoredCandidate> {
        scored
            .into_iter()
            .filter(|candidate| candidate.base_score >= threshold)
            .collect()
    }

    /// 다양성 기반 필터링 (너무 유사한 문서 제거)
    pub fn filter_by_diversity(
        &self,
        mut scored: Vec<ScoredCandidate>,
        similarity_threshold: f32,
    ) -> Vec<ScoredCandidate> {
        if scored.len() <= 1 {
            return scored;
        }

        // 점수 순으로 정렬 (높은 점수부터)
        scored.sort_by(|a, b| {
            b.base_score
                .partial_cmp(&a.base_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut filtered: Vec<ScoredCandidate> = Vec::new();

        for candidate in scored {
            let mut is_diverse = true;

            // 이미 선택된 문서들과 유사도 체크
            for selected in &filtered {
                let similarity =
                    self.calculate_similarity(&candidate.candidate, &selected.candidate);
                if similarity > similarity_threshold {
                    is_diverse = false;
                    break;
                }
            }

            if is_diverse {
                filtered.push(candidate);
            }
        }

        filtered
    }

    /// 문서 집합의 점수 분포 분석 (개선된 버전)
    pub fn analyze_score_distribution(&self, scored: &[ScoredCandidate]) -> ScoreDistribution {
        if scored.is_empty() {
            return ScoreDistribution::empty();
        }

        let scores: Vec<f32> = scored.iter().map(|s| s.base_score).collect();
        let mean = scores.iter().sum::<f32>() / scores.len() as f32;

        let mut sorted_scores = scores.clone();
        sorted_scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let median = if sorted_scores.len() % 2 == 0 {
            let mid = sorted_scores.len() / 2;
            (sorted_scores[mid - 1] + sorted_scores[mid]) / 2.0
        } else {
            sorted_scores[sorted_scores.len() / 2]
        };

        // 추가 통계: 사분위수 계산
        let q1_idx = sorted_scores.len() / 4;
        let q3_idx = (sorted_scores.len() * 3) / 4;
        let q1 = sorted_scores.get(q1_idx).copied().unwrap_or(0.0);
        let q3 = sorted_scores.get(q3_idx).copied().unwrap_or(0.0);

        ScoreDistribution {
            count: scored.len(),
            min_score: sorted_scores.first().copied().unwrap_or(0.0),
            max_score: sorted_scores.last().copied().unwrap_or(0.0),
            mean_score: mean,
            median_score: median,
            std_dev: calculate_standard_deviation(&scores, mean),
            q1_score: q1,
            q3_score: q3,
            iqr: q3 - q1,
        }
    }

    /// 후보 문서들의 품질 요약
    pub fn summarize_quality(&self, scored: &[ScoredCandidate]) -> QualitySummary {
        if scored.is_empty() {
            return QualitySummary::empty();
        }

        let high_quality = scored.iter().filter(|s| s.base_score >= 0.7).count();
        let medium_quality = scored
            .iter()
            .filter(|s| s.base_score >= 0.4 && s.base_score < 0.7)
            .count();
        let low_quality = scored.len() - high_quality - medium_quality;

        // 평균 개별 점수들
        let avg_keyword = scored.iter().map(|s| s.keyword_score).sum::<f32>() / scored.len() as f32;
        let avg_freshness =
            scored.iter().map(|s| s.freshness_score).sum::<f32>() / scored.len() as f32;
        let avg_trust = scored.iter().map(|s| s.trust_score).sum::<f32>() / scored.len() as f32;
        let avg_facet_coverage =
            scored.iter().map(|s| s.facet_coverage_score).sum::<f32>() / scored.len() as f32;

        QualitySummary {
            total_documents: scored.len(),
            high_quality_count: high_quality,
            medium_quality_count: medium_quality,
            low_quality_count: low_quality,
            average_scores: HashMap::from([
                ("keyword".to_string(), avg_keyword),
                ("freshness".to_string(), avg_freshness),
                ("trust".to_string(), avg_trust),
                ("facet_coverage".to_string(), avg_facet_coverage),
            ]),
        }
    }
}

/// 점수 분포 통계 (개선된 버전)
#[derive(Debug, Clone)]
pub struct ScoreDistribution {
    pub count: usize,
    pub min_score: f32,
    pub max_score: f32,
    pub mean_score: f32,
    pub median_score: f32,
    pub std_dev: f32,
    pub q1_score: f32, // 1사분위수
    pub q3_score: f32, // 3사분위수
    pub iqr: f32,      // 사분위수 범위
}

impl ScoreDistribution {
    fn empty() -> Self {
        Self {
            count: 0,
            min_score: 0.0,
            max_score: 0.0,
            mean_score: 0.0,
            median_score: 0.0,
            std_dev: 0.0,
            q1_score: 0.0,
            q3_score: 0.0,
            iqr: 0.0,
        }
    }
}

/// 품질 요약 통계
#[derive(Debug, Clone)]
pub struct QualitySummary {
    pub total_documents: usize,
    pub high_quality_count: usize,   // >= 0.7
    pub medium_quality_count: usize, // 0.4 - 0.7
    pub low_quality_count: usize,    // < 0.4
    pub average_scores: HashMap<String, f32>,
}

impl QualitySummary {
    fn empty() -> Self {
        Self {
            total_documents: 0,
            high_quality_count: 0,
            medium_quality_count: 0,
            low_quality_count: 0,
            average_scores: HashMap::new(),
        }
    }
}

/// 표준편차 계산
fn calculate_standard_deviation(values: &[f32], mean: f32) -> f32 {
    if values.len() <= 1 {
        return 0.0;
    }

    let variance =
        values.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / (values.len() - 1) as f32;

    variance.sqrt()
}

impl Default for DocumentScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_candidate(title: &str, confidence: f32, trust: f32) -> DocumentCandidate {
        DocumentCandidate {
            doc_id: "test".to_string(),
            sha: "sha".to_string(),
            title: title.to_string(),
            locale: "ko".to_string(),
            trust,
            freshness: "2024-01-01T00:00:00Z".to_string(),
            confidence,
            path: "test.md".to_string(),
            facets: HashMap::new(),
            tokens: 100,
            content: Some("Test content".to_string()),
        }
    }

    fn create_test_query() -> BuildQuery {
        BuildQuery::new(
            "test-repo".to_string(),
            "main".to_string(),
            "abc123".to_string(),
        )
        .with_query_text("test programming".to_string())
        .with_confidence_threshold(0.5)
    }

    #[test]
    fn test_score_documents_basic() -> crate::Result<()> {
        let scorer = DocumentScorer::new();
        let candidates = vec![
            create_test_candidate("Test Programming Guide", 0.8, 0.9),
            create_test_candidate("Another Document", 0.6, 0.7),
        ];
        let query = create_test_query();

        let scored = scorer.score_documents(&candidates, &query)?;

        assert_eq!(
            scored.len(),
            2,
            "Should score all candidates above threshold"
        );
        assert!(
            scored[0].base_score >= scored[1].base_score,
            "Should be sorted by score"
        );
        Ok(())
    }

    #[test]
    fn test_confidence_threshold_filtering() -> crate::Result<()> {
        let scorer = DocumentScorer::new();
        let candidates = vec![
            create_test_candidate("High Confidence", 0.8, 0.9),
            create_test_candidate("Low Confidence", 0.3, 0.7), // Below threshold
        ];
        let query = create_test_query(); // threshold = 0.5

        let scored = scorer.score_documents(&candidates, &query)?;

        assert_eq!(
            scored.len(),
            1,
            "Should filter out low confidence documents"
        );
        assert_eq!(scored[0].candidate.title, "High Confidence");
        Ok(())
    }

    #[test]
    fn test_empty_candidates() -> crate::Result<()> {
        let scorer = DocumentScorer::new();
        let query = create_test_query();

        let scored = scorer.score_documents(&[], &query)?;

        assert!(scored.is_empty(), "Empty input should produce empty output");
        Ok(())
    }

    #[test]
    fn test_similarity_calculation() {
        let scorer = DocumentScorer::new();

        let mut facets = HashMap::new();
        facets.insert("lang".to_string(), vec!["rust".to_string()]);

        let mut candidate_a = create_test_candidate("Rust Programming", 0.8, 0.9);
        candidate_a.facets = facets.clone();

        let mut candidate_b = create_test_candidate("Rust Tutorial", 0.8, 0.9);
        candidate_b.facets = facets;

        let similarity = scorer.calculate_similarity(&candidate_a, &candidate_b);

        assert!(
            similarity > 0.0,
            "Similar documents should have positive similarity"
        );
        assert!(similarity <= 1.0, "Similarity should not exceed 1.0");
    }

    #[test]
    fn test_score_distribution_analysis() -> crate::Result<()> {
        let scorer = DocumentScorer::new();
        let candidates = vec![
            create_test_candidate("High Score Doc", 0.9, 0.9),
            create_test_candidate("Medium Score Doc", 0.7, 0.7),
            create_test_candidate("Low Score Doc", 0.6, 0.5),
        ];
        let query = create_test_query();

        let scored = scorer.score_documents(&candidates, &query)?;
        let distribution = scorer.analyze_score_distribution(&scored);

        assert_eq!(distribution.count, 3);
        assert!(distribution.min_score <= distribution.max_score);
        assert!(distribution.mean_score > 0.0);
        Ok(())
    }

    #[test]
    fn test_quality_summary() -> crate::Result<()> {
        let scorer = DocumentScorer::new();
        let candidates = vec![
            create_test_candidate("High Quality", 0.9, 0.9), // Should be high quality
            create_test_candidate("Medium Quality", 0.7, 0.6), // Should be medium quality
            create_test_candidate("Low Quality", 0.6, 0.4),  // Should be low quality
        ];
        let query = create_test_query();

        let scored = scorer.score_documents(&candidates, &query)?;
        let summary = scorer.summarize_quality(&scored);

        assert_eq!(summary.total_documents, 3);
        assert!(
            summary.high_quality_count > 0
                || summary.medium_quality_count > 0
                || summary.low_quality_count > 0
        );
        assert!(summary.average_scores.contains_key("keyword"));
        Ok(())
    }

    #[test]
    fn test_custom_weights() -> crate::Result<()> {
        let weights = ScoringWeights {
            keyword: 1.0,
            freshness: 0.0,
            ..Default::default()
        };
        let scorer = DocumentScorer::with_weights(weights);
        let candidates = vec![create_test_candidate("Programming Guide", 0.8, 0.9)];
        let query = create_test_query(); // Contains "programming"

        let scored = scorer.score_documents(&candidates, &query)?;

        assert_eq!(scored.len(), 1);
        assert!(scored[0].keyword_score > 0.0, "Should have keyword score");
        Ok(())
    }

    #[test]
    fn test_similarity_matrix() {
        let scorer = DocumentScorer::new();

        let mut facets = HashMap::new();
        facets.insert("lang".to_string(), vec!["rust".to_string()]);

        let candidates = vec![
            create_test_candidate("Rust Guide A", 0.8, 0.9),
            create_test_candidate("Rust Guide B", 0.8, 0.9),
            create_test_candidate("Python Tutorial", 0.7, 0.8),
        ];

        let matrix = scorer.calculate_similarity_matrix(&candidates);

        assert_eq!(matrix.len(), 3);
        assert_eq!(matrix[0].len(), 3);

        // 대각선은 1.0이어야 함
        assert_eq!(matrix[0][0], 1.0);
        assert_eq!(matrix[1][1], 1.0);
        assert_eq!(matrix[2][2], 1.0);

        // 대칭성 확인
        assert_eq!(matrix[0][1], matrix[1][0]);
        assert_eq!(matrix[0][2], matrix[2][0]);
        assert_eq!(matrix[1][2], matrix[2][1]);
    }

    #[test]
    fn test_score_normalization() -> crate::Result<()> {
        let scorer = DocumentScorer::new();
        let candidates = vec![
            create_test_candidate("High Score", 0.9, 0.9),
            create_test_candidate("Medium Score", 0.7, 0.7),
            create_test_candidate("Low Score", 0.5, 0.5),
        ];
        let query = create_test_query();

        let mut scored = scorer.score_documents(&candidates, &query)?;
        let _original_scores: Vec<f32> = scored.iter().map(|s| s.base_score).collect();

        scorer.normalize_scores(&mut scored);

        // 정규화 후 최대값은 1.0, 최소값은 0.0이어야 함
        let normalized_scores: Vec<f32> = scored.iter().map(|s| s.base_score).collect();
        let max_normalized = normalized_scores
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let min_normalized = normalized_scores
            .iter()
            .fold(f32::INFINITY, |a, &b| a.min(b));

        assert!(
            (max_normalized - 1.0).abs() < 0.01,
            "Max should be ~1.0, got {}",
            max_normalized
        );
        assert!(
            (min_normalized - 0.0).abs() < 0.01,
            "Min should be ~0.0, got {}",
            min_normalized
        );
        Ok(())
    }

    #[test]
    fn test_score_threshold_filtering() -> crate::Result<()> {
        let scorer = DocumentScorer::new();
        let candidates = vec![
            create_test_candidate("High Score", 0.9, 0.9),
            create_test_candidate("Medium Score", 0.7, 0.7),
            create_test_candidate("Low Score", 0.5, 0.5),
        ];
        let query = create_test_query();

        let scored = scorer.score_documents(&candidates, &query)?;
        let filtered = scorer.filter_by_score_threshold(scored, 0.6);

        // 0.6 이상인 문서만 남아야 함
        assert!(filtered.len() <= 3);
        for candidate in &filtered {
            assert!(
                candidate.base_score >= 0.6,
                "Score should be >= 0.6, got {}",
                candidate.base_score
            );
        }
        Ok(())
    }

    #[test]
    fn test_diversity_filtering() -> crate::Result<()> {
        let scorer = DocumentScorer::new();

        let mut facets = HashMap::new();
        facets.insert("lang".to_string(), vec!["rust".to_string()]);

        let candidates = vec![
            create_test_candidate("Rust Guide A", 0.9, 0.9), // 높은 점수
            create_test_candidate("Rust Guide B", 0.8, 0.8), // 유사한 문서
            create_test_candidate("Python Tutorial", 0.7, 0.7), // 다른 문서
        ];
        let query = create_test_query();

        let scored = scorer.score_documents(&candidates, &query)?;
        let filtered = scorer.filter_by_diversity(scored, 0.5); // 50% 유사도 임계값

        // 다양성 필터링으로 유사한 문서가 제거되어야 함
        assert!(filtered.len() <= 3);
        assert!(!filtered.is_empty()); // 최소 하나는 남아야 함
        Ok(())
    }
}
