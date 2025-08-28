//! 유사성 계산 캐싱
//!
//! 문서 간 유사성 계산 결과를 캐싱하여 중복 계산을 방지합니다.

use super::{CacheKeyGenerator, CacheManager};
use crate::domain::constants::determinism::SIMILARITY_THRESHOLD;
use crate::domain::errors::ContextError;
use std::collections::HashMap;

/// 유사성 계산 캐싱 매니저
pub struct SimilarityCacheManager {
    cache_manager: &'static CacheManager,
}

impl SimilarityCacheManager {
    /// 새로운 유사성 캐싱 매니저 생성
    pub fn new() -> Self {
        Self {
            cache_manager: super::global_cache_manager(),
        }
    }

    /// 두 문서 간 유사성 점수 조회 (캐시 우선)
    pub fn get_similarity(
        &self,
        doc1_content: &str,
        doc1_metadata: &HashMap<String, String>,
        doc2_content: &str,
        doc2_metadata: &HashMap<String, String>,
    ) -> Option<f32> {
        let doc1_hash = CacheKeyGenerator::document_hash(doc1_content, doc1_metadata);
        let doc2_hash = CacheKeyGenerator::document_hash(doc2_content, doc2_metadata);

        self.cache_manager.get_similarity(doc1_hash, doc2_hash)
    }

    /// 두 문서 간 유사성 점수 저장
    pub fn put_similarity(
        &self,
        doc1_content: &str,
        doc1_metadata: &HashMap<String, String>,
        doc2_content: &str,
        doc2_metadata: &HashMap<String, String>,
        similarity: f32,
    ) {
        let doc1_hash = CacheKeyGenerator::document_hash(doc1_content, doc1_metadata);
        let doc2_hash = CacheKeyGenerator::document_hash(doc2_content, doc2_metadata);

        self.cache_manager
            .put_similarity(doc1_hash, doc2_hash, similarity);
    }

    /// 문서 해시로 직접 유사성 조회
    pub fn get_similarity_by_hash(&self, doc1_hash: u64, doc2_hash: u64) -> Option<f32> {
        self.cache_manager.get_similarity(doc1_hash, doc2_hash)
    }

    /// 문서 해시로 직접 유사성 저장
    pub fn put_similarity_by_hash(&self, doc1_hash: u64, doc2_hash: u64, similarity: f32) {
        self.cache_manager
            .put_similarity(doc1_hash, doc2_hash, similarity);
    }

    /// 문서와 유사한 문서들을 임계값 이상으로 찾기
    pub fn find_similar_documents(
        &self,
        target_content: &str,
        target_metadata: &HashMap<String, String>,
        candidate_documents: &[(String, HashMap<String, String>)],
        threshold: Option<f32>,
    ) -> Vec<(usize, f32)> {
        let threshold = threshold.unwrap_or(SIMILARITY_THRESHOLD);
        let target_hash = CacheKeyGenerator::document_hash(target_content, target_metadata);

        let mut similar_docs = Vec::new();

        for (idx, (content, metadata)) in candidate_documents.iter().enumerate() {
            let candidate_hash = CacheKeyGenerator::document_hash(content, metadata);

            if let Some(similarity) = self
                .cache_manager
                .get_similarity(target_hash, candidate_hash)
            {
                if similarity >= threshold {
                    similar_docs.push((idx, similarity));
                }
            }
        }

        // 유사도 내림차순 정렬
        similar_docs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        similar_docs
    }

    /// 배치 유사성 계산 및 캐싱
    pub fn compute_and_cache_batch_similarities<F>(
        &self,
        documents: &[(String, HashMap<String, String>)],
        similarity_fn: F,
    ) -> Result<HashMap<(usize, usize), f32>, ContextError>
    where
        F: Fn(
            &str,
            &HashMap<String, String>,
            &str,
            &HashMap<String, String>,
        ) -> Result<f32, ContextError>,
    {
        let mut results = HashMap::new();
        let doc_count = documents.len();

        // 문서 해시들 미리 계산
        let doc_hashes: Vec<u64> = documents
            .iter()
            .map(|(content, metadata)| CacheKeyGenerator::document_hash(content, metadata))
            .collect();

        for i in 0..doc_count {
            for j in (i + 1)..doc_count {
                let hash_i = doc_hashes[i];
                let hash_j = doc_hashes[j];

                // 캐시에서 먼저 확인
                let similarity =
                    if let Some(cached) = self.cache_manager.get_similarity(hash_i, hash_j) {
                        cached
                    } else {
                        // 캐시에 없으면 계산
                        let (content_i, metadata_i) = &documents[i];
                        let (content_j, metadata_j) = &documents[j];

                        let computed = similarity_fn(content_i, metadata_i, content_j, metadata_j)?;

                        // 캐시에 저장
                        self.cache_manager.put_similarity(hash_i, hash_j, computed);
                        computed
                    };

                results.insert((i, j), similarity);
            }
        }

        Ok(results)
    }

    /// 유사성 행렬 생성 (캐시 활용)
    pub fn build_similarity_matrix<F>(
        &self,
        documents: &[(String, HashMap<String, String>)],
        similarity_fn: F,
    ) -> Result<Vec<Vec<f32>>, ContextError>
    where
        F: Fn(
            &str,
            &HashMap<String, String>,
            &str,
            &HashMap<String, String>,
        ) -> Result<f32, ContextError>,
    {
        let doc_count = documents.len();
        let mut matrix = vec![vec![0.0; doc_count]; doc_count];

        // 대각선은 1.0 (자기 자신과의 유사도)
        for (i, row) in matrix.iter_mut().enumerate().take(doc_count) {
            row[i] = 1.0;
        }

        let similarities = self.compute_and_cache_batch_similarities(documents, similarity_fn)?;

        // 상삼각 행렬 채우기 (대칭이므로 하삼각도 동일)
        for ((i, j), similarity) in similarities {
            matrix[i][j] = similarity;
            matrix[j][i] = similarity;
        }

        Ok(matrix)
    }

    /// 캐시 히트율 계산
    pub fn calculate_hit_rate(&self, attempted_lookups: usize, cache_hits: usize) -> f64 {
        if attempted_lookups == 0 {
            0.0
        } else {
            cache_hits as f64 / attempted_lookups as f64
        }
    }

    /// 유사성 캐시 워밍업 (자주 사용되는 문서들 간 유사성 미리 계산)
    pub fn warm_up_cache<F>(
        &self,
        frequent_documents: &[(String, HashMap<String, String>)],
        similarity_fn: F,
    ) -> Result<usize, ContextError>
    where
        F: Fn(
            &str,
            &HashMap<String, String>,
            &str,
            &HashMap<String, String>,
        ) -> Result<f32, ContextError>,
    {
        let similarities =
            self.compute_and_cache_batch_similarities(frequent_documents, similarity_fn)?;
        Ok(similarities.len())
    }

    /// 특정 문서와 다른 모든 문서들 간의 유사성 조회
    pub fn get_document_similarities(
        &self,
        target_content: &str,
        target_metadata: &HashMap<String, String>,
        candidate_documents: &[(String, HashMap<String, String>)],
    ) -> Vec<Option<f32>> {
        let target_hash = CacheKeyGenerator::document_hash(target_content, target_metadata);

        candidate_documents
            .iter()
            .map(|(content, metadata)| {
                let candidate_hash = CacheKeyGenerator::document_hash(content, metadata);
                self.cache_manager
                    .get_similarity(target_hash, candidate_hash)
            })
            .collect()
    }

    /// 캐시 통계 조회
    pub fn get_cache_stats(&self) -> super::CacheStats {
        self.cache_manager.get_all_stats().similarity
    }
}

impl Default for SimilarityCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 유사성 계산 결과
#[derive(Debug, Clone)]
pub struct SimilarityResult {
    pub similarity: f32,
    pub was_cached: bool,
    pub computation_time_ms: Option<u64>,
}

impl SimilarityResult {
    pub fn cached(similarity: f32) -> Self {
        Self {
            similarity,
            was_cached: true,
            computation_time_ms: None,
        }
    }

    pub fn computed(similarity: f32, computation_time_ms: u64) -> Self {
        Self {
            similarity,
            was_cached: false,
            computation_time_ms: Some(computation_time_ms),
        }
    }
}

/// 유사성 계산 통계
#[derive(Debug, Clone, Default)]
pub struct SimilarityStats {
    pub total_requests: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub total_computation_time_ms: u64,
    pub average_computation_time_ms: f64,
}

impl SimilarityStats {
    pub fn hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_requests as f64
        }
    }

    pub fn add_hit(&mut self) {
        self.total_requests += 1;
        self.cache_hits += 1;
    }

    pub fn add_miss(&mut self, computation_time_ms: u64) {
        self.total_requests += 1;
        self.cache_misses += 1;
        self.total_computation_time_ms += computation_time_ms;
        self.average_computation_time_ms =
            self.total_computation_time_ms as f64 / self.cache_misses as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_similarity_fn(
        _content1: &str,
        _metadata1: &HashMap<String, String>,
        _content2: &str,
        _metadata2: &HashMap<String, String>,
    ) -> Result<f32, ContextError> {
        Ok(0.75) // 더미 유사도
    }

    #[test]
    fn test_similarity_cache_basic() {
        let cache_manager = SimilarityCacheManager::new();

        let metadata1 = HashMap::new();
        let metadata2 = HashMap::new();

        // 캐시에 없음
        assert_eq!(
            cache_manager.get_similarity("doc1", &metadata1, "doc2", &metadata2),
            None
        );

        // 캐시에 저장
        cache_manager.put_similarity("doc1", &metadata1, "doc2", &metadata2, 0.85);

        // 캐시에서 조회
        assert_eq!(
            cache_manager.get_similarity("doc1", &metadata1, "doc2", &metadata2),
            Some(0.85)
        );
        assert_eq!(
            cache_manager.get_similarity("doc2", &metadata2, "doc1", &metadata1),
            Some(0.85)
        );
    }

    #[test]
    fn test_batch_similarity_computation() {
        let cache_manager = SimilarityCacheManager::new();

        let documents = vec![
            ("document 1 content".to_string(), HashMap::new()),
            ("document 2 content".to_string(), HashMap::new()),
            ("document 3 content".to_string(), HashMap::new()),
        ];

        let results = cache_manager
            .compute_and_cache_batch_similarities(&documents, dummy_similarity_fn)
            .expect("Failed to compute similarities");

        assert_eq!(results.len(), 3); // (0,1), (0,2), (1,2)
        assert!(results.contains_key(&(0, 1)));
        assert!(results.contains_key(&(0, 2)));
        assert!(results.contains_key(&(1, 2)));
    }

    #[test]
    fn test_similarity_matrix() {
        // 새로운 캐시 매니저로 격리된 테스트 환경 생성
        let cache_manager = SimilarityCacheManager::new();

        // 고유한 문서 ID로 캐시 충돌 방지
        let documents = vec![
            ("matrix_test_doc1".to_string(), HashMap::new()),
            ("matrix_test_doc2".to_string(), HashMap::new()),
        ];

        let matrix = cache_manager
            .build_similarity_matrix(&documents, dummy_similarity_fn)
            .expect("Failed to build matrix");

        assert_eq!(matrix.len(), 2);
        assert_eq!(matrix[0].len(), 2);
        assert_eq!(matrix[0][0], 1.0); // 자기 자신
        assert_eq!(matrix[1][1], 1.0); // 자기 자신

        // dummy_similarity_fn이 반환하는 값 확인
        let expected_similarity =
            dummy_similarity_fn("", &HashMap::new(), "", &HashMap::new()).unwrap();
        assert_eq!(matrix[0][1], expected_similarity); // 계산된 유사도
        assert_eq!(matrix[1][0], expected_similarity); // 대칭
    }

    #[test]
    fn test_find_similar_documents() {
        let cache_manager = SimilarityCacheManager::new();

        let target_metadata = HashMap::new();
        let candidates = vec![
            ("similar doc".to_string(), HashMap::new()),
            ("different doc".to_string(), HashMap::new()),
        ];

        // 유사성 미리 캐싱
        cache_manager.put_similarity(
            "target",
            &target_metadata,
            "similar doc",
            &HashMap::new(),
            0.9,
        );
        cache_manager.put_similarity(
            "target",
            &target_metadata,
            "different doc",
            &HashMap::new(),
            0.3,
        );

        let similar = cache_manager.find_similar_documents(
            "target",
            &target_metadata,
            &candidates,
            Some(0.5),
        );

        assert_eq!(similar.len(), 1);
        assert_eq!(similar[0].0, 0); // 첫 번째 문서
        assert_eq!(similar[0].1, 0.9);
    }
}
