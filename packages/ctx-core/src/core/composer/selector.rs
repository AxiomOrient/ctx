use super::scorer::{DocumentScorer, ScoredCandidate, ScoringWeights};
use super::types::{BuildQuery, DocumentCandidate};
use crate::Result;
use crate::common::constants::composition::*;
use std::sync::Arc;

// ---- Type aliases to reduce signature complexity (clippy::type-complexity)
type Candidate = DocumentCandidate;
type Selected = (DocumentCandidate, String);
type Selection = (Vec<Candidate>, Vec<Selected>);

/// 문서 선택기 - MMR 알고리즘과 토큰 예산 제약을 통한 최적 문서 선택
///
/// Requirements 1.3을 충족하는 DocumentSelector 구현:
/// - MMR 알고리즘으로 관련성과 다양성의 균형
/// - 토큰 예산 제약 처리
/// - DocumentScorer에 대한 의존성으로 유사도 계산
pub struct DocumentSelector {
    mmr_lambda: f32,
    scorer: Arc<DocumentScorer>,
}

/// MMR 선택기 (하위 호환성을 위한 별칭)
pub type MMRSelector = DocumentSelector;

/// 선택 결과
#[derive(Debug, Clone)]
pub struct SelectionResult {
    pub chosen: Vec<DocumentCandidate>,
    pub rejected: Vec<(DocumentCandidate, String)>,
    pub rationale: String,
    pub total_tokens: usize,
    pub average_confidence: f32,
}

impl DocumentSelector {
    /// 기본 설정으로 DocumentSelector 생성
    pub fn new() -> Self {
        Self {
            mmr_lambda: DEFAULT_MMR_LAMBDA,
            scorer: Arc::new(DocumentScorer::new()),
        }
    }

    /// 커스텀 가중치로 DocumentSelector 생성
    pub fn with_weights(weights: ScoringWeights) -> Self {
        Self {
            mmr_lambda: DEFAULT_MMR_LAMBDA,
            scorer: Arc::new(DocumentScorer::with_weights(weights)),
        }
    }

    /// 커스텀 MMR lambda 파라미터로 DocumentSelector 생성
    pub fn with_mmr_lambda(mmr_lambda: f32) -> Self {
        Self {
            mmr_lambda,
            scorer: Arc::new(DocumentScorer::new()),
        }
    }

    /// DocumentScorer 인스턴스를 공유하여 DocumentSelector 생성
    pub fn with_scorer(scorer: Arc<DocumentScorer>) -> Self {
        Self {
            mmr_lambda: DEFAULT_MMR_LAMBDA,
            scorer,
        }
    }

    /// 완전한 커스터마이징으로 DocumentSelector 생성
    pub fn with_config(mmr_lambda: f32, scorer: Arc<DocumentScorer>) -> Self {
        Self { mmr_lambda, scorer }
    }

    /// design.md 인터페이스에 맞는 문서 선택 메서드
    /// Requirements 1.3: MMR 알고리즘으로 토큰 예산 내에서 최적 문서 선택
    pub fn select_documents(
        &self,
        scored: Vec<ScoredCandidate>,
        budget: usize,
        reserve: usize,
    ) -> Result<Vec<ScoredCandidate>> {
        let effective_budget = budget.saturating_sub(reserve);

        // Phase 1: Greedy selection by score (fast path for small sets)
        if scored.len() <= GREEDY_SELECTION_THRESHOLD {
            return self.greedy_selection(scored, effective_budget);
        }

        // Phase 2: MMR selection for larger sets
        self.mmr_selection(scored, effective_budget)
    }

    /// MMR + 배낭 알고리즘으로 문서 선택 (기존 인터페이스 유지)
    pub fn select_with_mmr(
        &self,
        scored_candidates: Vec<ScoredCandidate>,
        budget: usize,
        _query: &BuildQuery,
    ) -> Result<SelectionResult> {
        if scored_candidates.is_empty() {
            return Ok(SelectionResult {
                chosen: Vec::new(),
                rejected: Vec::new(),
                rationale: "No candidates available".to_string(),
                total_tokens: 0,
                average_confidence: 0.0,
            });
        }

        // 1. MMR로 상위 후보들 재정렬 (성능을 위해 최대 200개로 제한)
        let mmr_candidates = self.apply_mmr_reranking(scored_candidates, 200)?;

        // 2. 0/1 배낭 알고리즘으로 최적 조합 선택
        let (chosen, rejected) = self.knapsack_selection(mmr_candidates, budget)?;

        // 3. 선택 근거 생성
        let rationale = self.generate_rationale(&chosen, &rejected, budget);

        // 4. 통계 계산
        let total_tokens = chosen.iter().map(|c| c.tokens).sum();
        let average_confidence = if chosen.is_empty() {
            0.0
        } else {
            chosen.iter().map(|c| c.confidence).sum::<f32>() / chosen.len() as f32
        };

        Ok(SelectionResult {
            chosen: chosen.to_vec(),
            rejected: rejected.to_vec(),
            rationale,
            total_tokens,
            average_confidence,
        })
    }

    /// MMR 재정렬 - PLAN.md 공식: MMR(d) = λ * score(d) - (1-λ) * max_{s∈S} sim(d,s)
    fn apply_mmr_reranking(
        &self,
        candidates: Vec<ScoredCandidate>,
        max_candidates: usize,
    ) -> Result<Vec<ScoredCandidate>> {
        let lambda = crate::common::constants::scoring::DEFAULT_MMR_LAMBDA;
        let mut selected: Vec<ScoredCandidate> = Vec::new();
        let mut remaining = candidates;

        while !remaining.is_empty() && selected.len() < max_candidates {
            // 각 후보의 MMR 점수 계산 (사이드이팩트 제거 - 원본 base_score 보존)
            let mut mmr_scores: Vec<f32> = Vec::with_capacity(remaining.len());
            
            for candidate in &remaining {
                let redundancy = if selected.is_empty() {
                    0.0
                } else {
                    selected
                        .iter()
                        .map(|s| {
                            self.scorer
                                .calculate_similarity(&candidate.candidate, &s.candidate)
                        })
                        .fold(0.0, f32::max)
                };

                // MMR 점수 계산 (원본 점수를 수정하지 않음)
                let mmr_score = lambda * candidate.base_score - (1.0 - lambda) * redundancy;
                mmr_scores.push(mmr_score);
            }

            // 최고 MMR 점수 후보 선택
            let best_idx = mmr_scores
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| {
                    a.partial_cmp(b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i);

            let best_idx = match best_idx {
                Some(idx) => idx,
                None => break, // No more candidates
            };

            let chosen = remaining.swap_remove(best_idx);
            selected.push(chosen);
        }

        Ok(selected)
    }

    /// MMR 선택 알고리즘 (design.md 인터페이스용)
    fn mmr_selection(
        &self,
        mut candidates: Vec<ScoredCandidate>,
        budget: usize,
    ) -> Result<Vec<ScoredCandidate>> {
        let mut selected = Vec::new();
        let mut remaining_budget = budget;

        // Sort by relevance score initially
        candidates.sort_by(|a, b| {
            b.base_score
                .partial_cmp(&a.base_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        while !candidates.is_empty() && remaining_budget > 0 {
            let mut best_idx = 0;
            let mut best_mmr_score = f32::NEG_INFINITY;

            for (idx, candidate) in candidates.iter().enumerate() {
                if candidate.candidate.tokens > remaining_budget {
                    continue; // Skip if doesn't fit
                }

                // Calculate diversity score (average similarity to selected documents)
                let diversity_score = if selected.is_empty() {
                    1.0 // First document has maximum diversity
                } else {
                    let avg_similarity = selected
                        .iter()
                        .map(|sel: &ScoredCandidate| {
                            self.scorer
                                .calculate_similarity(&candidate.candidate, &sel.candidate)
                        })
                        .sum::<f32>()
                        / selected.len() as f32;
                    1.0 - avg_similarity // Higher diversity = lower similarity
                };

                // MMR score: (1-λ) * relevance + λ * diversity
                let mmr_score = (1.0 - self.mmr_lambda) * candidate.base_score
                    + self.mmr_lambda * diversity_score;

                if mmr_score > best_mmr_score {
                    best_mmr_score = mmr_score;
                    best_idx = idx;
                }
            }

            if best_mmr_score == f32::NEG_INFINITY {
                break; // No more documents fit in budget
            }

            let selected_doc = candidates.remove(best_idx);
            remaining_budget = remaining_budget.saturating_sub(selected_doc.candidate.tokens);
            selected.push(selected_doc);
        }

        Ok(selected)
    }

    /// 0/1 배낭 알고리즘으로 최적 조합 선택
    fn knapsack_selection(
        &self,
        candidates: Vec<ScoredCandidate>,
        budget: usize,
    ) -> Result<Selection> {
        let n = candidates.len();
        if n == 0 || budget == 0 {
            let rejected = candidates
                .into_iter()
                .map(|c| (c.candidate, "No budget available".to_string()))
                .collect();
            return Ok((Vec::new(), rejected));
        }

        // 성능을 위해 예산이 너무 크면 그리디 알고리즘 사용
        if budget > crate::common::constants::scoring::KNAPSACK_BUDGET_THRESHOLD {
            return self.greedy_selection_legacy(candidates, budget);
        }

        // DP 테이블: dp[i][w] = 첫 i개 아이템으로 무게 w 이하에서 얻을 수 있는 최대 가치
        let mut dp = vec![vec![0.0f32; budget + 1]; n + 1];
        let mut take = vec![vec![false; budget + 1]; n + 1];

        // DP 테이블 채우기
        for i in 1..=n {
            let candidate = &candidates[i - 1];
            let weight = candidate.candidate.tokens.min(budget);
            let value = candidate.base_score;

            for w in 0..=budget {
                if weight <= w {
                    let include_value = dp[i - 1][w - weight] + value;
                    let exclude_value = dp[i - 1][w];

                    if include_value > exclude_value {
                        dp[i][w] = include_value;
                        take[i][w] = true;
                    } else {
                        dp[i][w] = exclude_value;
                        take[i][w] = false;
                    }
                } else {
                    dp[i][w] = dp[i - 1][w];
                    take[i][w] = false;
                }
            }
        }

        // 역추적으로 선택된 아이템 찾기
        let mut chosen = Vec::new();
        let mut w = budget;

        for i in (1..=n).rev() {
            if take[i][w] {
                chosen.push(candidates[i - 1].candidate.clone());
                w = w.saturating_sub(candidates[i - 1].candidate.tokens);
            }
        }

        chosen.reverse();

        // 선택되지 않은 문서들
        let chosen_ids: std::collections::HashSet<_> = chosen.iter().map(|c| &c.doc_id).collect();
        let rejected = candidates
            .into_iter()
            .filter(|c| !chosen_ids.contains(&c.candidate.doc_id))
            .map(|c| {
                (
                    c.candidate,
                    format!(
                        "Not selected (score={:.2}) or budget constraint",
                        c.base_score
                    ),
                )
            })
            .collect();

        Ok((chosen, rejected))
    }

    /// 그리디 선택 (design.md 인터페이스용)
    fn greedy_selection(
        &self,
        mut candidates: Vec<ScoredCandidate>,
        budget: usize,
    ) -> Result<Vec<ScoredCandidate>> {
        // Simple greedy selection for small sets
        candidates.sort_by(|a, b| {
            b.base_score
                .partial_cmp(&a.base_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut selected = Vec::new();
        let mut remaining_budget = budget;

        for candidate in candidates {
            if candidate.candidate.tokens <= remaining_budget {
                remaining_budget = remaining_budget.saturating_sub(candidate.candidate.tokens);
                selected.push(candidate);
            }
        }

        Ok(selected)
    }

    /// 그리디 선택 (기존 인터페이스용 - 큰 예산용 대안)
    fn greedy_selection_legacy(
        &self,
        mut candidates: Vec<ScoredCandidate>,
        budget: usize,
    ) -> Result<Selection> {
        // 효율성(점수/토큰) 기준으로 정렬
        candidates.sort_by(|a, b| {
            let efficiency_a = a.base_score / a.candidate.tokens.max(1) as f32;
            let efficiency_b = b.base_score / b.candidate.tokens.max(1) as f32;
            efficiency_b
                .partial_cmp(&efficiency_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut chosen = Vec::new();
        let mut used_tokens = 0;

        for candidate in &candidates {
            if used_tokens + candidate.candidate.tokens <= budget {
                used_tokens += candidate.candidate.tokens;
                chosen.push(candidate.candidate.clone());
            }
        }

        let chosen_ids: std::collections::HashSet<_> = chosen.iter().map(|c| &c.doc_id).collect();
        let rejected = candidates
            .into_iter()
            .filter(|c| !chosen_ids.contains(&c.candidate.doc_id))
            .map(|c| (c.candidate, "Budget constraint (greedy)".to_string()))
            .collect();

        Ok((chosen, rejected))
    }

    /// 선택 근거 생성
    fn generate_rationale(
        &self,
        chosen: &[DocumentCandidate],
        rejected: &[(DocumentCandidate, String)],
        budget: usize,
    ) -> String {
        let mut rationale = String::new();

        rationale.push_str(&format!(
            "## Selection Rationale (Budget: {} tokens)\n\n",
            budget
        ));

        if !chosen.is_empty() {
            rationale.push_str("### Selected Documents:\n");
            for (i, doc) in chosen.iter().enumerate() {
                rationale.push_str(&format!(
                    "{}. **{}** ({})\n   - Tokens: {}, Confidence: {:.2}, Trust: {:.2}\n   - Path: {}\n\n",
                    i + 1,
                    doc.title,
                    doc.doc_id,
                    doc.tokens,
                    doc.confidence,
                    doc.trust,
                    doc.path
                ));
            }

            let total_tokens: usize = chosen.iter().map(|c| c.tokens).sum();
            let avg_confidence: f32 =
                chosen.iter().map(|c| c.confidence).sum::<f32>() / chosen.len() as f32;

            rationale.push_str(&format!(
                "**Summary**: {} documents selected, {} tokens used ({:.1}% of budget), average confidence: {:.2}\n\n",
                chosen.len(),
                total_tokens,
                (total_tokens as f32 / budget as f32) * 100.0,
                avg_confidence
            ));
        }

        if !rejected.is_empty() {
            rationale.push_str("### Rejected Documents:\n");
            for (doc, reason) in rejected.iter().take(10) {
                // 최대 10개만 표시
                rationale.push_str(&format!("- **{}**: {}\n", doc.title, reason));
            }

            if rejected.len() > 10 {
                rationale.push_str(&format!(
                    "- ... and {} more documents\n",
                    rejected.len() - 10
                ));
            }
        }

        rationale
    }
}

impl Default for DocumentSelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_scored_candidate(
        id: &str,
        title: &str,
        tokens: usize,
        score: f32,
    ) -> ScoredCandidate {
        let candidate = DocumentCandidate {
            doc_id: id.to_string(),
            sha: "test-sha".to_string(),
            title: title.to_string(),
            locale: "ko".to_string(),
            trust: 0.8,
            freshness: "2025-01-01T00:00:00Z".to_string(),
            confidence: 0.8,
            path: format!("{}.md", id),
            facets: HashMap::new(),
            tokens,
            content: None,
        };

        let mut scored = ScoredCandidate::new(candidate);
        scored.base_score = score;
        scored.freshness_score = 0.8;
        scored.trust_score = 0.8;
        scored.confidence_score = 0.8;
        scored.facet_coverage_score = 0.8;

        // Update breakdown for compatibility
        scored.score_breakdown.freshness_score = 0.8;
        scored.score_breakdown.trust_score = 0.8;
        scored.score_breakdown.confidence_score = 0.8;
        scored.score_breakdown.facet_coverage_score = 0.8;

        scored
    }

    #[test]
    fn test_document_selector_creation() -> Result<()> {
        // Test basic constructor
        let selector = DocumentSelector::new();
        assert_eq!(selector.mmr_lambda, DEFAULT_MMR_LAMBDA);

        // Test with custom lambda
        let custom_lambda = 0.5;
        let selector_custom = DocumentSelector::with_mmr_lambda(custom_lambda);
        assert_eq!(selector_custom.mmr_lambda, custom_lambda);

        // Test with weights
        let weights = ScoringWeights::default();
        let selector_weights = DocumentSelector::with_weights(weights);
        assert_eq!(selector_weights.mmr_lambda, DEFAULT_MMR_LAMBDA);

        Ok(())
    }

    #[test]
    fn test_select_documents_interface() -> Result<()> {
        let selector = DocumentSelector::new();

        let candidates = vec![
            create_test_scored_candidate("high_value", "High Value Doc", 80, 10.0),
            create_test_scored_candidate("low_value", "Low Value Doc", 90, 2.0),
        ];

        let budget = 100;
        let reserve = 10;
        let selected = selector.select_documents(candidates, budget, reserve)?;

        // Should select the high value document within budget
        assert!(!selected.is_empty());
        assert!(selected[0].candidate.doc_id == "high_value");

        Ok(())
    }

    #[test]
    fn test_knapsack_selection_basic() -> Result<()> {
        let selector = DocumentSelector::new();

        let candidates = vec![
            create_test_scored_candidate("high_value", "High Value Doc", 80, 10.0),
            create_test_scored_candidate("low_value", "Low Value Doc", 90, 2.0),
        ];

        let (chosen, _rejected) = selector.knapsack_selection(candidates, 100)?;

        // 높은 가치의 문서만 선택되어야 함
        assert_eq!(chosen.len(), 1);
        assert_eq!(chosen[0].doc_id, "high_value");

        Ok(())
    }

    #[test]
    fn test_greedy_selection_efficiency() -> Result<()> {
        let selector = DocumentSelector::new();

        let candidates = vec![
            create_test_scored_candidate("efficient", "Efficient Doc", 50, 5.0), // 효율성: 0.1
            create_test_scored_candidate("inefficient", "Inefficient Doc", 100, 6.0), // 효율성: 0.06
        ];

        let (chosen, _rejected) = selector.greedy_selection_legacy(candidates, 150)?;

        // 효율성이 높은 문서가 먼저 선택되어야 함
        assert_eq!(chosen.len(), 2);
        assert_eq!(chosen[0].doc_id, "efficient");

        Ok(())
    }

    #[test]
    fn test_mmr_algorithm_correctness() -> Result<()> {
        let selector = DocumentSelector::with_mmr_lambda(0.5); // 50% relevance, 50% diversity

        // 유사한 문서들 생성 (같은 facet을 가짐)
        let mut facets = std::collections::HashMap::new();
        facets.insert("topic".to_string(), vec!["rust".to_string()]);

        let mut candidates = vec![
            create_test_scored_candidate("high_score_1", "Rust Programming Guide", 100, 10.0),
            create_test_scored_candidate("high_score_2", "Rust Advanced Guide", 100, 9.0),
            create_test_scored_candidate("different", "Python Tutorial", 100, 8.0),
        ];

        // 첫 두 문서를 유사하게 만들기
        candidates[0].candidate.facets = facets.clone();
        candidates[1].candidate.facets = facets;

        let selected = selector.select_documents(candidates, 250, 0)?;

        // MMR은 다양성을 고려하므로 유사한 두 문서보다는 다른 문서를 선택할 가능성이 높음
        assert!(!selected.is_empty());
        assert!(selected.len() <= 3);

        Ok(())
    }

    #[test]
    fn test_mmr_diversity_calculation() -> Result<()> {
        let selector = DocumentSelector::with_mmr_lambda(0.8); // 80% diversity weight

        let candidates = vec![
            create_test_scored_candidate("doc1", "Document 1", 50, 5.0),
            create_test_scored_candidate("doc2", "Document 2", 50, 4.0),
            create_test_scored_candidate("doc3", "Document 3", 50, 3.0),
        ];

        let selected = selector.select_documents(candidates, 150, 0)?;

        // 높은 다양성 가중치로 인해 모든 문서가 선택될 가능성이 높음
        assert!(selected.len() >= 2);

        Ok(())
    }

    #[test]
    fn test_lambda_parameter_from_constants() -> Result<()> {
        let selector = DocumentSelector::new();

        // DEFAULT_MMR_LAMBDA 상수가 사용되는지 확인
        assert_eq!(selector.mmr_lambda, DEFAULT_MMR_LAMBDA);

        // 커스텀 lambda 설정 확인
        let custom_selector = DocumentSelector::with_mmr_lambda(0.7);
        assert_eq!(custom_selector.mmr_lambda, 0.7);

        Ok(())
    }

    #[test]
    fn test_token_budget_constraint_enforcement() -> Result<()> {
        let selector = DocumentSelector::new();

        let candidates = vec![
            create_test_scored_candidate("large_doc", "Large Document", 150, 10.0),
            create_test_scored_candidate("small_doc", "Small Document", 50, 8.0),
        ];

        let budget = 100; // Only small doc should fit
        let selected = selector.select_documents(candidates, budget, 0)?;

        // 예산 제약으로 인해 작은 문서만 선택되어야 함
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].candidate.doc_id, "small_doc");

        Ok(())
    }

    #[test]
    fn test_greedy_fallback_for_small_sets() -> Result<()> {
        let selector = DocumentSelector::new();

        // GREEDY_SELECTION_THRESHOLD 이하의 문서 수
        let candidates = vec![
            create_test_scored_candidate("doc1", "Document 1", 50, 10.0),
            create_test_scored_candidate("doc2", "Document 2", 50, 8.0),
        ];

        let selected = selector.select_documents(candidates, 150, 0)?;

        // 작은 집합에서는 그리디 선택이 사용되어야 함
        assert_eq!(selected.len(), 2);
        // 높은 점수 순으로 정렬되어야 함
        assert!(selected[0].base_score >= selected[1].base_score);

        Ok(())
    }

    #[test]
    fn test_budget_exceeded_truncation() -> Result<()> {
        let selector = DocumentSelector::new();

        let candidates = vec![
            create_test_scored_candidate("doc1", "Document 1", 80, 10.0),
            create_test_scored_candidate("doc2", "Document 2", 80, 9.0),
            create_test_scored_candidate("doc3", "Document 3", 80, 8.0),
        ];

        let budget = 150; // Only 1-2 documents should fit
        let selected = selector.select_documents(candidates, budget, 0)?;

        // 예산 초과 시 일부 문서만 선택
        assert!(selected.len() <= 2);

        // 선택된 문서들의 총 토큰이 예산을 초과하지 않아야 함
        let total_tokens: usize = selected.iter().map(|s| s.candidate.tokens).sum();
        assert!(total_tokens <= budget);

        Ok(())
    }
}
