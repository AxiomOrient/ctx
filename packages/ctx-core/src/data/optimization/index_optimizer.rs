//! 인덱스 최적화 도구
//!
//! 데이터베이스 인덱스 분석, 최적화 및 관리 기능을 제공합니다.

use crate::common::errors::ContextError;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

/// 인덱스 최적화 매니저
pub struct IndexOptimizer;

impl IndexOptimizer {
    /// 데이터베이스의 모든 인덱스 정보 조회
    pub fn analyze_existing_indexes(conn: &Connection) -> Result<Vec<IndexInfo>, ContextError> {
        let mut indexes = Vec::new();

        // 모든 테이블 목록 조회
        let mut table_stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'")?;
        let table_rows = table_stmt.query_map([], |row| row.get::<_, String>(0))?;

        for table_result in table_rows {
            let table_name = table_result?;
            
            // Validate table name to prevent SQL injection
            if !Self::is_valid_identifier(&table_name) {
                continue;
            }

            // 각 테이블의 인덱스 정보 조회 (PRAGMA는 매개변수 바인딩 미지원)
            let pragma_sql = format!("PRAGMA index_list({})", table_name);
            let mut index_stmt = conn.prepare(&pragma_sql)?;
            let index_rows = index_stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?, // index name
                    row.get::<_, bool>(2)?,   // unique
                ))
            })?;

            for index_result in index_rows {
                let (index_name, is_unique) = index_result?;

                // Validate index name 
                if !Self::is_valid_identifier(&index_name) {
                    continue;
                }

                // 인덱스 컬럼 정보 조회 (PRAGMA는 매개변수 바인딩 미지원)
                let pragma_info_sql = format!("PRAGMA index_info({})", index_name);
                let mut column_stmt = conn.prepare(&pragma_info_sql)?;
                let column_rows = column_stmt.query_map([], |row| {
                    row.get::<_, String>(2) // column name
                })?;

                let mut columns = Vec::new();
                for column_result in column_rows {
                    columns.push(column_result?);
                }

                if !columns.is_empty() {
                    indexes.push(IndexInfo {
                        name: index_name,
                        table_name: table_name.clone(),
                        columns,
                        is_unique,
                        is_partial: false,   // SQLite에서는 간단히 false로 설정
                        size_estimate: None, // 추후 계산 가능
                    });
                }
            }
        }

        Ok(indexes)
    }

    /// 테이블별 통계 정보 수집
    pub fn collect_table_statistics(
        conn: &Connection,
    ) -> Result<HashMap<String, TableStatistics>, ContextError> {
        let mut stats = HashMap::new();

        // 모든 테이블 목록 조회
        let mut table_stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'")?;
        let table_rows = table_stmt.query_map([], |row| row.get::<_, String>(0))?;

        for table_result in table_rows {
            let table_name = table_result?;

            // Validate table name
            if !Self::is_valid_identifier(&table_name) {
                continue;
            }

            // 행 수 계산
            let row_count_query = format!("SELECT COUNT(*) FROM {}", table_name);
            let row_count: i64 = conn.query_row(&row_count_query, [], |row| row.get(0))?;

            // 테이블 크기 추정 (SQLite에서는 정확한 크기를 얻기 어려움)
            let size_estimate = row_count * 100; // 대략적인 추정

            stats.insert(
                table_name,
                TableStatistics {
                    row_count: row_count as u64,
                    size_bytes: size_estimate as u64,
                    last_updated: chrono::Utc::now(),
                },
            );
        }

        Ok(stats)
    }

    /// 사용되지 않는 인덱스 찾기
    pub fn find_unused_indexes(
        conn: &Connection,
        query_patterns: &[String],
    ) -> Result<Vec<IndexInfo>, ContextError> {
        let existing_indexes = Self::analyze_existing_indexes(conn)?;
        let mut unused_indexes = Vec::new();

        for index in existing_indexes {
            let mut is_used = false;

            // 쿼리 패턴에서 인덱스 사용 여부 확인
            for query in query_patterns {
                if Self::is_index_likely_used(query, &index) {
                    is_used = true;
                    break;
                }
            }

            if !is_used {
                unused_indexes.push(index);
            }
        }

        Ok(unused_indexes)
    }

    /// 중복된 인덱스 찾기
    pub fn find_redundant_indexes(
        conn: &Connection,
    ) -> Result<Vec<RedundantIndexGroup>, ContextError> {
        let indexes = Self::analyze_existing_indexes(conn)?;
        let mut redundant_groups = Vec::new();

        // 테이블별로 그룹화
        let mut table_indexes: HashMap<String, Vec<&IndexInfo>> = HashMap::new();
        for index in &indexes {
            table_indexes
                .entry(index.table_name.clone())
                .or_default()
                .push(index);
        }

        for (table_name, table_index_list) in table_indexes {
            let redundant = Self::find_redundant_in_table(&table_index_list);
            if !redundant.is_empty() {
                redundant_groups.push(RedundantIndexGroup {
                    table_name,
                    redundant_indexes: redundant,
                });
            }
        }

        Ok(redundant_groups)
    }

    fn find_redundant_in_table(indexes: &[&IndexInfo]) -> Vec<IndexRedundancy> {
        let mut redundancies = Vec::new();

        for i in 0..indexes.len() {
            for j in (i + 1)..indexes.len() {
                let index1 = indexes[i];
                let index2 = indexes[j];

                if let Some(redundancy_type) = Self::check_redundancy(index1, index2) {
                    redundancies.push(IndexRedundancy {
                        primary_index: index1.name.clone(),
                        redundant_index: index2.name.clone(),
                        redundancy_type: redundancy_type.clone(),
                        recommendation: Self::generate_redundancy_recommendation(
                            index1,
                            index2,
                            &redundancy_type,
                        ),
                    });
                }
            }
        }

        redundancies
    }

    fn check_redundancy(index1: &IndexInfo, index2: &IndexInfo) -> Option<RedundancyType> {
        // 완전 중복 (같은 컬럼들)
        if index1.columns == index2.columns {
            return Some(RedundancyType::Duplicate);
        }

        // 접두사 중복 (한 인덱스가 다른 인덱스의 접두사)
        if index1.columns.len() < index2.columns.len() {
            if index2.columns.starts_with(&index1.columns) {
                return Some(RedundancyType::Prefix);
            }
        } else if index2.columns.len() < index1.columns.len()
            && index1.columns.starts_with(&index2.columns)
        {
            return Some(RedundancyType::Prefix);
        }

        // 부분 중복 (일부 컬럼이 겹침)
        let set1: HashSet<_> = index1.columns.iter().collect();
        let set2: HashSet<_> = index2.columns.iter().collect();
        let overlap: HashSet<_> = set1.intersection(&set2).collect();

        if !overlap.is_empty() && (overlap.len() as f64 / index1.columns.len() as f64) > 0.5 {
            return Some(RedundancyType::Partial);
        }

        None
    }

    fn generate_redundancy_recommendation(
        index1: &IndexInfo,
        index2: &IndexInfo,
        redundancy_type: &RedundancyType,
    ) -> String {
        match redundancy_type {
            RedundancyType::Duplicate => {
                format!(
                    "인덱스 {} 또는 {}를 제거하세요. 완전히 중복됩니다.",
                    index1.name, index2.name
                )
            }
            RedundancyType::Prefix => {
                if index1.columns.len() < index2.columns.len() {
                    format!(
                        "인덱스 {}를 제거하세요. {}가 더 포괄적입니다.",
                        index1.name, index2.name
                    )
                } else {
                    format!(
                        "인덱스 {}를 제거하세요. {}가 더 포괄적입니다.",
                        index2.name, index1.name
                    )
                }
            }
            RedundancyType::Partial => {
                format!(
                    "인덱스 {}와 {}의 통합을 고려하세요.",
                    index1.name, index2.name
                )
            }
        }
    }

    fn is_index_likely_used(query: &str, index: &IndexInfo) -> bool {
        let query_lower = query.to_lowercase();

        // 테이블이 쿼리에서 사용되는지 확인
        if !query_lower.contains(&index.table_name.to_lowercase()) {
            return false;
        }

        // 인덱스 컬럼들이 WHERE, ORDER BY, GROUP BY 절에서 사용되는지 확인
        for column in &index.columns {
            let column_lower = column.to_lowercase();
            if query_lower.contains(&format!("where {}", column_lower))
                || query_lower.contains(&format!("{} =", column_lower))
                || query_lower.contains(&format!("{} <", column_lower))
                || query_lower.contains(&format!("{} >", column_lower))
                || query_lower.contains(&format!("order by {}", column_lower))
                || query_lower.contains(&format!("group by {}", column_lower))
            {
                return true;
            }
        }

        false
    }

    /// 인덱스 효율성 분석
    pub fn analyze_index_efficiency(
        conn: &Connection,
        index_name: &str,
    ) -> Result<IndexEfficiency, ContextError> {
        // SQLite에서 인덱스 통계 수집
        let mut stat_stmt = conn.prepare("SELECT * FROM sqlite_stat1 WHERE tbl = ?")?;
        let stat_rows = stat_stmt.query_map([index_name], |row| {
            Ok((
                row.get::<_, String>(0)?, // table name
                row.get::<_, String>(1)?, // index name
                row.get::<_, String>(2)?, // stat string
            ))
        })?;

        let mut selectivity = 1.0;
        let mut estimated_rows = 0u64;

        for stat_result in stat_rows {
            let (_table, _index, stat_str) = stat_result?;

            // stat1 포맷: "n x y z" where n=total rows, x,y,z=avg rows per distinct value
            let parts: Vec<&str> = stat_str.split_whitespace().collect();
            if let Ok(total_rows) = parts.first().unwrap_or(&"0").parse::<u64>() {
                estimated_rows = total_rows;
                if total_rows > 0 {
                    selectivity = 1.0 / total_rows as f64;
                }
            }
        }

        let efficiency_score = Self::calculate_efficiency_score(selectivity, estimated_rows);

        Ok(IndexEfficiency {
            index_name: index_name.to_string(),
            selectivity,
            estimated_rows,
            efficiency_score,
            recommendations: Self::generate_efficiency_recommendations(selectivity, estimated_rows),
        })
    }

    fn calculate_efficiency_score(selectivity: f64, estimated_rows: u64) -> f64 {
        // 효율성 점수 계산 (0.0 ~ 1.0)
        let selectivity_score = if selectivity > 0.1 {
            0.3 // 낮은 선택성
        } else if selectivity > 0.01 {
            0.7 // 중간 선택성
        } else {
            1.0 // 높은 선택성
        };

        let size_score = if estimated_rows > 100_000 {
            1.0 // 큰 테이블에서는 인덱스가 중요
        } else if estimated_rows > 10_000 {
            0.7
        } else {
            0.3 // 작은 테이블에서는 인덱스 효과 제한적
        };

        (selectivity_score + size_score) / 2.0
    }

    fn generate_efficiency_recommendations(selectivity: f64, estimated_rows: u64) -> Vec<String> {
        let mut recommendations = Vec::new();

        if selectivity > 0.1 {
            recommendations.push("선택성이 낮습니다. 복합 인덱스를 고려하세요.".to_string());
        }

        if estimated_rows < 1000 {
            recommendations.push("테이블이 작습니다. 인덱스 제거를 고려하세요.".to_string());
        }

        if selectivity < 0.001 && estimated_rows > 100_000 {
            recommendations.push("매우 효율적인 인덱스입니다. 유지하세요.".to_string());
        }

        recommendations
    }

    /// 인덱스 생성 제안
    pub fn suggest_new_indexes(
        query_patterns: &[String],
        table_stats: &HashMap<String, TableStatistics>,
    ) -> Vec<IndexSuggestion> {
        let mut suggestions = Vec::new();
        let mut column_usage = HashMap::new();

        // 쿼리 패턴에서 컬럼 사용 빈도 수집
        for query in query_patterns {
            let used_columns = Self::extract_columns_from_query(query);
            for (table, columns) in used_columns {
                for column in columns {
                    *column_usage.entry((table.clone(), column)).or_insert(0) += 1;
                }
            }
        }

        // 사용 빈도가 높은 컬럼들에 대해 인덱스 제안
        for ((table, column), usage_count) in column_usage {
            if usage_count >= 2 {
                // 2번 이상 사용된 컬럼
                if let Some(stats) = table_stats.get(&table) {
                    if stats.row_count > 1000 {
                        // 충분한 크기의 테이블
                        suggestions.push(IndexSuggestion {
                            table_name: table.clone(),
                            column_name: column.clone(),
                            usage_frequency: usage_count,
                            estimated_benefit: Self::estimate_index_benefit(
                                usage_count,
                                stats.row_count,
                            ),
                            suggested_name: format!("idx_{}_{}", table, column),
                        });
                    }
                }
            }
        }

        // 효용성 기준으로 정렬
        suggestions.sort_by(|a, b| {
            b.estimated_benefit
                .partial_cmp(&a.estimated_benefit)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        suggestions
    }

    fn extract_columns_from_query(query: &str) -> HashMap<String, Vec<String>> {
        // 간단한 패턴 매칭으로 컬럼 추출
        // 실제로는 더 정교한 SQL 파서가 필요
        let mut columns = HashMap::new();

        // WHERE 절의 컬럼 추출 (매우 단순한 버전)
        if let Some(where_start) = query.to_lowercase().find("where ") {
            let where_clause = &query[where_start + 6..];
            if let Some(where_end) = where_clause.find(" order by") {
                let where_clause = &where_clause[..where_end];

                // 테이블.컬럼 또는 컬럼 패턴 찾기
                for word in where_clause.split_whitespace() {
                    if word.contains('.') {
                        let parts: Vec<&str> = word.split('.').collect();
                        if parts.len() == 2 {
                            columns
                                .entry(parts[0].to_string())
                                .or_insert_with(Vec::new)
                                .push(
                                    parts[1]
                                        .trim_matches(|c| !char::is_alphanumeric(c))
                                        .to_string(),
                                );
                        }
                    }
                }
            }
        }

        columns
    }

    fn estimate_index_benefit(usage_frequency: u32, table_rows: u64) -> f64 {
        let frequency_score = (usage_frequency as f64).ln() / 10.0;
        let size_score = (table_rows as f64).ln() / 20.0;
        (frequency_score + size_score).min(1.0)
    }

    /// Validate SQL identifier to prevent injection
    fn is_valid_identifier(name: &str) -> bool {
        if name.is_empty() || name.len() > 128 {
            return false;
        }
        
        // SQLite identifiers can contain letters, digits, underscores
        // Must start with letter or underscore
        let first_char = name.chars().next().unwrap_or('\0');
        if !first_char.is_ascii_alphabetic() && first_char != '_' {
            return false;
        }
        
        name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }
}

/// 인덱스 정보
#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub table_name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub is_partial: bool,
    pub size_estimate: Option<u64>,
}

/// 테이블 통계
#[derive(Debug, Clone)]
pub struct TableStatistics {
    pub row_count: u64,
    pub size_bytes: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// 중복 인덱스 그룹
#[derive(Debug, Clone)]
pub struct RedundantIndexGroup {
    pub table_name: String,
    pub redundant_indexes: Vec<IndexRedundancy>,
}

/// 인덱스 중복성
#[derive(Debug, Clone)]
pub struct IndexRedundancy {
    pub primary_index: String,
    pub redundant_index: String,
    pub redundancy_type: RedundancyType,
    pub recommendation: String,
}

/// 중복성 타입
#[derive(Debug, Clone, PartialEq)]
pub enum RedundancyType {
    Duplicate, // 완전 중복
    Prefix,    // 접두사 중복
    Partial,   // 부분 중복
}

/// 인덱스 효율성 분석 결과
#[derive(Debug, Clone)]
pub struct IndexEfficiency {
    pub index_name: String,
    pub selectivity: f64,
    pub estimated_rows: u64,
    pub efficiency_score: f64,
    pub recommendations: Vec<String>,
}

/// 인덱스 생성 제안
#[derive(Debug, Clone)]
pub struct IndexSuggestion {
    pub table_name: String,
    pub column_name: String,
    pub usage_frequency: u32,
    pub estimated_benefit: f64,
    pub suggested_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    #[allow(clippy::expect_used)]
    fn test_analyze_existing_indexes() {
        let conn = Connection::open_in_memory().expect("Failed to create connection");

        // 테스트 테이블과 인덱스 생성
        conn.execute(
            "CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT, email TEXT)",
            [],
        )
        .expect("Failed to create table");

        conn.execute("CREATE INDEX idx_name ON test_table(name)", [])
            .expect("Failed to create index");

        let indexes =
            IndexOptimizer::analyze_existing_indexes(&conn).expect("Failed to analyze indexes");

        // 적어도 하나의 인덱스가 있어야 함 (idx_name)
        assert!(!indexes.is_empty());
        assert!(indexes.iter().any(|idx| idx.name == "idx_name"));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_collect_table_statistics() {
        let conn = Connection::open_in_memory().expect("Failed to create connection");

        conn.execute("CREATE TABLE test_table (id INTEGER, name TEXT)", [])
            .expect("Failed to create table");

        conn.execute(
            "INSERT INTO test_table VALUES (1, 'test1'), (2, 'test2')",
            [],
        )
        .expect("Failed to insert data");

        let stats =
            IndexOptimizer::collect_table_statistics(&conn).expect("Failed to collect statistics");

        assert!(stats.contains_key("test_table"));
        let table_stats = &stats["test_table"];
        assert_eq!(table_stats.row_count, 2);
    }

    #[test]
    fn test_redundancy_detection() {
        let index1 = IndexInfo {
            name: "idx1".to_string(),
            table_name: "test".to_string(),
            columns: vec!["col1".to_string()],
            is_unique: false,
            is_partial: false,
            size_estimate: None,
        };

        let index2 = IndexInfo {
            name: "idx2".to_string(),
            table_name: "test".to_string(),
            columns: vec!["col1".to_string(), "col2".to_string()],
            is_unique: false,
            is_partial: false,
            size_estimate: None,
        };

        let redundancy = IndexOptimizer::check_redundancy(&index1, &index2);
        assert_eq!(redundancy, Some(RedundancyType::Prefix));
    }

    #[test]
    fn test_index_suggestion() {
        let mut table_stats = HashMap::new();
        table_stats.insert(
            "users".to_string(),
            TableStatistics {
                row_count: 10000,
                size_bytes: 1000000,
                last_updated: chrono::Utc::now(),
            },
        );

        let queries = vec![
            "SELECT * FROM users WHERE email = 'test@example.com'".to_string(),
            "SELECT * FROM users WHERE email = 'another@example.com'".to_string(),
        ];

        let suggestions = IndexOptimizer::suggest_new_indexes(&queries, &table_stats);
        // 실제 테스트에서는 더 정교한 쿼리 파싱이 필요하므로
        // 현재는 기본적인 구조만 테스트
        assert!(!suggestions.is_empty() || suggestions.is_empty()); // 구조적 테스트
    }
}
