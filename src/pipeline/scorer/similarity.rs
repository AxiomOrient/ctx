use crate::domain::constants::determinism::*;
use crate::domain::types::DocumentCandidate;
use std::collections::HashSet;

/// 두 문서 간 유사도 계산 (다중 기준 조합)
///
/// 공식: w_facet * FacetSim + w_text * TextSim + w_struct * StructSim
/// - FacetSim: 패싯 기반 Jaccard 유사도
/// - TextSim: 제목/내용 기반 Trigram 유사도  
/// - StructSim: 구조적 유사도 (토큰 수, 경로 등)
pub fn calculate_similarity(a: &DocumentCandidate, b: &DocumentCandidate) -> f32 {
    let facet_sim = calculate_facet_similarity(a, b);
    let text_sim = calculate_text_similarity(a, b);
    let struct_sim = calculate_structural_similarity(a, b);

    let similarity = FACET_SIMILARITY_WEIGHT * facet_sim
        + TEXT_SIMILARITY_WEIGHT * text_sim
        + STRUCT_SIMILARITY_WEIGHT * struct_sim;

    similarity.clamp(0.0, 1.0)
}

/// 패싯 기반 유사도 계산 (Jaccard Similarity)
pub fn calculate_facet_similarity(a: &DocumentCandidate, b: &DocumentCandidate) -> f32 {
    let set_a = a.facet_set();
    let set_b = b.facet_set();

    if set_a.is_empty() && set_b.is_empty() {
        return 1.0; // 둘 다 패싯이 없으면 동일하다고 간주
    }

    if set_a.is_empty() || set_b.is_empty() {
        return 0.0; // 하나만 패싯이 없으면 다르다고 간주
    }

    let intersection = set_a.intersection(&set_b).count() as f32;
    let union = set_a.union(&set_b).count() as f32;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// 텍스트 기반 유사도 계산 (제목 + 내용 Trigram)
pub fn calculate_text_similarity(a: &DocumentCandidate, b: &DocumentCandidate) -> f32 {
    // 제목 유사도 (가중치 더 높음)
    let title_sim = trigram_similarity(&a.title, &b.title);

    // 내용 유사도 (있는 경우)
    let content_sim = match (&a.content, &b.content) {
        (Some(content_a), Some(content_b)) => {
            // 내용이 너무 길면 처음 1000자만 사용
            let truncated_a = truncate_content(content_a, 1000);
            let truncated_b = truncate_content(content_b, 1000);
            trigram_similarity(&truncated_a, &truncated_b)
        }
        _ => title_sim, // 내용이 없으면 제목 유사도 재사용
    };

    // 제목과 내용의 가중 평균 (제목 70%, 내용 30%)
    title_sim * 0.7 + content_sim * 0.3
}

/// 구조적 유사도 계산 (개선된 가중 버전)
pub fn calculate_structural_similarity(a: &DocumentCandidate, b: &DocumentCandidate) -> f32 {
    let mut weighted_similarity = 0.0;
    let mut total_weight = 0.0;

    // 1. 토큰 수 유사도 (가중치: 0.3)
    let token_sim = calculate_token_similarity(a.tokens, b.tokens);
    let token_weight = 0.3;
    weighted_similarity += token_sim * token_weight;
    total_weight += token_weight;

    // 2. 파일 경로 유사도 (가중치: 0.25)
    let path_sim = calculate_path_similarity(&a.path, &b.path);
    let path_weight = 0.25;
    weighted_similarity += path_sim * path_weight;
    total_weight += path_weight;

    // 3. 로케일 일치도 (가중치: 0.15)
    let locale_sim = if a.locale == b.locale { 1.0 } else { 0.0 };
    let locale_weight = 0.15;
    weighted_similarity += locale_sim * locale_weight;
    total_weight += locale_weight;

    // 4. 신뢰도 유사도 (가중치: 0.1)
    let trust_sim = 1.0 - (a.trust - b.trust).abs();
    let trust_weight = 0.1;
    weighted_similarity += trust_sim * trust_weight;
    total_weight += trust_weight;

    // 5. 신선도 유사도 (가중치: 0.1) - 새로 추가
    let freshness_sim = calculate_freshness_similarity(a, b);
    let freshness_weight = 0.1;
    weighted_similarity += freshness_sim * freshness_weight;
    total_weight += freshness_weight;

    // 6. 문서 ID 패턴 유사도 (가중치: 0.1) - 새로 추가
    let id_sim = calculate_id_pattern_similarity(&a.doc_id, &b.doc_id);
    let id_weight = 0.1;
    weighted_similarity += id_sim * id_weight;
    total_weight += id_weight;

    if total_weight > 0.0 {
        weighted_similarity / total_weight
    } else {
        0.0
    }
}

/// 신선도 기반 유사도 계산
fn calculate_freshness_similarity(a: &DocumentCandidate, b: &DocumentCandidate) -> f32 {
    // 두 문서의 신선도 날짜 차이를 계산
    use chrono::{DateTime, Utc};

    let parse_date = |freshness: &str| -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(freshness)
            .map(|dt| dt.with_timezone(&Utc))
            .ok()
    };

    match (parse_date(&a.freshness), parse_date(&b.freshness)) {
        (Some(date_a), Some(date_b)) => {
            let diff_days = (date_a - date_b).num_days().abs() as f32;

            // 30일 이내면 높은 유사도, 그 이후는 점진적 감소
            if diff_days <= 30.0 {
                1.0 - (diff_days / 30.0) * 0.3 // 최대 30% 감소
            } else {
                let decay_factor = (-diff_days / 365.0).exp(); // 1년 기준 지수 감쇠
                (0.7 * decay_factor).max(0.1) // 최소 10% 유사도
            }
        }
        _ => 0.5, // 날짜 정보가 없으면 중립적 점수
    }
}

/// 문서 ID 패턴 유사도 계산
fn calculate_id_pattern_similarity(id_a: &str, id_b: &str) -> f32 {
    if id_a == id_b {
        return 1.0;
    }

    // ID에서 공통 패턴 추출 (예: 접두사, 숫자 패턴 등)
    let extract_pattern = |id: &str| -> Vec<String> {
        id.split(&['-', '_', '.', '/'])
            .filter(|part| !part.is_empty())
            .map(|s| s.to_string())
            .collect()
    };

    let parts_a = extract_pattern(id_a);
    let parts_b = extract_pattern(id_b);

    if parts_a.is_empty() || parts_b.is_empty() {
        return 0.0;
    }

    // 공통 부분의 비율 계산
    let common_parts = parts_a.iter().filter(|part| parts_b.contains(part)).count() as f32;

    let total_unique_parts = parts_a.len().max(parts_b.len()) as f32;

    if total_unique_parts > 0.0 {
        common_parts / total_unique_parts
    } else {
        0.0
    }
}

/// Trigram 유사도 계산 (개선된 가중 버전)
fn trigram_similarity(s1: &str, s2: &str) -> f32 {
    if s1 == s2 {
        return 1.0; // 완전 일치
    }

    if s1.is_empty() || s2.is_empty() {
        return 0.0; // 하나가 비어있으면 유사도 0
    }

    // 길이 차이가 너무 크면 유사도 페널티 적용
    let len_ratio = s1.len().min(s2.len()) as f32 / s1.len().max(s2.len()) as f32;
    let length_penalty = if len_ratio < 0.3 { 0.8 } else { 1.0 };

    let normalized_s1 = normalize_text_for_similarity(s1);
    let normalized_s2 = normalize_text_for_similarity(s2);

    let trigrams1 = extract_trigrams(&normalized_s1);
    let trigrams2 = extract_trigrams(&normalized_s2);

    if trigrams1.is_empty() || trigrams2.is_empty() {
        return 0.0;
    }

    // Jaccard 유사도 계산
    let intersection = trigrams1.intersection(&trigrams2).count() as f32;
    let union = trigrams1.union(&trigrams2).count() as f32;

    if union == 0.0 {
        return 0.0;
    }

    let jaccard_similarity = intersection / union;

    // 추가적인 유사도 메트릭: 공통 단어 비율
    let word_similarity = calculate_word_overlap_similarity(&normalized_s1, &normalized_s2);

    // 가중 조합: Trigram 70% + Word overlap 30%
    let combined_similarity = jaccard_similarity * 0.7 + word_similarity * 0.3;

    // 길이 페널티 적용
    (combined_similarity * length_penalty).clamp(0.0, 1.0)
}

/// 단어 겹침 기반 유사도 계산
fn calculate_word_overlap_similarity(s1: &str, s2: &str) -> f32 {
    let words1: HashSet<&str> = s1.split_whitespace().collect();
    let words2: HashSet<&str> = s2.split_whitespace().collect();

    if words1.is_empty() || words2.is_empty() {
        return 0.0;
    }

    let intersection = words1.intersection(&words2).count() as f32;
    let union = words1.union(&words2).count() as f32;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// 문자열에서 trigram 추출 (개선된 버전)
fn extract_trigrams(s: &str) -> HashSet<String> {
    let mut trigrams = HashSet::new();

    // 빈 문자열이면 빈 집합 반환
    if s.is_empty() {
        return trigrams;
    }

    // 패딩 추가하여 시작/끝 부분도 고려
    let padded = format!("  {}  ", s);
    let chars: Vec<char> = padded.chars().collect();

    for i in 0..chars.len().saturating_sub(2) {
        let trigram: String = chars[i..i + 3].iter().collect();
        trigrams.insert(trigram);
    }

    trigrams
}

/// 토큰 수 기반 유사도 계산
fn calculate_token_similarity(tokens_a: usize, tokens_b: usize) -> f32 {
    let max_tokens = tokens_a.max(tokens_b) as f32;
    let min_tokens = tokens_a.min(tokens_b) as f32;

    if max_tokens == 0.0 {
        1.0 // 둘 다 0이면 동일
    } else {
        min_tokens / max_tokens
    }
}

/// 파일 경로 기반 유사도 계산
fn calculate_path_similarity(path_a: &str, path_b: &str) -> f32 {
    if path_a == path_b {
        return 1.0;
    }

    // 경로를 구성 요소로 분할
    let components_a: Vec<&str> = path_a.split('/').collect();
    let components_b: Vec<&str> = path_b.split('/').collect();

    if components_a.is_empty() || components_b.is_empty() {
        return 0.0;
    }

    // 공통 접두사 길이 계산
    let common_prefix = components_a
        .iter()
        .zip(components_b.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let max_components = components_a.len().max(components_b.len());

    if max_components == 0 {
        1.0
    } else {
        common_prefix as f32 / max_components as f32
    }
}

/// 유사도 계산을 위한 텍스트 정규화
fn normalize_text_for_similarity(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// 긴 내용을 truncate
fn truncate_content(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        content.to_string()
    } else {
        format!("{}...", &content[..max_chars])
    }
}

/// 유사도 품질 평가
pub fn assess_similarity_quality(similarity: f32) -> SimilarityQuality {
    match similarity {
        s if s >= 0.9 => SimilarityQuality::VeryHigh,
        s if s >= 0.7 => SimilarityQuality::High,
        s if s >= 0.5 => SimilarityQuality::Medium,
        s if s >= 0.3 => SimilarityQuality::Low,
        _ => SimilarityQuality::VeryLow,
    }
}

/// 유사도 품질 등급
#[derive(Debug, Clone, PartialEq)]
pub enum SimilarityQuality {
    VeryHigh, // 매우 유사함 (>= 0.9)
    High,     // 유사함 (>= 0.7)
    Medium,   // 보통 (>= 0.5)
    Low,      // 낮음 (>= 0.3)
    VeryLow,  // 매우 낮음 (< 0.3)
}

/// 유사도 상세 분석
#[derive(Debug, Clone)]
pub struct SimilarityAnalysis {
    pub total_similarity: f32,
    pub facet_similarity: f32,
    pub text_similarity: f32,
    pub structural_similarity: f32,
    pub quality: SimilarityQuality,
}

/// 두 문서의 유사도 상세 분석
pub fn analyze_similarity(a: &DocumentCandidate, b: &DocumentCandidate) -> SimilarityAnalysis {
    let facet_similarity = calculate_facet_similarity(a, b);
    let text_similarity = calculate_text_similarity(a, b);
    let structural_similarity = calculate_structural_similarity(a, b);
    let total_similarity = calculate_similarity(a, b);
    let quality = assess_similarity_quality(total_similarity);

    SimilarityAnalysis {
        total_similarity,
        facet_similarity,
        text_similarity,
        structural_similarity,
        quality,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_candidate(
        title: &str,
        content: Option<&str>,
        facets: HashMap<String, Vec<String>>,
        tokens: usize,
        path: &str,
    ) -> DocumentCandidate {
        DocumentCandidate {
            doc_id: "test".to_string(),
            sha: "sha".to_string(),
            title: title.to_string(),
            locale: "ko".to_string(),
            trust: 0.8,
            freshness: "2024-01-01".to_string(),
            confidence: 0.9,
            path: path.to_string(),
            facets,
            tokens,
            content: content.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_identical_documents() {
        let mut facets = HashMap::new();
        facets.insert("lang".to_string(), vec!["rust".to_string()]);

        let doc_a = create_test_candidate(
            "Rust Guide",
            Some("Rust programming"),
            facets.clone(),
            100,
            "rust/guide.md",
        );
        let doc_b = create_test_candidate(
            "Rust Guide",
            Some("Rust programming"),
            facets,
            100,
            "rust/guide.md",
        );

        let similarity = calculate_similarity(&doc_a, &doc_b);
        assert!(
            similarity > 0.9,
            "Identical documents should have very high similarity, got {}",
            similarity
        );
    }

    #[test]
    fn test_completely_different_documents() {
        let mut facets_a = HashMap::new();
        facets_a.insert("lang".to_string(), vec!["rust".to_string()]);

        let mut facets_b = HashMap::new();
        facets_b.insert("lang".to_string(), vec!["python".to_string()]);

        let doc_a = create_test_candidate(
            "Rust Guide",
            Some("Rust programming"),
            facets_a,
            100,
            "rust/guide.md",
        );
        let doc_b = create_test_candidate(
            "Python Tutorial",
            Some("Python scripting"),
            facets_b,
            200,
            "python/tutorial.md",
        );

        let similarity = calculate_similarity(&doc_a, &doc_b);
        assert!(
            similarity < 0.5,
            "Completely different documents should have low similarity, got {}",
            similarity
        );
    }

    #[test]
    fn test_facet_similarity() {
        let mut facets_common = HashMap::new();
        facets_common.insert("lang".to_string(), vec!["rust".to_string()]);
        facets_common.insert("type".to_string(), vec!["guide".to_string()]);

        let mut facets_partial = HashMap::new();
        facets_partial.insert("lang".to_string(), vec!["rust".to_string()]);
        facets_partial.insert("type".to_string(), vec!["tutorial".to_string()]);

        let doc_a = create_test_candidate("Test A", None, facets_common, 100, "test_a.md");
        let doc_b = create_test_candidate("Test B", None, facets_partial, 100, "test_b.md");

        let facet_sim = calculate_facet_similarity(&doc_a, &doc_b);
        assert!(
            facet_sim > 0.0 && facet_sim < 1.0,
            "Should have partial facet similarity, got {}",
            facet_sim
        );
    }

    #[test]
    fn test_trigram_similarity() {
        let sim1 = trigram_similarity("Rust Programming Guide", "Rust Programming Tutorial");
        assert!(
            sim1 > 0.4,
            "Similar titles should have reasonable trigram similarity, got {}",
            sim1
        );

        let sim2 = trigram_similarity("Rust Guide", "Python Tutorial");
        assert!(
            sim2 < 0.3,
            "Different titles should have low trigram similarity, got {}",
            sim2
        );

        let sim3 = trigram_similarity("Test", "Test");
        assert_eq!(sim3, 1.0, "Identical strings should have similarity 1.0");

        let sim4 = trigram_similarity("", "Test");
        assert_eq!(sim4, 0.0, "Empty string should have similarity 0.0");
    }

    #[test]
    fn test_token_similarity() {
        assert_eq!(
            calculate_token_similarity(100, 100),
            1.0,
            "Same tokens should be 1.0"
        );
        assert_eq!(
            calculate_token_similarity(100, 200),
            0.5,
            "1:2 ratio should be 0.5"
        );
        assert_eq!(
            calculate_token_similarity(0, 0),
            1.0,
            "Both zero should be 1.0"
        );
        assert_eq!(
            calculate_token_similarity(0, 100),
            0.0,
            "Zero to non-zero should be 0.0"
        );
    }

    #[test]
    fn test_path_similarity() {
        assert_eq!(
            calculate_path_similarity("a/b/c.md", "a/b/c.md"),
            1.0,
            "Same paths should be 1.0"
        );
        assert!(
            calculate_path_similarity("a/b/c.md", "a/b/d.md") > 0.5,
            "Similar paths should have high similarity"
        );
        assert!(
            calculate_path_similarity("a/b/c.md", "x/y/z.md") < 0.5,
            "Different paths should have low similarity"
        );
    }

    #[test]
    fn test_extract_trigrams() {
        let trigrams = extract_trigrams("abc");
        assert!(!trigrams.is_empty(), "Should extract trigrams from 'abc'");

        let trigrams_empty = extract_trigrams("");
        assert!(
            trigrams_empty.is_empty(),
            "Empty string should produce no trigrams"
        );
    }

    #[test]
    fn test_similarity_quality_assessment() {
        assert_eq!(assess_similarity_quality(0.95), SimilarityQuality::VeryHigh);
        assert_eq!(assess_similarity_quality(0.8), SimilarityQuality::High);
        assert_eq!(assess_similarity_quality(0.6), SimilarityQuality::Medium);
        assert_eq!(assess_similarity_quality(0.4), SimilarityQuality::Low);
        assert_eq!(assess_similarity_quality(0.1), SimilarityQuality::VeryLow);
    }

    #[test]
    fn test_similarity_bounds() {
        let mut facets = HashMap::new();
        facets.insert("test".to_string(), vec!["value".to_string()]);

        let doc_a = create_test_candidate("Test A", None, facets.clone(), 100, "test.md");
        let doc_b = create_test_candidate("Test B", None, facets, 100, "test.md");

        let similarity = calculate_similarity(&doc_a, &doc_b);
        assert!(
            (0.0..=1.0).contains(&similarity),
            "Similarity in [0,1], got {}",
            similarity
        );
    }

    #[test]
    fn test_analyze_similarity() {
        let mut facets = HashMap::new();
        facets.insert("lang".to_string(), vec!["rust".to_string()]);

        let doc_a = create_test_candidate(
            "Rust Guide",
            Some("Programming"),
            facets.clone(),
            100,
            "rust/guide.md",
        );
        let doc_b = create_test_candidate(
            "Rust Tutorial",
            Some("Coding"),
            facets,
            120,
            "rust/tutorial.md",
        );

        let analysis = analyze_similarity(&doc_a, &doc_b);

        assert!(analysis.total_similarity >= 0.0 && analysis.total_similarity <= 1.0);
        assert!(analysis.facet_similarity >= 0.0 && analysis.facet_similarity <= 1.0);
        assert!(analysis.text_similarity >= 0.0 && analysis.text_similarity <= 1.0);
        assert!(analysis.structural_similarity >= 0.0 && analysis.structural_similarity <= 1.0);
    }
}
