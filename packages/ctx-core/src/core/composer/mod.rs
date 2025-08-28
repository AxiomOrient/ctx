pub mod merger;
pub mod query;
pub mod scorer;
pub mod selector;
pub mod types;

#[cfg(test)]
mod tests;

pub use crate::common::MergedDocument;
pub use merger::{DocumentMerger, MergeOptions};
pub use scorer::{DocumentScorer, QuerySignals, ScoringWeights};
pub use selector::{DocumentSelector, MMRSelector, SelectionResult};
pub use types::{BuildQuery, DocumentCandidate};

use crate::Result;
use crate::common::CompositionResult;
use crate::core::composer::scorer::ScoredCandidate;
use sha2::{Digest, Sha256};

/// 빌드 컴포저 - MMR + 배낭 알고리즘으로 문서 조합
///
/// 이 컴포저는 다음 단계로 문서를 조합합니다:
/// 1. 문서별 기본 점수 계산 (키워드, 신선도, 신뢰도 등)
/// 2. MMR(Maximal Marginal Relevance)로 중복 억제하며 선택
/// 3. 선택된 문서들을 병합하여 최종 프롬프트 생성
///
/// # Examples
///
/// ```rust
/// use ctx_core::core::composer::{BuildComposer, BuildQuery};
///
/// let composer = BuildComposer::new();
/// let query = BuildQuery::new("repo".to_string(), "main".to_string(), "abc123".to_string());
/// // let result = composer.compose(candidates, &query, 2000, 200)?;
/// ```
pub struct BuildComposer {
    scorer: DocumentScorer,
    selector: DocumentSelector,
    merger: DocumentMerger,
}

impl BuildComposer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            scorer: DocumentScorer::new(),
            selector: DocumentSelector::new(),
            merger: DocumentMerger::new()?,
        })
    }

    /// 문서들을 조합하여 최적화된 프롬프트 생성
    ///
    /// # Arguments
    ///
    /// * `candidates` - 후보 문서들
    /// * `query` - 빌드 쿼리 (필수 패싯, 신뢰도 임계값 등)
    /// * `budget` - 총 토큰 예산
    /// * `reserve` - 예약 토큰 (시스템 메시지 등을 위한)
    ///
    /// # Returns
    ///
    /// 선택된 문서들의 병합 결과와 선택 근거
    ///
    /// # Errors
    ///
    /// 스코어링, 선택, 또는 병합 과정에서 오류가 발생할 수 있습니다.
    pub fn compose(
        &self,
        candidates: Vec<DocumentCandidate>,
        query: &BuildQuery,
        budget: usize,
        reserve: usize,
    ) -> Result<CompositionResult> {
        // 1. 문서별 기본 점수 계산
        let scored_candidates = self.scorer.score_documents(&candidates, query)?;

        // 2. MMR로 중복 억제하며 선택
        let selection = self.selector.select_with_mmr(
            scored_candidates,
            budget.saturating_sub(reserve),
            query,
        )?;

        // 3. 선택된 문서들을 병합 (쿼리 메타데이터 포함 포맷)
        // 선택된 문서들과 스코어 정보를 매칭
        let id_set: std::collections::HashSet<String> =
            selection.chosen.iter().map(|c| c.doc_id.clone()).collect();

        // 재스코어 없이 기존 스코어 목록에서 선택된 것만 추출
        let scored_map: std::collections::HashMap<String, ScoredCandidate> = self
            .scorer
            .score_documents(&candidates, query)?
            .into_iter()
            .map(|sc| (sc.candidate.doc_id.clone(), sc))
            .collect();

        let mut chosen_scored: Vec<ScoredCandidate> = Vec::with_capacity(id_set.len());
        for id in id_set {
            if let Some(sc) = scored_map.get(&id) {
                chosen_scored.push(sc.clone());
            }
        }

        let merged = self
            .merger
            .merge_documents_with_query(chosen_scored, query)?;

        // 4. 실행 ID 생성 (결정성): repo+branch+sha+doc_ids+tokens 기반 해시
        let execution_id = self.compute_execution_id(query, &merged);

        // 5. 공용 결과 타입으로 반환 (생성 시각 포함)
        let mut result = CompositionResult::new(merged, execution_id, selection.average_confidence);
        // 선택 근거와 거절 문서를 설정
        result.selection_rationale = selection.rationale;
        result.rejected_documents = selection
            .rejected
            .into_iter()
            .map(|(doc, _reason)| doc)
            .collect();

        Ok(result)
    }
}

impl Default for BuildComposer {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            scorer: DocumentScorer::new(),
            selector: DocumentSelector::new(),
            merger: DocumentMerger::default(),
        })
    }
}

impl BuildComposer {
    fn compute_execution_id(&self, query: &BuildQuery, merged: &MergedDocument) -> String {
        let mut hasher = Sha256::new();
        hasher.update(query.repo.as_bytes());
        hasher.update(query.branch.as_bytes());
        hasher.update(query.commit_sha.as_bytes());
        for id in &merged.source_documents {
            hasher.update(id.as_bytes());
        }
        hasher.update(merged.tokens.to_le_bytes());
        let hex = format!("{:x}", hasher.finalize());
        // truncate to configured length if available
        let len = crate::common::constants::determinism::EXECUTION_ID_LENGTH;
        hex.chars().take(len).collect()
    }
}
