//! 데이터베이스 쿼리 성능 최적화
//!
//! 쿼리 실행 시간 모니터링, 인덱스 최적화, 쿼리 플랜 분석 등을 제공합니다.

use crate::common::errors::ContextError;
use rusqlite::Connection;
use std::collections::HashMap;
use std::time::Instant;

pub mod index_optimizer;
pub mod query_analyzer;
pub mod query_cache;

/// 쿼리 성능 메트릭
#[derive(Debug, Clone)]
pub struct QueryMetrics {
    pub query_text: String,
    pub execution_time_ms: u64,
    pub rows_examined: Option<u64>,
    pub rows_returned: Option<u64>,
    pub index_used: Option<String>,
    pub execution_plan: Option<String>,
    pub cache_hit: bool,
}

impl QueryMetrics {
    pub fn new(query_text: String) -> Self {
        Self {
            query_text,
            execution_time_ms: 0,
            rows_examined: None,
            rows_returned: None,
            index_used: None,
            execution_plan: None,
            cache_hit: false,
        }
    }

    pub fn efficiency_ratio(&self) -> Option<f64> {
        match (self.rows_examined, self.rows_returned) {
            (Some(examined), Some(returned)) if examined > 0 => {
                Some(returned as f64 / examined as f64)
            }
            _ => None,
        }
    }

    pub fn is_slow_query(&self, threshold_ms: u64) -> bool {
        self.execution_time_ms > threshold_ms
    }
}

/// 쿼리 성능 모니터
pub struct QueryPerformanceMonitor {
    slow_query_threshold_ms: u64,
    metrics_history: Vec<QueryMetrics>,
    query_stats: HashMap<String, QueryStats>,
}

impl QueryPerformanceMonitor {
    pub fn new(slow_query_threshold_ms: u64) -> Self {
        Self {
            slow_query_threshold_ms,
            metrics_history: Vec::new(),
            query_stats: HashMap::new(),
        }
    }

    /// 쿼리 실행 모니터링
    pub fn monitor_query<F, T>(&mut self, query: &str, execution_fn: F) -> Result<T, ContextError>
    where
        F: FnOnce() -> Result<T, ContextError>,
    {
        let start_time = Instant::now();
        let result = execution_fn()?;
        let execution_time = start_time.elapsed().as_millis() as u64;

        let mut metrics = QueryMetrics::new(query.to_string());
        metrics.execution_time_ms = execution_time;

        // 통계 업데이트
        self.update_query_stats(query, execution_time);

        // 슬로우 쿼리 로깅
        if execution_time > self.slow_query_threshold_ms {
            self.log_slow_query(&metrics);
        }

        self.metrics_history.push(metrics);

        // 히스토리 크기 제한 (메모리 관리)
        if self.metrics_history.len() > crate::common::constants::defaults::MAX_CACHE_HISTORY_SIZE {
            self.metrics_history.drain(0..100);
        }

        Ok(result)
    }

    /// 쿼리 통계 업데이트
    fn update_query_stats(&mut self, query: &str, execution_time_ms: u64) {
        let stats = self
            .query_stats
            .entry(query.to_string())
            .or_insert(QueryStats::new());
        stats.execution_count += 1;
        stats.total_execution_time_ms += execution_time_ms;
        stats.last_execution_time_ms = execution_time_ms;

        if execution_time_ms > stats.max_execution_time_ms {
            stats.max_execution_time_ms = execution_time_ms;
        }

        if stats.min_execution_time_ms == 0 || execution_time_ms < stats.min_execution_time_ms {
            stats.min_execution_time_ms = execution_time_ms;
        }

        stats.average_execution_time_ms = stats.total_execution_time_ms / stats.execution_count;
    }

    fn log_slow_query(&self, metrics: &QueryMetrics) {
        eprintln!(
            "[SLOW QUERY] {}ms: {}",
            metrics.execution_time_ms,
            metrics.query_text.chars().take(100).collect::<String>()
        );
    }

    /// 슬로우 쿼리들 반환
    pub fn get_slow_queries(&self) -> Vec<&QueryMetrics> {
        self.metrics_history
            .iter()
            .filter(|m| m.is_slow_query(self.slow_query_threshold_ms))
            .collect()
    }

    /// 쿼리 통계 조회
    pub fn get_query_stats(&self, query: &str) -> Option<&QueryStats> {
        self.query_stats.get(query)
    }

    /// 모든 쿼리 통계
    pub fn get_all_stats(&self) -> &HashMap<String, QueryStats> {
        &self.query_stats
    }

    /// 성능 요약
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        let total_queries = self.metrics_history.len();
        let slow_queries = self.get_slow_queries().len();

        let total_time: u64 = self
            .metrics_history
            .iter()
            .map(|m| m.execution_time_ms)
            .sum();

        let average_time = if total_queries > 0 {
            total_time / total_queries as u64
        } else {
            0
        };

        PerformanceSummary {
            total_queries,
            slow_queries,
            slow_query_percentage: if total_queries > 0 {
                (slow_queries as f64 / total_queries as f64) * 100.0
            } else {
                0.0
            },
            average_execution_time_ms: average_time,
            total_execution_time_ms: total_time,
        }
    }

    /// 통계 초기화
    pub fn reset_stats(&mut self) {
        self.metrics_history.clear();
        self.query_stats.clear();
    }
}

/// 쿼리별 통계
#[derive(Debug, Clone)]
pub struct QueryStats {
    pub execution_count: u64,
    pub total_execution_time_ms: u64,
    pub average_execution_time_ms: u64,
    pub min_execution_time_ms: u64,
    pub max_execution_time_ms: u64,
    pub last_execution_time_ms: u64,
}

impl QueryStats {
    fn new() -> Self {
        Self {
            execution_count: 0,
            total_execution_time_ms: 0,
            average_execution_time_ms: 0,
            min_execution_time_ms: 0,
            max_execution_time_ms: 0,
            last_execution_time_ms: 0,
        }
    }

    pub fn performance_trend(&self) -> PerformanceTrend {
        let avg = self.average_execution_time_ms;
        let last = self.last_execution_time_ms;

        if last > avg + (avg / 4) {
            PerformanceTrend::Degrading
        } else if last < avg - (avg / 4) {
            PerformanceTrend::Improving
        } else {
            PerformanceTrend::Stable
        }
    }
}

/// 성능 추세
#[derive(Debug, Clone, PartialEq)]
pub enum PerformanceTrend {
    Improving,
    Stable,
    Degrading,
}

/// 성능 요약
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub total_queries: usize,
    pub slow_queries: usize,
    pub slow_query_percentage: f64,
    pub average_execution_time_ms: u64,
    pub total_execution_time_ms: u64,
}

/// SQLite 쿼리 플랜 분석기
pub struct QueryPlanAnalyzer;

impl QueryPlanAnalyzer {
    /// 쿼리 실행 계획 분석
    pub fn analyze_query_plan(conn: &Connection, query: &str) -> Result<QueryPlan, ContextError> {
        let explain_query = format!("EXPLAIN QUERY PLAN {}", query);
        let mut stmt = conn
            .prepare(&explain_query)
            .map_err(ContextError::DatabaseError)?;

        let mut plan_steps = Vec::new();
        let rows = stmt
            .query_map([], |row| {
                Ok(PlanStep {
                    id: row.get::<_, i32>(0)? as u32,
                    parent: row.get::<_, i32>(1).ok().map(|p| p as u32),
                    detail: row.get::<_, String>(3)?,
                })
            })
            .map_err(ContextError::DatabaseError)?;

        for row in rows {
            plan_steps.push(row.map_err(ContextError::DatabaseError)?);
        }

        let analysis = Self::analyze_plan_steps(&plan_steps);

        Ok(QueryPlan {
            query: query.to_string(),
            steps: plan_steps,
            analysis,
        })
    }

    fn analyze_plan_steps(steps: &[PlanStep]) -> PlanAnalysis {
        let has_full_scan = steps.iter().any(|step| step.detail.contains("SCAN"));
        let has_index_scan = steps.iter().any(|step| step.detail.contains("INDEX"));
        let has_sort = steps.iter().any(|step| step.detail.contains("ORDER BY"));
        let has_temp_btree = steps.iter().any(|step| step.detail.contains("TEMP B-TREE"));

        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        if has_full_scan {
            issues.push("Full table scan detected".to_string());
            suggestions.push("Consider adding appropriate indexes".to_string());
        }

        if has_temp_btree {
            issues.push("Temporary B-tree creation for sorting".to_string());
            suggestions.push("Consider adding index for ORDER BY clause".to_string());
        }

        let complexity = if steps.len() > 10 {
            QueryComplexity::High
        } else if steps.len() > 5 {
            QueryComplexity::Medium
        } else {
            QueryComplexity::Low
        };

        PlanAnalysis {
            has_full_scan,
            has_index_scan,
            has_sort,
            has_temp_btree,
            complexity,
            issues,
            suggestions,
        }
    }

    /// 쿼리 최적화 제안
    pub fn suggest_optimizations(plan: &QueryPlan) -> Vec<OptimizationSuggestion> {
        let mut suggestions = Vec::new();

        for issue in &plan.analysis.issues {
            if issue.contains("Full table scan") {
                suggestions.push(OptimizationSuggestion {
                    severity: OptimizationSeverity::High,
                    suggestion_type: OptimizationType::AddIndex,
                    description: "Add index to avoid full table scan".to_string(),
                    estimated_impact: ImpactLevel::High,
                });
            }

            if issue.contains("Temporary B-tree") {
                suggestions.push(OptimizationSuggestion {
                    severity: OptimizationSeverity::Medium,
                    suggestion_type: OptimizationType::AddIndex,
                    description: "Add composite index for ORDER BY optimization".to_string(),
                    estimated_impact: ImpactLevel::Medium,
                });
            }
        }

        if plan.analysis.complexity == QueryComplexity::High {
            suggestions.push(OptimizationSuggestion {
                severity: OptimizationSeverity::Medium,
                suggestion_type: OptimizationType::QueryRewrite,
                description: "Consider breaking down complex query into simpler parts".to_string(),
                estimated_impact: ImpactLevel::Medium,
            });
        }

        suggestions
    }
}

/// 쿼리 실행 계획
#[derive(Debug, Clone)]
pub struct QueryPlan {
    pub query: String,
    pub steps: Vec<PlanStep>,
    pub analysis: PlanAnalysis,
}

/// 실행 계획 단계
#[derive(Debug, Clone)]
pub struct PlanStep {
    pub id: u32,
    pub parent: Option<u32>,
    pub detail: String,
}

/// 실행 계획 분석 결과
#[derive(Debug, Clone)]
pub struct PlanAnalysis {
    pub has_full_scan: bool,
    pub has_index_scan: bool,
    pub has_sort: bool,
    pub has_temp_btree: bool,
    pub complexity: QueryComplexity,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
}

/// 쿼리 복잡도
#[derive(Debug, Clone, PartialEq)]
pub enum QueryComplexity {
    Low,
    Medium,
    High,
}

/// 최적화 제안
#[derive(Debug, Clone)]
pub struct OptimizationSuggestion {
    pub severity: OptimizationSeverity,
    pub suggestion_type: OptimizationType,
    pub description: String,
    pub estimated_impact: ImpactLevel,
}

/// 최적화 심각도
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// 최적화 타입
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationType {
    AddIndex,
    QueryRewrite,
    ParameterTuning,
    DataReorganization,
}

/// 영향 수준
#[derive(Debug, Clone, PartialEq)]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::time::Duration;

    #[test]
    fn test_query_performance_monitor() {
        let mut monitor = QueryPerformanceMonitor::new(100);

        // 빠른 쿼리 시뮬레이션
        monitor
            .monitor_query("SELECT 1", || {
                std::thread::sleep(Duration::from_millis(50));
                Ok(())
            })
            .expect("Query should succeed");
        assert_eq!(monitor.get_slow_queries().len(), 0);

        // 느린 쿼리 시뮬레이션
        monitor
            .monitor_query("SELECT * FROM large_table", || {
                std::thread::sleep(Duration::from_millis(150));
                Ok(())
            })
            .expect("Query should succeed");

        assert_eq!(monitor.get_slow_queries().len(), 1);
    }

    #[test]
    fn test_query_stats() {
        let mut monitor = QueryPerformanceMonitor::new(100);
        let query = "SELECT COUNT(*) FROM test";

        // 같은 쿼리 여러 번 실행
        for i in 0..5 {
            monitor
                .monitor_query(query, || {
                    std::thread::sleep(Duration::from_millis(10 + i * 5));
                    Ok(())
                })
                .expect("Query should succeed");
        }

        let stats = monitor.get_query_stats(query).expect("Stats should exist");
        assert_eq!(stats.execution_count, 5);
        assert!(stats.total_execution_time_ms > 0);
        assert!(stats.average_execution_time_ms > 0);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_query_plan_analyzer() {
        let conn = Connection::open_in_memory().expect("Failed to create connection");

        // 테스트 테이블 생성
        conn.execute(
            "CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT, value INTEGER)",
            [],
        )
        .expect("Failed to create table");

        let plan = QueryPlanAnalyzer::analyze_query_plan(
            &conn,
            "SELECT * FROM test_table WHERE name = 'test'",
        )
        .expect("Failed to analyze plan");

        assert!(!plan.steps.is_empty());
        assert_eq!(plan.query, "SELECT * FROM test_table WHERE name = 'test'");
    }

    #[test]
    fn test_performance_summary() {
        let mut monitor = QueryPerformanceMonitor::new(50);

        // 다양한 실행 시간으로 쿼리 실행
        let execution_times = vec![30, 60, 25, 80, 40];
        for time in execution_times {
            monitor
                .monitor_query("TEST QUERY", || {
                    std::thread::sleep(Duration::from_millis(time));
                    Ok(())
                })
                .expect("Query should succeed");
        }

        let summary = monitor.get_performance_summary();
        assert_eq!(summary.total_queries, 5);
        assert!(summary.slow_queries > 0); // 60ms와 80ms가 임계값 50ms 초과
        assert!(summary.slow_query_percentage > 0.0);
    }
}
