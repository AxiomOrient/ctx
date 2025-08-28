//! 쿼리 분석 및 최적화 도구
//!
//! SQL 쿼리의 성능 특성을 분석하고 최적화 방안을 제안합니다.

use super::{
    ImpactLevel, OptimizationSeverity, OptimizationSuggestion, OptimizationType, QueryPlan,
};
use crate::domain::errors::ContextError;
use regex::Regex;
use std::collections::HashMap;

/// 쿼리 분석기
pub struct QueryAnalyzer {
    // 쿼리 패턴 분석을 위한 정규식들
    select_regex: Regex,
    where_regex: Regex,
    join_regex: Regex,
    order_by_regex: Regex,
    group_by_regex: Regex,
    like_regex: Regex,
    subquery_regex: Regex,
}

impl QueryAnalyzer {
    pub fn new() -> Result<Self, ContextError> {
        Ok(Self {
            select_regex: Regex::new(r"(?i)SELECT\s+(.+?)\s+FROM")
                .map_err(|e| ContextError::Other(format!("Regex error: {}", e)))?,
            where_regex: Regex::new(r"(?i)WHERE\s+(.+?)(?:\s+(?:ORDER|GROUP|LIMIT)|$)")
                .map_err(|e| ContextError::Other(format!("Regex error: {}", e)))?,
            join_regex: Regex::new(r"(?i)(INNER|LEFT|RIGHT|FULL|CROSS)\s+JOIN")
                .map_err(|e| ContextError::Other(format!("Regex error: {}", e)))?,
            order_by_regex: Regex::new(r"(?i)ORDER\s+BY\s+(.+?)(?:\s+(?:LIMIT|$))")
                .map_err(|e| ContextError::Other(format!("Regex error: {}", e)))?,
            group_by_regex: Regex::new(r"(?i)GROUP\s+BY\s+(.+?)(?:\s+(?:HAVING|ORDER|LIMIT|$))")
                .map_err(|e| ContextError::Other(format!("Regex error: {}", e)))?,
            like_regex: Regex::new(r"(?i)\w+\s+LIKE\s+")
                .map_err(|e| ContextError::Other(format!("Regex error: {}", e)))?,
            subquery_regex: Regex::new(r"\([^()]*SELECT[^()]*\)")
                .map_err(|e| ContextError::Other(format!("Regex error: {}", e)))?,
        })
    }

    /// 쿼리 복잡도 분석
    pub fn analyze_query_complexity(&self, query: &str) -> QueryComplexityAnalysis {
        let mut score = 0;
        let mut factors = Vec::new();

        // SELECT 절 복잡도
        if let Some(captures) = self.select_regex.captures(query) {
            if let Some(select_clause) = captures.get(1) {
                let select_items = select_clause.as_str().split(',').count();
                if select_items > 10 {
                    score += 2;
                    factors.push("많은 컬럼 선택".to_string());
                } else if select_items > 5 {
                    score += 1;
                    factors.push("여러 컬럼 선택".to_string());
                }
            }
        }

        // WHERE 절 복잡도
        if let Some(captures) = self.where_regex.captures(query) {
            if let Some(where_clause) = captures.get(1) {
                let where_text = where_clause.as_str();
                let and_count = where_text.matches(" AND ").count();
                let or_count = where_text.matches(" OR ").count();

                if and_count + or_count > 5 {
                    score += 3;
                    factors.push("복잡한 WHERE 조건".to_string());
                } else if and_count + or_count > 2 {
                    score += 1;
                    factors.push("여러 WHERE 조건".to_string());
                }
            }
        }

        // JOIN 복잡도
        let join_count = self.join_regex.find_iter(query).count();
        if join_count > 3 {
            score += 3;
            factors.push("다중 JOIN".to_string());
        } else if join_count > 1 {
            score += 1;
            factors.push("JOIN 사용".to_string());
        }

        // ORDER BY 복잡도
        if let Some(captures) = self.order_by_regex.captures(query) {
            if let Some(order_clause) = captures.get(1) {
                let order_items = order_clause.as_str().split(',').count();
                if order_items > 3 {
                    score += 2;
                    factors.push("복잡한 정렬".to_string());
                } else if order_items > 1 {
                    score += 1;
                    factors.push("다중 정렬".to_string());
                }
            }
        }

        // GROUP BY 복잡도
        if self.group_by_regex.is_match(query) {
            score += 2;
            factors.push("GROUP BY 사용".to_string());
        }

        // LIKE 패턴 매칭
        let like_count = self.like_regex.find_iter(query).count();
        if like_count > 2 {
            score += 2;
            factors.push("다중 LIKE 패턴".to_string());
        } else if like_count > 0 {
            score += 1;
            factors.push("LIKE 패턴 사용".to_string());
        }

        // 서브쿼리
        let subquery_count = self.subquery_regex.find_iter(query).count();
        if subquery_count > 2 {
            score += 3;
            factors.push("다중 서브쿼리".to_string());
        } else if subquery_count > 0 {
            score += 2;
            factors.push("서브쿼리 사용".to_string());
        }

        let complexity_level = match score {
            0..=2 => QueryComplexityLevel::Low,
            3..=6 => QueryComplexityLevel::Medium,
            7..=12 => QueryComplexityLevel::High,
            _ => QueryComplexityLevel::VeryHigh,
        };

        QueryComplexityAnalysis {
            complexity_level,
            complexity_score: score,
            complexity_factors: factors,
        }
    }

    /// 쿼리 최적화 제안 생성
    pub fn suggest_optimizations(
        &self,
        query: &str,
        plan: Option<&QueryPlan>,
    ) -> Vec<OptimizationSuggestion> {
        let mut suggestions = Vec::new();

        // 쿼리 복잡도 기반 제안
        let complexity = self.analyze_query_complexity(query);
        if complexity.complexity_level == QueryComplexityLevel::VeryHigh {
            suggestions.push(OptimizationSuggestion {
                severity: OptimizationSeverity::High,
                suggestion_type: OptimizationType::QueryRewrite,
                description: "쿼리 복잡도가 매우 높습니다. 간단한 쿼리들로 분할을 고려하세요."
                    .to_string(),
                estimated_impact: ImpactLevel::High,
            });
        }

        // WHERE 절 최적화 제안
        if let Some(captures) = self.where_regex.captures(query) {
            if let Some(where_clause) = captures.get(1) {
                let where_text = where_clause.as_str().to_lowercase();

                // LIKE 패턴 최적화
                if where_text.contains("like '%") && where_text.contains("%'") {
                    suggestions.push(OptimizationSuggestion {
                        severity: OptimizationSeverity::Medium,
                        suggestion_type: OptimizationType::AddIndex,
                        description: "LIKE '%pattern%' 사용으로 인덱스를 활용할 수 없습니다. FTS(Full-Text Search)를 고려하세요.".to_string(),
                        estimated_impact: ImpactLevel::Medium,
                    });
                }

                // OR 조건 최적화
                if where_text.matches(" or ").count() > 2 {
                    suggestions.push(OptimizationSuggestion {
                        severity: OptimizationSeverity::Medium,
                        suggestion_type: OptimizationType::QueryRewrite,
                        description: "다중 OR 조건은 UNION으로 대체하거나 IN 절을 사용하는 것이 더 효율적일 수 있습니다.".to_string(),
                        estimated_impact: ImpactLevel::Medium,
                    });
                }
            }
        }

        // JOIN 최적화 제안
        let join_count = self.join_regex.find_iter(query).count();
        if join_count > 2 {
            suggestions.push(OptimizationSuggestion {
                severity: OptimizationSeverity::Medium,
                suggestion_type: OptimizationType::AddIndex,
                description: "다중 JOIN 사용 시 JOIN 키에 대한 인덱스가 필요합니다.".to_string(),
                estimated_impact: ImpactLevel::High,
            });
        }

        // ORDER BY 최적화 제안
        if let Some(captures) = self.order_by_regex.captures(query) {
            if let Some(order_clause) = captures.get(1) {
                let order_items = order_clause.as_str().split(',').count();
                if order_items > 1 {
                    suggestions.push(OptimizationSuggestion {
                        severity: OptimizationSeverity::Medium,
                        suggestion_type: OptimizationType::AddIndex,
                        description: "다중 컬럼 ORDER BY를 위한 복합 인덱스를 고려하세요."
                            .to_string(),
                        estimated_impact: ImpactLevel::Medium,
                    });
                }
            }
        }

        // 실행 계획 기반 제안
        if let Some(plan) = plan {
            if plan.analysis.has_full_scan {
                suggestions.push(OptimizationSuggestion {
                    severity: OptimizationSeverity::High,
                    suggestion_type: OptimizationType::AddIndex,
                    description:
                        "전체 테이블 스캔이 발생하고 있습니다. 적절한 인덱스를 추가하세요."
                            .to_string(),
                    estimated_impact: ImpactLevel::High,
                });
            }

            if plan.analysis.has_temp_btree {
                suggestions.push(OptimizationSuggestion {
                    severity: OptimizationSeverity::Medium,
                    suggestion_type: OptimizationType::AddIndex,
                    description: "Temporary B-tree creation detected. Consider adding an index for ORDER BY clause optimization.".to_string(),
                    estimated_impact: ImpactLevel::Medium,
                });
            }
        }

        suggestions
    }

    /// 인덱스 제안 생성
    pub fn suggest_indexes(
        &self,
        query: &str,
        table_columns: &HashMap<String, Vec<String>>,
    ) -> Vec<IndexSuggestion> {
        let mut suggestions = Vec::new();

        // WHERE 절에서 사용되는 컬럼들 추출
        if let Some(captures) = self.where_regex.captures(query) {
            if let Some(where_clause) = captures.get(1) {
                let where_text = where_clause.as_str();

                for (table, columns) in table_columns {
                    for column in columns {
                        if where_text.contains(column) {
                            suggestions.push(IndexSuggestion {
                                table_name: table.clone(),
                                columns: vec![column.clone()],
                                index_type: IndexType::BTree,
                                reason: "WHERE 절에서 사용됨".to_string(),
                                estimated_benefit: if where_text.contains(&format!("{} =", column))
                                {
                                    IndexBenefit::High
                                } else {
                                    IndexBenefit::Medium
                                },
                            });
                        }
                    }
                }
            }
        }

        // ORDER BY 절에서 사용되는 컬럼들
        if let Some(captures) = self.order_by_regex.captures(query) {
            if let Some(order_clause) = captures.get(1) {
                let order_columns: Vec<&str> = order_clause
                    .as_str()
                    .split(',')
                    .map(|s| s.split_whitespace().next().unwrap_or(""))
                    .collect();

                if order_columns.len() > 1 {
                    for (table, columns) in table_columns {
                        let matching_columns: Vec<String> = order_columns
                            .iter()
                            .filter_map(|col| {
                                if columns.contains(&col.to_string()) {
                                    Some(col.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect();

                        if !matching_columns.is_empty() {
                            suggestions.push(IndexSuggestion {
                                table_name: table.clone(),
                                columns: matching_columns,
                                index_type: IndexType::BTree,
                                reason: "ORDER BY 절 최적화".to_string(),
                                estimated_benefit: IndexBenefit::High,
                            });
                        }
                    }
                }
            }
        }

        // 중복 제거
        suggestions.sort_by(|a, b| {
            a.table_name
                .cmp(&b.table_name)
                .then(a.columns.cmp(&b.columns))
        });
        suggestions.dedup_by(|a, b| a.table_name == b.table_name && a.columns == b.columns);

        suggestions
    }

    /// 쿼리 재작성 제안
    pub fn suggest_query_rewrites(&self, query: &str) -> Vec<QueryRewriteSuggestion> {
        let mut suggestions = Vec::new();

        // EXISTS vs IN 최적화
        if query.to_lowercase().contains(" in (select ") {
            suggestions.push(QueryRewriteSuggestion {
                original_pattern: "IN (SELECT ...)".to_string(),
                suggested_pattern: "EXISTS (SELECT 1 FROM ...)".to_string(),
                reason: "EXISTS는 첫 번째 매치에서 중단되어 더 효율적일 수 있습니다.".to_string(),
                estimated_improvement: PerformanceImprovement::Medium,
            });
        }

        // COUNT(*) vs EXISTS 최적화
        if query.to_lowercase().contains("count(*)") && query.to_lowercase().contains("> 0") {
            suggestions.push(QueryRewriteSuggestion {
                original_pattern: "SELECT COUNT(*) ... > 0".to_string(),
                suggested_pattern: "SELECT EXISTS(SELECT 1 ...)".to_string(),
                reason: "존재 여부 확인에는 EXISTS가 더 효율적입니다.".to_string(),
                estimated_improvement: PerformanceImprovement::High,
            });
        }

        // DISTINCT 최적화
        if query.to_lowercase().contains("select distinct")
            && query.to_lowercase().contains("order by")
        {
            suggestions.push(QueryRewriteSuggestion {
                original_pattern: "SELECT DISTINCT ... ORDER BY".to_string(),
                suggested_pattern: "GROUP BY를 사용한 대안".to_string(),
                reason: "GROUP BY가 DISTINCT + ORDER BY보다 효율적일 수 있습니다.".to_string(),
                estimated_improvement: PerformanceImprovement::Medium,
            });
        }

        suggestions
    }
}

/// 쿼리 복잡도 분석 결과
#[derive(Debug, Clone)]
pub struct QueryComplexityAnalysis {
    pub complexity_level: QueryComplexityLevel,
    pub complexity_score: i32,
    pub complexity_factors: Vec<String>,
}

/// 쿼리 복잡도 수준
#[derive(Debug, Clone, PartialEq)]
pub enum QueryComplexityLevel {
    Low,
    Medium,
    High,
    VeryHigh,
}

/// 인덱스 제안
#[derive(Debug, Clone)]
pub struct IndexSuggestion {
    pub table_name: String,
    pub columns: Vec<String>,
    pub index_type: IndexType,
    pub reason: String,
    pub estimated_benefit: IndexBenefit,
}

/// 인덱스 타입
#[derive(Debug, Clone, PartialEq)]
pub enum IndexType {
    BTree,
    Hash,
    FullText,
}

/// 인덱스 효과
#[derive(Debug, Clone, PartialEq)]
pub enum IndexBenefit {
    Low,
    Medium,
    High,
}

/// 쿼리 재작성 제안
#[derive(Debug, Clone)]
pub struct QueryRewriteSuggestion {
    pub original_pattern: String,
    pub suggested_pattern: String,
    pub reason: String,
    pub estimated_improvement: PerformanceImprovement,
}

/// 성능 개선 정도
#[derive(Debug, Clone, PartialEq)]
pub enum PerformanceImprovement {
    Low,
    Medium,
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::expect_used)]
    fn test_query_complexity_analysis() {
        let analyzer = QueryAnalyzer::new().expect("Failed to create analyzer");

        // 간단한 쿼리
        let simple_query = "SELECT id FROM users WHERE id = 1";
        let analysis = analyzer.analyze_query_complexity(simple_query);
        assert_eq!(analysis.complexity_level, QueryComplexityLevel::Low);

        // 복잡한 쿼리 (더 복잡하게 만들어서 Medium/High 보장)
        let complex_query = r#"
            SELECT u.id, u.name, u.email, u.created_at, p.title, p.content, p.published_at, 
                   c.name as category, t.name as tag, COUNT(*) as comment_count
            FROM users u 
            INNER JOIN posts p ON u.id = p.user_id 
            LEFT JOIN categories c ON p.category_id = c.id 
            LEFT JOIN post_tags pt ON p.id = pt.post_id
            LEFT JOIN tags t ON pt.tag_id = t.id
            LEFT JOIN comments cm ON p.id = cm.post_id
            WHERE u.active = 1 AND p.published = 1 AND u.created_at > '2023-01-01'
                  AND (u.name LIKE '%admin%' OR u.email LIKE '%@company.com%')
                  AND p.title LIKE '%important%'
            GROUP BY u.id, p.id, c.id, t.id
            HAVING COUNT(*) > 5
            ORDER BY u.name, p.created_at DESC, comment_count DESC
        "#;
        let analysis = analyzer.analyze_query_complexity(complex_query);
        // 이 쿼리는 충분히 복잡해서 Medium 이상이어야 함
        assert!(
            analysis.complexity_level == QueryComplexityLevel::Medium
                || analysis.complexity_level == QueryComplexityLevel::High
                || analysis.complexity_level == QueryComplexityLevel::VeryHigh,
            "Expected Medium/High/VeryHigh but got {:?} with score {}",
            analysis.complexity_level,
            analysis.complexity_score
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_optimization_suggestions() {
        let analyzer = QueryAnalyzer::new().expect("Failed to create analyzer");

        let query = "SELECT * FROM users WHERE name LIKE '%john%' OR email LIKE '%john%'";
        let suggestions = analyzer.suggest_optimizations(query, None);

        // 디버깅을 위해 제안 내용 출력
        if suggestions.is_empty() {
            eprintln!("No suggestions generated for query: {}", query);
        } else {
            for suggestion in &suggestions {
                eprintln!("Suggestion: {}", suggestion.description);
            }
        }

        assert!(
            !suggestions.is_empty(),
            "Expected at least one optimization suggestion"
        );
        assert!(
            suggestions
                .iter()
                .any(|s| s.description.contains("LIKE") || s.description.contains("FTS")),
            "Expected LIKE-related suggestion but got: {:?}",
            suggestions
                .iter()
                .map(|s| &s.description)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_index_suggestions() {
        let analyzer = QueryAnalyzer::new().expect("Failed to create analyzer");

        let mut table_columns = HashMap::new();
        table_columns.insert(
            "users".to_string(),
            vec!["id".to_string(), "name".to_string(), "email".to_string()],
        );

        let query = "SELECT * FROM users WHERE name = 'john' ORDER BY email";
        let suggestions = analyzer.suggest_indexes(query, &table_columns);

        assert!(!suggestions.is_empty());
        assert!(
            suggestions
                .iter()
                .any(|s| s.columns.contains(&"name".to_string()))
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_query_rewrite_suggestions() {
        let analyzer = QueryAnalyzer::new().expect("Failed to create analyzer");

        let query =
            "SELECT * FROM users WHERE id IN (SELECT user_id FROM posts WHERE published = 1)";
        let suggestions = analyzer.suggest_query_rewrites(query);

        assert!(!suggestions.is_empty());
        assert!(
            suggestions
                .iter()
                .any(|s| s.suggested_pattern.contains("EXISTS"))
        );
    }
}
