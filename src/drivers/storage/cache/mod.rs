//! 계산 캐싱 시스템
//!
//! 유사성 계산, 점수 계산 등 비용이 많이 드는 연산들을 캐싱하여 성능을 최적화합니다.

use crate::domain::constants::defaults::*;
use crate::domain::errors::ContextError;
use ahash::AHasher;
use lru::LruCache;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub mod scoring;
pub mod similarity;

/// 캐시 항목
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub value: T,
    pub created_at: Instant,
    pub access_count: u64,
    pub last_accessed: Instant,
}

impl<T> CacheEntry<T> {
    pub fn new(value: T) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            access_count: 1,
            last_accessed: now,
        }
    }

    pub fn access(&mut self) -> &T {
        self.access_count += 1;
        self.last_accessed = Instant::now();
        &self.value
    }

    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

/// 메모리 기반 LRU 캐시
pub struct MemoryCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    cache: Arc<RwLock<LruCache<K, CacheEntry<V>>>>,
    ttl: Duration,
    max_size: usize,
}

impl<K, V> MemoryCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(max_size: usize, ttl_secs: u64) -> Result<Self, ContextError> {
        let capacity = NonZeroUsize::new(max_size.max(1))
            .ok_or_else(|| ContextError::ConfigError("Cache size must be positive".into()))?;
            
        Ok(Self {
            cache: Arc::new(RwLock::new(LruCache::new(capacity))),
            ttl: Duration::from_secs(ttl_secs),
            max_size,
        })
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.write().ok()?;

        if let Some(entry) = cache.get_mut(key) {
            if entry.is_expired(self.ttl) {
                cache.pop(key);
                None
            } else {
                Some(entry.access().clone())
            }
        } else {
            None
        }
    }

    pub fn put(&self, key: K, value: V) {
        if let Ok(mut cache) = self.cache.write() {
            cache.put(key, CacheEntry::new(value));
        }
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        self.cache.write().ok()?.pop(key).map(|entry| entry.value)
    }

    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.cache.read().map(|cache| cache.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.max_size
    }

    /// 만료된 항목들 정리
    pub fn cleanup_expired(&self) -> usize {
        let mut cache = match self.cache.write() {
            Ok(cache) => cache,
            Err(_) => return 0,
        };

        let mut expired_keys = Vec::new();

        // 만료된 키들 수집
        for (key, entry) in cache.iter() {
            if entry.is_expired(self.ttl) {
                expired_keys.push(key.clone());
            }
        }

        // 만료된 항목들 제거
        let removed_count = expired_keys.len();
        for key in expired_keys {
            cache.pop(&key);
        }

        removed_count
    }

    /// 캐시 통계
    pub fn stats(&self) -> CacheStats {
        let cache = match self.cache.read() {
            Ok(cache) => cache,
            Err(_) => return CacheStats::default(),
        };

        let mut total_access_count = 0;
        let mut expired_count = 0;

        for entry in cache.iter().map(|(_, entry)| entry) {
            total_access_count += entry.access_count;
            if entry.is_expired(self.ttl) {
                expired_count += 1;
            }
        }

        CacheStats {
            size: cache.len(),
            capacity: self.max_size,
            total_access_count,
            expired_count,
            hit_rate: 0.0, // 별도로 추적 필요
        }
    }
}

/// 캐시 통계
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub size: usize,
    pub capacity: usize,
    pub total_access_count: u64,
    pub expired_count: usize,
    pub hit_rate: f64,
}

impl CacheStats {
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            self.size as f64 / self.capacity as f64
        }
    }
}

/// 해시 기반 캐시 키 생성기
pub struct CacheKeyGenerator;

impl CacheKeyGenerator {
    /// 문자열들로부터 캐시 키 생성
    pub fn generate_key(parts: &[&str]) -> u64 {
        let mut hasher = AHasher::default();
        for part in parts {
            part.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// 값들로부터 캐시 키 생성
    pub fn generate_key_from_values<T: Hash>(values: &[T]) -> u64 {
        let mut hasher = AHasher::default();
        for value in values {
            value.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// 문서 내용과 메타데이터로부터 문서 해시 생성
    pub fn document_hash(content: &str, metadata: &HashMap<String, String>) -> u64 {
        let mut hasher = AHasher::default();
        content.hash(&mut hasher);

        // 메타데이터는 정렬하여 일관된 해시 생성
        let mut sorted_metadata: Vec<_> = metadata.iter().collect();
        sorted_metadata.sort_by_key(|(k, _)| *k);

        for (key, value) in sorted_metadata {
            key.hash(&mut hasher);
            value.hash(&mut hasher);
        }

        hasher.finish()
    }

    /// 검색 쿼리 해시 생성
    pub fn query_hash(text: &str, facets: &[String], metadata: &HashMap<String, String>) -> u64 {
        let mut hasher = AHasher::default();
        text.hash(&mut hasher);

        // 패싯 정렬
        let mut sorted_facets = facets.to_vec();
        sorted_facets.sort();
        sorted_facets.hash(&mut hasher);

        // 메타데이터 정렬
        let mut sorted_metadata: Vec<_> = metadata.iter().collect();
        sorted_metadata.sort_by_key(|(k, _)| *k);
        sorted_metadata.hash(&mut hasher);

        hasher.finish()
    }
}

/// 캐시 매니저 - 여러 캐시들을 통합 관리
pub struct CacheManager {
    similarity_cache: MemoryCache<u64, f32>,
    score_cache: MemoryCache<u64, f32>,
    document_cache: MemoryCache<u64, String>, // 문서 내용 캐시
    query_result_cache: MemoryCache<u64, Vec<String>>, // 쿼리 결과 캐시
}

impl CacheManager {
    pub fn new() -> Result<Self, ContextError> {
        Ok(Self {
            similarity_cache: MemoryCache::new(
                DEFAULT_SIMILARITY_CACHE_SIZE,
                DEFAULT_CACHE_TTL_SECS,
            )?,
            score_cache: MemoryCache::new(DEFAULT_SCORE_CACHE_SIZE, DEFAULT_CACHE_TTL_SECS)?,
            document_cache: MemoryCache::new(
                DEFAULT_CACHE_SIZE,
                DEFAULT_CACHE_TTL_SECS * 2, // 문서는 더 오래 캐싱
            )?,
            query_result_cache: MemoryCache::new(
                DEFAULT_CACHE_SIZE / 2,
                DEFAULT_CACHE_TTL_SECS / 2, // 쿼리 결과는 짧게 캐싱
            )?,
        })
    }

    /// 유사성 점수 캐싱
    pub fn get_similarity(&self, doc1_hash: u64, doc2_hash: u64) -> Option<f32> {
        let key = if doc1_hash < doc2_hash {
            CacheKeyGenerator::generate_key_from_values(&[doc1_hash, doc2_hash])
        } else {
            CacheKeyGenerator::generate_key_from_values(&[doc2_hash, doc1_hash])
        };
        self.similarity_cache.get(&key)
    }

    pub fn put_similarity(&self, doc1_hash: u64, doc2_hash: u64, similarity: f32) {
        let key = if doc1_hash < doc2_hash {
            CacheKeyGenerator::generate_key_from_values(&[doc1_hash, doc2_hash])
        } else {
            CacheKeyGenerator::generate_key_from_values(&[doc2_hash, doc1_hash])
        };
        self.similarity_cache.put(key, similarity);
    }

    /// 점수 캐싱
    pub fn get_score(&self, doc_hash: u64, query_hash: u64) -> Option<f32> {
        let key = CacheKeyGenerator::generate_key_from_values(&[doc_hash, query_hash]);
        self.score_cache.get(&key)
    }

    pub fn put_score(&self, doc_hash: u64, query_hash: u64, score: f32) {
        let key = CacheKeyGenerator::generate_key_from_values(&[doc_hash, query_hash]);
        self.score_cache.put(key, score);
    }

    /// 문서 내용 캐싱
    pub fn get_document(&self, doc_hash: u64) -> Option<String> {
        self.document_cache.get(&doc_hash)
    }

    pub fn put_document(&self, doc_hash: u64, content: String) {
        self.document_cache.put(doc_hash, content);
    }

    /// 쿼리 결과 캐싱
    pub fn get_query_result(&self, query_hash: u64) -> Option<Vec<String>> {
        self.query_result_cache.get(&query_hash)
    }

    pub fn put_query_result(&self, query_hash: u64, result: Vec<String>) {
        self.query_result_cache.put(query_hash, result);
    }

    /// 모든 캐시 정리
    pub fn cleanup_all(&self) -> CacheCleanupStats {
        let similarity_cleaned = self.similarity_cache.cleanup_expired();
        let score_cleaned = self.score_cache.cleanup_expired();
        let document_cleaned = self.document_cache.cleanup_expired();
        let query_result_cleaned = self.query_result_cache.cleanup_expired();

        CacheCleanupStats {
            similarity_cleaned,
            score_cleaned,
            document_cleaned,
            query_result_cleaned,
            total_cleaned: similarity_cleaned
                + score_cleaned
                + document_cleaned
                + query_result_cleaned,
        }
    }

    /// 모든 캐시 통계
    pub fn get_all_stats(&self) -> AllCacheStats {
        AllCacheStats {
            similarity: self.similarity_cache.stats(),
            score: self.score_cache.stats(),
            document: self.document_cache.stats(),
            query_result: self.query_result_cache.stats(),
        }
    }

    /// 모든 캐시 클리어
    pub fn clear_all(&self) {
        self.similarity_cache.clear();
        self.score_cache.clear();
        self.document_cache.clear();
        self.query_result_cache.clear();
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default CacheManager")
    }
}

/// 캐시 정리 통계
#[derive(Debug, Clone)]
pub struct CacheCleanupStats {
    pub similarity_cleaned: usize,
    pub score_cleaned: usize,
    pub document_cleaned: usize,
    pub query_result_cleaned: usize,
    pub total_cleaned: usize,
}

/// 모든 캐시 통계
#[derive(Debug, Clone)]
pub struct AllCacheStats {
    pub similarity: CacheStats,
    pub score: CacheStats,
    pub document: CacheStats,
    pub query_result: CacheStats,
}

impl AllCacheStats {
    pub fn total_size(&self) -> usize {
        self.similarity.size + self.score.size + self.document.size + self.query_result.size
    }

    pub fn total_capacity(&self) -> usize {
        self.similarity.capacity
            + self.score.capacity
            + self.document.capacity
            + self.query_result.capacity
    }

    pub fn overall_utilization(&self) -> f64 {
        let total_capacity = self.total_capacity();
        if total_capacity == 0 {
            0.0
        } else {
            self.total_size() as f64 / total_capacity as f64
        }
    }
}

// 전역 캐시 매니저
lazy_static::lazy_static! {
    static ref GLOBAL_CACHE_MANAGER: CacheManager = 
        CacheManager::new().expect("Failed to initialize global cache manager");
}

/// 전역 캐시 매니저에 접근
pub fn global_cache_manager() -> &'static CacheManager {
    &GLOBAL_CACHE_MANAGER
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_memory_cache_basic() {
        let cache: MemoryCache<String, i32> = MemoryCache::new(3, 1).unwrap();

        cache.put("key1".to_string(), 10);
        cache.put("key2".to_string(), 20);
        cache.put("key3".to_string(), 30);

        assert_eq!(cache.get(&"key1".to_string()), Some(10));
        assert_eq!(cache.get(&"key2".to_string()), Some(20));
        assert_eq!(cache.get(&"key3".to_string()), Some(30));
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn test_cache_lru_eviction() {
        let cache: MemoryCache<String, i32> = MemoryCache::new(2, 60).unwrap();

        cache.put("key1".to_string(), 10);
        cache.put("key2".to_string(), 20);
        cache.put("key3".to_string(), 30); // should evict key1

        assert_eq!(cache.get(&"key1".to_string()), None);
        assert_eq!(cache.get(&"key2".to_string()), Some(20));
        assert_eq!(cache.get(&"key3".to_string()), Some(30));
    }

    #[test]
    fn test_cache_expiration() {
        let cache: MemoryCache<String, i32> = MemoryCache::new(10, 1).unwrap(); // 1 second TTL

        cache.put("key1".to_string(), 10);
        assert_eq!(cache.get(&"key1".to_string()), Some(10));

        thread::sleep(Duration::from_secs(2));
        assert_eq!(cache.get(&"key1".to_string()), None);
    }

    #[test]
    fn test_cache_key_generator() {
        let key1 = CacheKeyGenerator::generate_key(&["doc1", "doc2"]);
        let key2 = CacheKeyGenerator::generate_key(&["doc1", "doc2"]);
        let key3 = CacheKeyGenerator::generate_key(&["doc2", "doc1"]);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cache_manager() {
        let manager = CacheManager::new().unwrap();

        // 유사성 캐싱 테스트
        assert_eq!(manager.get_similarity(1, 2), None);
        manager.put_similarity(1, 2, 0.85);
        assert_eq!(manager.get_similarity(1, 2), Some(0.85));
        assert_eq!(manager.get_similarity(2, 1), Some(0.85)); // 순서 무관

        // 점수 캐싱 테스트
        manager.put_score(100, 200, 0.75);
        assert_eq!(manager.get_score(100, 200), Some(0.75));

        // 통계 확인
        let stats = manager.get_all_stats();
        assert!(stats.similarity.size > 0);
        assert!(stats.score.size > 0);
    }

    #[test]
    fn test_document_hash() {
        let mut metadata1 = HashMap::new();
        metadata1.insert("author".to_string(), "Alice".to_string());
        metadata1.insert("version".to_string(), "1.0".to_string());

        let mut metadata2 = HashMap::new();
        metadata2.insert("version".to_string(), "1.0".to_string());
        metadata2.insert("author".to_string(), "Alice".to_string());

        let hash1 = CacheKeyGenerator::document_hash("content", &metadata1);
        let hash2 = CacheKeyGenerator::document_hash("content", &metadata2);

        assert_eq!(hash1, hash2); // 메타데이터 순서는 영향 없음
    }
}
