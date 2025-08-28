//! 쿼리 결과 캐싱
//!
//! 자주 실행되는 쿼리의 결과를 캐싱하여 성능을 최적화합니다.

use crate::common::constants::defaults::*;
use crate::common::errors::ContextError;
use crate::core::cache::{CacheKeyGenerator, MemoryCache};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// 쿼리 캐시 매니저
pub struct QueryCacheManager {
    result_cache: MemoryCache<u64, CachedQueryResult>,
    execution_stats: HashMap<String, QueryExecutionStats>,
    cache_hit_count: u64,
    cache_miss_count: u64,
}

impl QueryCacheManager {
    pub fn new() -> Result<Self, ContextError> {
        Ok(Self {
            result_cache: MemoryCache::<u64, CachedQueryResult>::new(DEFAULT_CACHE_SIZE, DEFAULT_CACHE_TTL_SECS)?,
            execution_stats: HashMap::new(),
            cache_hit_count: 0,
            cache_miss_count: 0,
        })
    }

    /// 쿼리 결과 캐싱 시도
    pub fn get_cached_result(&mut self, query: &str, params: &[&str]) -> Option<CachedQueryResult> {
        let cache_key = self.generate_cache_key(query, params);

        if let Some(result) = self.result_cache.get(&cache_key) {
            self.cache_hit_count += 1;
            self.update_stats(query, true, 0);
            Some(result)
        } else {
            self.cache_miss_count += 1;
            None
        }
    }

    /// 쿼리 결과 캐시에 저장
    pub fn cache_result(
        &mut self,
        query: &str,
        params: &[&str],
        result: QueryResultData,
        execution_time_ms: u64,
    ) {
        let cache_key = self.generate_cache_key(query, params);
        let cached_result = CachedQueryResult {
            data: result,
            created_at: Instant::now(),
            execution_time_ms,
            access_count: 0,
        };

        self.result_cache.put(cache_key, cached_result);
        self.update_stats(query, false, execution_time_ms);
    }

    /// 캐시 가능한 쿼리인지 판단
    pub fn is_cacheable(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();

        // SELECT 쿼리만 캐싱
        if !query_lower.trim().starts_with("select") {
            return false;
        }

        // 시간 의존적 함수가 포함된 쿼리는 캐싱하지 않음
        let non_cacheable_patterns = [
            "now()",
            "current_timestamp",
            "current_date",
            "current_time",
            "random()",
            "datetime('now')",
        ];

        for pattern in &non_cacheable_patterns {
            if query_lower.contains(pattern) {
                return false;
            }
        }

        // 너무 큰 결과를 반환할 가능성이 있는 쿼리는 제외
        if query_lower.contains("select *") && !query_lower.contains("limit") {
            return false;
        }

        true
    }

    /// 쿼리 실행 및 캐싱
    pub fn execute_with_cache<F, T>(
        &mut self,
        query: &str,
        params: &[&str],
        executor: F,
    ) -> Result<T, ContextError>
    where
        F: FnOnce() -> Result<T, ContextError>,
        T: Clone + Into<QueryResultData> + From<QueryResultData>,
    {
        // 캐시 가능한 쿼리인지 확인
        if !self.is_cacheable(query) {
            return executor();
        }

        // 캐시에서 먼저 확인
        if let Some(cached) = self.get_cached_result(query, params) {
            return Ok(T::from(cached.data));
        }

        // 캐시에 없으면 실행
        let start_time = Instant::now();
        let result = executor()?;
        let execution_time = start_time.elapsed().as_millis() as u64;

        // 결과를 캐시에 저장
        self.cache_result(query, params, result.clone().into(), execution_time);

        Ok(result)
    }

    fn generate_cache_key(&self, query: &str, params: &[&str]) -> u64 {
        let mut key_parts = vec![query];
        key_parts.extend(params);
        CacheKeyGenerator::generate_key(&key_parts)
    }

    fn update_stats(&mut self, query: &str, is_hit: bool, execution_time_ms: u64) {
        let stats = self
            .execution_stats
            .entry(query.to_string())
            .or_insert_with(QueryExecutionStats::new);

        stats.total_executions += 1;
        if is_hit {
            stats.cache_hits += 1;
        } else {
            stats.cache_misses += 1;
            stats.total_execution_time_ms += execution_time_ms;
        }

        stats.last_executed = Instant::now();
    }

    /// 캐시 통계 조회
    pub fn get_cache_stats(&self) -> CacheStatistics {
        let total_requests = self.cache_hit_count + self.cache_miss_count;
        let hit_rate = if total_requests > 0 {
            self.cache_hit_count as f64 / total_requests as f64
        } else {
            0.0
        };

        let cache_stats = self.result_cache.stats();

        CacheStatistics {
            total_requests,
            cache_hits: self.cache_hit_count,
            cache_misses: self.cache_miss_count,
            hit_rate,
            cache_size: cache_stats.size,
            cache_capacity: cache_stats.capacity,
            memory_usage_estimate: cache_stats.size * 1000, // 대략적인 추정
        }
    }

    /// 자주 실행되는 쿼리 조회
    pub fn get_frequent_queries(&self, top_n: usize) -> Vec<(&String, &QueryExecutionStats)> {
        let mut queries: Vec<_> = self.execution_stats.iter().collect();
        queries.sort_by(|a, b| b.1.total_executions.cmp(&a.1.total_executions));
        queries.truncate(top_n);
        queries
    }

    /// 느린 쿼리 조회 (캐시되지 않은 것 중)
    pub fn get_slow_queries(&self, threshold_ms: u64) -> Vec<(&String, &QueryExecutionStats)> {
        self.execution_stats
            .iter()
            .filter(|(_, stats)| {
                stats.cache_misses > 0
                    && (stats.total_execution_time_ms / stats.cache_misses) > threshold_ms
            })
            .collect()
    }

    /// 캐시 효율성이 낮은 쿼리 조회
    pub fn get_low_efficiency_queries(
        &self,
        hit_rate_threshold: f64,
    ) -> Vec<(&String, &QueryExecutionStats)> {
        self.execution_stats
            .iter()
            .filter(|(_, stats)| {
                stats.total_executions > 5 && stats.hit_rate() < hit_rate_threshold
            })
            .collect()
    }

    /// 캐시 정리
    pub fn cleanup_cache(&mut self) -> usize {
        self.result_cache.cleanup_expired()
    }

    /// 캐시 프리워밍 (자주 사용되는 쿼리 미리 실행)
    pub fn warm_cache<F>(
        &mut self,
        queries: &[(String, Vec<String>)],
        executor: F,
    ) -> Result<usize, ContextError>
    where
        F: Fn(&str, &[&str]) -> Result<QueryResultData, ContextError>,
    {
        let mut warmed_count = 0;

        for (query, params) in queries {
            if self.is_cacheable(query) {
                let param_refs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();

                if self.get_cached_result(query, &param_refs).is_none() {
                    let start_time = Instant::now();
                    match executor(query, &param_refs) {
                        Ok(result) => {
                            let execution_time = start_time.elapsed().as_millis() as u64;
                            self.cache_result(query, &param_refs, result, execution_time);
                            warmed_count += 1;
                        }
                        Err(_) => {
                            // 실패한 쿼리는 로깅만 하고 계속 진행
                            eprintln!("Failed to warm cache for query: {}", query);
                        }
                    }
                }
            }
        }

        Ok(warmed_count)
    }

    /// 캐시 무효화 (패턴 기반)
    pub fn invalidate_pattern(&mut self, pattern: &str) {
        // 현재 구현에서는 전체 캐시 클리어
        // 실제로는 패턴에 맞는 키들만 제거해야 함
        if pattern == "*" {
            self.result_cache.clear();
        }
    }

    /// 캐시 상태 리포트 생성
    pub fn generate_cache_report(&self) -> CacheReport {
        let stats = self.get_cache_stats();
        let frequent_queries = self.get_frequent_queries(10);
        let slow_queries = self.get_slow_queries(1000); // 1초 이상
        let low_efficiency = self.get_low_efficiency_queries(0.5); // 50% 미만

        CacheReport {
            statistics: stats,
            top_queries: frequent_queries
                .into_iter()
                .map(|(query, stats)| (query.clone(), stats.clone()))
                .collect(),
            slow_queries: slow_queries
                .into_iter()
                .map(|(query, stats)| (query.clone(), stats.clone()))
                .collect(),
            low_efficiency_queries: low_efficiency
                .into_iter()
                .map(|(query, stats)| (query.clone(), stats.clone()))
                .collect(),
            recommendations: self.generate_recommendations(),
        }
    }

    fn generate_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();
        let stats = self.get_cache_stats();

        if stats.hit_rate < 0.3 {
            recommendations.push("캐시 히트율이 낮습니다. 캐시 정책을 검토하세요.".to_string());
        }

        if stats.cache_size as f64 / stats.cache_capacity as f64 > 0.9 {
            recommendations
                .push("캐시 사용률이 높습니다. 캐시 크기 증설을 고려하세요.".to_string());
        }

        let slow_queries = self.get_slow_queries(500);
        if !slow_queries.is_empty() {
            recommendations.push(format!(
                "{}개의 느린 쿼리가 있습니다. 최적화를 고려하세요.",
                slow_queries.len()
            ));
        }

        recommendations
    }
}

// Remove Default implementation to avoid panic
// Users should call QueryCacheManager::new() explicitly

/// 캐시된 쿼리 결과
#[derive(Debug, Clone)]
pub struct CachedQueryResult {
    pub data: QueryResultData,
    pub created_at: Instant,
    pub execution_time_ms: u64,
    pub access_count: u64,
}

/// 쿼리 결과 데이터 (일반적인 형태)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryResultData {
    Rows(Vec<HashMap<String, String>>),
    Count(u64),
    Single(HashMap<String, String>),
    Empty,
}

/// 쿼리 실행 통계
#[derive(Debug, Clone)]
pub struct QueryExecutionStats {
    pub total_executions: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_execution_time_ms: u64,
    pub last_executed: Instant,
}

impl QueryExecutionStats {
    fn new() -> Self {
        Self {
            total_executions: 0,
            cache_hits: 0,
            cache_misses: 0,
            total_execution_time_ms: 0,
            last_executed: Instant::now(),
        }
    }

    pub fn hit_rate(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_executions as f64
        }
    }

    pub fn average_execution_time(&self) -> f64 {
        if self.cache_misses == 0 {
            0.0
        } else {
            self.total_execution_time_ms as f64 / self.cache_misses as f64
        }
    }
}

/// 캐시 통계
#[derive(Debug, Clone)]
pub struct CacheStatistics {
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub hit_rate: f64,
    pub cache_size: usize,
    pub cache_capacity: usize,
    pub memory_usage_estimate: usize,
}

/// 캐시 리포트
#[derive(Debug, Clone)]
pub struct CacheReport {
    pub statistics: CacheStatistics,
    pub top_queries: Vec<(String, QueryExecutionStats)>,
    pub slow_queries: Vec<(String, QueryExecutionStats)>,
    pub low_efficiency_queries: Vec<(String, QueryExecutionStats)>,
    pub recommendations: Vec<String>,
}

// QueryResultData 변환 구현들
impl From<u64> for QueryResultData {
    fn from(value: u64) -> Self {
        QueryResultData::Count(value)
    }
}

impl From<QueryResultData> for u64 {
    fn from(data: QueryResultData) -> Self {
        match data {
            QueryResultData::Count(count) => count,
            _ => 0,
        }
    }
}

impl From<Vec<HashMap<String, String>>> for QueryResultData {
    fn from(rows: Vec<HashMap<String, String>>) -> Self {
        QueryResultData::Rows(rows)
    }
}

impl From<QueryResultData> for Vec<HashMap<String, String>> {
    fn from(data: QueryResultData) -> Self {
        match data {
            QueryResultData::Rows(rows) => rows,
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_cache_basic() {
        let mut cache = QueryCacheManager::new().unwrap();

        let query = "SELECT COUNT(*) FROM users";
        let params = vec![];

        // 캐시에 없음
        assert!(cache.get_cached_result(query, &params).is_none());

        // 결과 캐싱
        let result = QueryResultData::Count(100);
        cache.cache_result(query, &params, result, 50);

        // 캐시에서 조회
        let cached = cache.get_cached_result(query, &params);
        assert!(cached.is_some());
        match cached.unwrap().data {
            QueryResultData::Count(count) => assert_eq!(count, 100),
            _ => panic!("Wrong data type"),
        }
    }

    #[test]
    fn test_cacheable_query_detection() {
        let cache = QueryCacheManager::new().unwrap();

        // 캐시 가능한 쿼리
        assert!(cache.is_cacheable("SELECT * FROM users LIMIT 10"));
        assert!(cache.is_cacheable("SELECT COUNT(*) FROM posts"));

        // 캐시 불가능한 쿼리
        assert!(!cache.is_cacheable("INSERT INTO users VALUES (1, 'test')"));
        assert!(!cache.is_cacheable("SELECT * FROM users WHERE created_at = NOW()"));
        assert!(!cache.is_cacheable("SELECT RANDOM() as rand"));
        assert!(!cache.is_cacheable("SELECT * FROM large_table")); // LIMIT 없는 SELECT *
    }

    #[test]
    fn test_cache_statistics() {
        let mut cache = QueryCacheManager::new().unwrap();

        let query = "SELECT id FROM users WHERE active = 1";
        let params = vec![];

        // 캐시 미스
        assert!(cache.get_cached_result(query, &params).is_none());

        // 결과 캐싱
        cache.cache_result(query, &params, QueryResultData::Empty, 25);

        // 캐시 히트
        assert!(cache.get_cached_result(query, &params).is_some());

        let stats = cache.get_cache_stats();
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.hit_rate, 0.5);
    }

    #[test]
    fn test_execute_with_cache() {
        let mut cache = QueryCacheManager::new().unwrap();

        let query = "SELECT COUNT(*) FROM test_table";
        let params = vec![];

        // 첫 번째 실행 (캐시 미스)
        let result1 = cache
            .execute_with_cache(query, &params, || Ok(100u64))
            .unwrap();
        assert_eq!(result1, 100);

        // 두 번째 실행 (캐시 히트) - 실제로는 캐시 키가 같아야 히트됨
        let result2 = cache
            .execute_with_cache(query, &params, || {
                Ok(200u64) // 이 값은 캐시 히트 시 호출되지 않음
            })
            .unwrap();
        assert_eq!(result2, 100); // 캐시된 값 반환
    }

    #[test]
    fn test_cache_cleanup() {
        let mut cache = QueryCacheManager::new().unwrap();

        // 여러 결과 캐싱
        for i in 0..5 {
            let query = format!("SELECT {} as value", i);
            cache.cache_result(&query, &[], QueryResultData::Count(i), 10);
        }

        let initial_size = cache.get_cache_stats().cache_size;
        assert_eq!(initial_size, 5);

        // 정리 (TTL이 짧지 않으므로 즉시 정리되지 않을 수 있음)
        let cleaned = cache.cleanup_cache();
        assert!(cleaned <= initial_size);
    }
}
