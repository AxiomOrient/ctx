use super::QuerySignals;
use crate::core::composer::types::DocumentCandidate;
use std::collections::HashSet;

/// 패싯 커버리지 점수 계산 (개선된 가중 교집합 기반)
///
/// 공식: weighted_intersection_ratio with importance weighting
/// - 완전 매치: 1.0
/// - 부분 매치: 가중 매치 비율
/// - 매치 없음: 0.0
///
/// 개선사항:
/// - 네임스페이스별 중요도 가중치 적용
/// - 부분 매치에 대한 점진적 점수 부여
/// - 필수/선택 패싯 구분 지원
pub fn calculate_facet_coverage(candidate: &DocumentCandidate, signals: &QuerySignals) -> f32 {
    if signals.required_facets.is_empty() {
        return get_default_facet_coverage(); // 패싯 요구사항이 없으면 기본값
    }

    let mut total_weight = 0.0;
    let mut matched_weight = 0.0;

    for (namespace, required_values) in &signals.required_facets {
        // 네임스페이스별 중요도 가중치 계산
        let namespace_weight = calculate_namespace_importance(namespace);

        if let Some(candidate_values) = candidate.facets.get(namespace) {
            if required_values.is_empty() {
                // 네임스페이스만 있으면 됨 (값은 무관)
                total_weight += namespace_weight;
                if !candidate_values.is_empty() {
                    matched_weight += namespace_weight;
                }
            } else {
                // 특정 값들이 요구됨 - 가중 교집합 비율 계산
                let intersection_count =
                    calculate_value_intersection(required_values, candidate_values);
                let partial_score = intersection_count as f32 / required_values.len() as f32;

                total_weight += namespace_weight;
                matched_weight += namespace_weight * partial_score;
            }
        } else {
            // 문서에 해당 네임스페이스가 없음
            total_weight += namespace_weight;
            // matched_weight는 증가하지 않음 (0점)
        }
    }

    if total_weight == 0.0 {
        get_default_facet_coverage()
    } else {
        let coverage_ratio = matched_weight / total_weight;

        // 부분 매치에 대한 보너스 적용 (완전 매치가 아니어도 합리적 점수)
        let adjusted_ratio = if coverage_ratio > 0.0 && coverage_ratio < 1.0 {
            // 부분 매치 보너스: 최대 10% 추가
            let bonus = (coverage_ratio * 0.1).min(0.1);
            (coverage_ratio + bonus).min(1.0)
        } else {
            coverage_ratio
        };

        adjusted_ratio.clamp(0.0, 1.0)
    }
}

/// 네임스페이스별 중요도 가중치 계산
fn calculate_namespace_importance(namespace: &str) -> f32 {
    match namespace {
        // 핵심 분류 패싯들 (높은 가중치)
        "lang" | "language" => 1.2,
        "type" | "category" => 1.1,
        "topic" | "subject" => 1.1,

        // 구조적 패싯들 (중간 가중치)
        "artifact" | "format" => 1.0,
        "level" | "difficulty" => 0.9,

        // 메타데이터 패싯들 (낮은 가중치)
        "author" | "source" => 0.8,
        "version" | "status" => 0.7,

        // 기타 (기본 가중치)
        _ => 1.0,
    }
}

/// 두 값 집합 간의 교집합 개수 계산
fn calculate_value_intersection(required: &[String], candidate: &[String]) -> usize {
    let required_set: HashSet<&String> = required.iter().collect();
    let candidate_set: HashSet<&String> = candidate.iter().collect();

    required_set.intersection(&candidate_set).count()
}

/// 축 매칭 점수 계산 (개선된 다중 네임스페이스 지원)
pub fn calculate_axes_match(candidate: &DocumentCandidate, signals: &QuerySignals) -> f32 {
    let required_axes = match &signals.required_axes {
        Some(axes) => axes,
        None => return get_default_axes_match(), // 축 요구사항 없음
    };

    if required_axes.is_empty() {
        return get_default_axes_match();
    }

    // 다중 네임스페이스에서 축 정보 확인 (우선순위 순)
    let axis_namespaces = ["artifact", "type", "category", "format"];
    let mut best_match_score: f32 = 0.0;
    let mut found_any_axis = false;

    for namespace in &axis_namespaces {
        if let Some(candidate_values) = candidate.facets.get(*namespace) {
            if !candidate_values.is_empty() {
                found_any_axis = true;

                // 교집합 비율 계산
                let intersection_count =
                    calculate_value_intersection(required_axes, candidate_values);
                let match_ratio = intersection_count as f32 / required_axes.len() as f32;

                // 네임스페이스별 가중치 적용
                let weighted_score = match *namespace {
                    "artifact" => match_ratio * 1.0, // 기본 가중치
                    "type" => match_ratio * 0.9,     // 약간 낮은 가중치
                    "category" => match_ratio * 0.8, // 더 낮은 가중치
                    "format" => match_ratio * 0.7,   // 가장 낮은 가중치
                    _ => match_ratio * 0.5,
                };

                best_match_score = best_match_score.max(weighted_score);
            }
        }
    }

    if !found_any_axis {
        return 0.0; // 어떤 축 정보도 없음
    }

    // 부분 매치에 대한 보너스 적용
    let adjusted_score = if best_match_score > 0.0 && best_match_score < 1.0 {
        // 부분 매치 보너스: 최대 15% 추가 (축 매칭은 더 관대하게)
        let bonus = (best_match_score * 0.15).min(0.15);
        (best_match_score + bonus).min(1.0)
    } else {
        best_match_score
    };

    adjusted_score.clamp(0.0, 1.0)
}

/// 패싯 커버리지의 기본값
fn get_default_facet_coverage() -> f32 {
    0.6 // 중성적 값 - 패싯 정보가 없을 때 불이익을 받지 않도록
}

/// 축 매칭의 기본값
fn get_default_axes_match() -> f32 {
    0.4 // 중성적 값 - 축 정보가 없을 때 불이익을 받지 않도록
}

/// 패싯 매칭 품질 평가
pub fn assess_facet_quality(coverage_score: f32, axes_score: f32) -> FacetMatchQuality {
    let combined_score = (coverage_score + axes_score) / 2.0;

    match combined_score {
        s if s >= 0.9 => FacetMatchQuality::Perfect,
        s if s >= 0.7 => FacetMatchQuality::Good,
        s if s >= 0.5 => FacetMatchQuality::Partial,
        s if s >= 0.3 => FacetMatchQuality::Weak,
        _ => FacetMatchQuality::Poor,
    }
}

/// 패싯 매칭 품질 등급
#[derive(Debug, Clone, PartialEq)]
pub enum FacetMatchQuality {
    Perfect, // 완벽한 매치
    Good,    // 좋은 매치
    Partial, // 부분 매치
    Weak,    // 약한 매치
    Poor,    // 매치 부족
}

/// 패싯 커버리지 상세 분석
#[derive(Debug, Clone)]
pub struct FacetCoverageAnalysis {
    pub coverage_score: f32,
    pub axes_score: f32,
    pub matched_facets: Vec<String>,
    pub missing_facets: Vec<String>,
    pub quality: FacetMatchQuality,
}

/// 패싯 커버리지 상세 분석 수행
pub fn analyze_facet_coverage(
    candidate: &DocumentCandidate,
    signals: &QuerySignals,
) -> FacetCoverageAnalysis {
    let coverage_score = calculate_facet_coverage(candidate, signals);
    let axes_score = calculate_axes_match(candidate, signals);
    let quality = assess_facet_quality(coverage_score, axes_score);

    let mut matched_facets = Vec::new();
    let mut missing_facets = Vec::new();

    // 패싯별 매칭 분석
    for (namespace, required_values) in &signals.required_facets {
        if let Some(candidate_values) = candidate.facets.get(namespace) {
            if required_values.is_empty() {
                if !candidate_values.is_empty() {
                    matched_facets.push(namespace.clone());
                } else {
                    missing_facets.push(namespace.clone());
                }
            } else {
                let has_match = required_values.iter().any(|v| candidate_values.contains(v));
                if has_match {
                    if let Some(matched_value) = required_values
                        .iter()
                        .find(|v| candidate_values.contains(v))
                    {
                        matched_facets.push(format!("{}:{}", namespace, matched_value));
                    }
                } else {
                    missing_facets.push(format!("{}:{}", namespace, required_values.join("|")));
                }
            }
        } else if required_values.is_empty() {
            missing_facets.push(namespace.clone());
        } else {
            missing_facets.push(format!("{}:{}", namespace, required_values.join("|")));
        }
    }

    FacetCoverageAnalysis {
        coverage_score,
        axes_score,
        matched_facets,
        missing_facets,
        quality,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::composer::types::DocumentCandidate;
    use std::collections::HashMap;

    fn create_test_candidate(facets: HashMap<String, Vec<String>>) -> DocumentCandidate {
        DocumentCandidate {
            doc_id: "test".to_string(),
            sha: "sha".to_string(),
            title: "Test Document".to_string(),
            locale: "ko".to_string(),
            trust: 0.8,
            freshness: "2024-01-01".to_string(),
            confidence: 0.9,
            path: "test.md".to_string(),
            facets,
            tokens: 100,
            content: Some("Test content".to_string()),
        }
    }

    fn create_test_signals(required_facets: Vec<(String, Vec<String>)>) -> QuerySignals {
        QuerySignals {
            query_text: None,
            required_facets,
            required_axes: None,
        }
    }

    #[test]
    fn test_perfect_facet_coverage() {
        let mut facets = HashMap::new();
        facets.insert("lang".to_string(), vec!["rust".to_string()]);
        facets.insert("type".to_string(), vec!["guide".to_string()]);

        let candidate = create_test_candidate(facets);
        let signals = create_test_signals(vec![
            ("lang".to_string(), vec!["rust".to_string()]),
            ("type".to_string(), vec!["guide".to_string()]),
        ]);

        let score = calculate_facet_coverage(&candidate, &signals);
        assert_eq!(score, 1.0, "Perfect match should score 1.0");
    }

    #[test]
    fn test_partial_facet_coverage() {
        let mut facets = HashMap::new();
        facets.insert("lang".to_string(), vec!["rust".to_string()]);

        let candidate = create_test_candidate(facets);
        let signals = create_test_signals(vec![
            ("lang".to_string(), vec!["rust".to_string()]),
            ("type".to_string(), vec!["guide".to_string()]), // Missing
        ]);

        let score = calculate_facet_coverage(&candidate, &signals);
        // 개선된 알고리즘에서는 가중치와 보너스가 적용되어 0.5보다 약간 높을 수 있음
        assert!(
            (0.5..=0.6).contains(&score),
            "Half match ~0.5-0.6, got {}",
            score
        );
    }

    #[test]
    fn test_no_facet_coverage() {
        let facets = HashMap::new(); // Empty facets

        let candidate = create_test_candidate(facets);
        let signals = create_test_signals(vec![("lang".to_string(), vec!["rust".to_string()])]);

        let score = calculate_facet_coverage(&candidate, &signals);
        assert_eq!(score, 0.0, "No match should score 0.0");
    }

    #[test]
    fn test_no_required_facets() {
        let mut facets = HashMap::new();
        facets.insert("lang".to_string(), vec!["rust".to_string()]);

        let candidate = create_test_candidate(facets);
        let signals = create_test_signals(vec![]);

        let score = calculate_facet_coverage(&candidate, &signals);
        assert_eq!(
            score,
            get_default_facet_coverage(),
            "No requirements should return default"
        );
    }

    #[test]
    fn test_namespace_only_match() {
        let mut facets = HashMap::new();
        facets.insert("lang".to_string(), vec!["python".to_string()]);

        let candidate = create_test_candidate(facets);
        let signals = create_test_signals(vec![
            ("lang".to_string(), vec![]), // Any value in lang namespace
        ]);

        let score = calculate_facet_coverage(&candidate, &signals);
        assert_eq!(score, 1.0, "Namespace-only match should score 1.0");
    }

    #[test]
    fn test_axes_match_perfect() {
        let mut facets = HashMap::new();
        facets.insert(
            "artifact".to_string(),
            vec!["api".to_string(), "guide".to_string()],
        );

        let candidate = create_test_candidate(facets);
        let mut signals = create_test_signals(vec![]);
        signals.required_axes = Some(vec!["api".to_string()]);

        let score = calculate_axes_match(&candidate, &signals);
        assert_eq!(score, 1.0, "Perfect axes match should score 1.0");
    }

    #[test]
    fn test_axes_match_none() {
        let mut facets = HashMap::new();
        facets.insert("artifact".to_string(), vec!["guide".to_string()]);

        let candidate = create_test_candidate(facets);
        let mut signals = create_test_signals(vec![]);
        signals.required_axes = Some(vec!["api".to_string()]);

        let score = calculate_axes_match(&candidate, &signals);
        assert_eq!(score, 0.0, "No axes match should score 0.0");
    }

    #[test]
    fn test_value_intersection() {
        let required = vec!["rust".to_string(), "python".to_string()];
        let candidate = vec!["rust".to_string(), "go".to_string()];

        let intersection = calculate_value_intersection(&required, &candidate);
        assert_eq!(intersection, 1, "Should find 1 intersection (rust)");
    }

    #[test]
    fn test_facet_quality_assessment() {
        assert_eq!(assess_facet_quality(1.0, 1.0), FacetMatchQuality::Perfect);
        assert_eq!(assess_facet_quality(0.8, 0.7), FacetMatchQuality::Good);
        assert_eq!(assess_facet_quality(0.6, 0.4), FacetMatchQuality::Partial);
        assert_eq!(assess_facet_quality(0.3, 0.3), FacetMatchQuality::Weak);
        assert_eq!(assess_facet_quality(0.1, 0.1), FacetMatchQuality::Poor);
    }

    #[test]
    fn test_coverage_analysis() {
        let mut facets = HashMap::new();
        facets.insert("lang".to_string(), vec!["rust".to_string()]);
        facets.insert("artifact".to_string(), vec!["api".to_string()]);

        let candidate = create_test_candidate(facets);
        let mut signals = create_test_signals(vec![
            ("lang".to_string(), vec!["rust".to_string()]),
            ("type".to_string(), vec!["guide".to_string()]), // Missing
        ]);
        signals.required_axes = Some(vec!["api".to_string()]);

        let analysis = analyze_facet_coverage(&candidate, &signals);

        assert!(
            analysis.coverage_score >= 0.5 && analysis.coverage_score <= 0.6,
            "Coverage score should be around 0.5-0.6, got {}",
            analysis.coverage_score
        );
        assert_eq!(analysis.axes_score, 1.0);
        assert!(analysis.matched_facets.contains(&"lang:rust".to_string()));
        assert!(analysis.missing_facets.contains(&"type:guide".to_string()));
    }
}
