use super::QuerySignals;
use crate::core::composer::types::DocumentCandidate;
use std::collections::HashMap;

/// 키워드 점수 계산 (TF-IDF 기반)
pub fn calculate_keyword_score(candidate: &DocumentCandidate, signals: &QuerySignals) -> f32 {
    let query_text = match &signals.query_text {
        Some(text) => text,
        None => return 0.0,
    };

    if query_text.trim().is_empty() {
        return 0.0;
    }

    // 쿼리 토큰화
    let query_terms = tokenize_text(query_text);
    if query_terms.is_empty() {
        return 0.0;
    }

    // 문서 내용 준비 (제목 + 내용)
    let document_text = if let Some(content) = &candidate.content {
        format!("{} {}", candidate.title, content)
    } else {
        candidate.title.clone()
    };

    let doc_terms = tokenize_text(&document_text);
    if doc_terms.is_empty() {
        return 0.0;
    }

    // TF-IDF 점수 계산
    calculate_tfidf_score(&query_terms, &doc_terms)
}

/// 텍스트를 토큰으로 분리하고 빈도 계산 (개선된 버전)
fn tokenize_text(text: &str) -> HashMap<String, f32> {
    let mut term_freq = HashMap::new();
    let mut total_terms = 0;

    // 개선된 토큰화: 더 정교한 분리와 정규화
    for word in text
        .split_whitespace()
        .flat_map(|w| {
            // 구두점과 특수문자로 분리
            w.split(&[
                ',', '.', '!', '?', ':', ';', '(', ')', '[', ']', '{', '}', '"', '\'', '`',
            ])
        })
        .map(|w| {
            // 앞뒤 공백 제거 후 소문자 변환
            w.trim().to_lowercase()
        })
        .filter(|w| {
            // 필터링 조건 개선
            !w.is_empty()
                && w.len() > 2  // 2글자 이하 제외
                && !is_stop_word(w)  // 불용어 제외
                && w.chars().any(|c| c.is_alphabetic()) // 최소 하나의 알파벳 포함
        })
    {
        *term_freq.entry(word).or_insert(0.0) += 1.0;
        total_terms += 1;
    }

    // 정규화: 빈도를 총 용어 수로 나누기
    if total_terms > 0 {
        for freq in term_freq.values_mut() {
            *freq /= total_terms as f32;
        }
    }

    term_freq
}

/// 간단한 불용어 체크 (한국어/영어 기본 불용어)
fn is_stop_word(word: &str) -> bool {
    matches!(
        word,
        // 영어 불용어
        "the" | "and" | "or" | "but" | "in" | "on" | "at" | "to" | "for" | "of" | "with" | "by" |
        "from" | "up" | "about" | "into" | "through" | "during" | "before" | "after" | "above" |
        "below" | "between" | "among" | "this" | "that" | "these" | "those" | "is" | "are" | "was" |
        "were" | "be" | "been" | "being" | "have" | "has" | "had" | "do" | "does" | "did" | "will" |
        "would" | "could" | "should" | "may" | "might" | "must" | "can" | "shall" |
        // 한국어 불용어 (로마자 표기)
        "gwa" | "eul" | "reul" | "ui" | "ga" | "i" | "eun" | "neun" | "ro" | "euro" |
        "eseo" | "buteo" | "kkaji" | "wa" | "hago"
    )
}

/// TF-IDF 점수 계산 (개선된 버전)
fn calculate_tfidf_score(
    query_terms: &HashMap<String, f32>,
    doc_terms: &HashMap<String, f32>,
) -> f32 {
    if query_terms.is_empty() || doc_terms.is_empty() {
        return 0.0;
    }

    let mut score = 0.0;
    let mut matched_terms = 0;
    let mut total_query_weight = 0.0;

    for (term, query_tf) in query_terms {
        total_query_weight += query_tf;

        if let Some(doc_tf) = doc_terms.get(term) {
            // TF: 문서 내 용어 빈도 (로그 정규화)
            let tf = (1.0 + doc_tf.ln()).max(0.0);

            // IDF: 개선된 역문서빈도 근사
            // 높은 빈도 용어에 대한 페널티 적용
            let idf = if *doc_tf > 0.0 {
                let raw_idf = (1.0 / doc_tf).ln() + 1.0;
                raw_idf.max(0.1) // 최소값 보장
            } else {
                1.0
            };

            // 쿼리 용어의 가중치와 TF-IDF 결합
            let term_score = query_tf * tf * idf;
            score += term_score;
            matched_terms += 1;
        }
    }

    if total_query_weight == 0.0 {
        return 0.0;
    }

    // 정규화: 매치 커버리지와 가중 점수 결합
    let coverage = matched_terms as f32 / query_terms.len() as f32;
    let normalized_score = score / total_query_weight;

    // 커버리지와 점수의 가중 평균 (조화 평균 대신 더 관대한 계산)
    let final_score = if coverage > 0.0 {
        // 커버리지 60% + 정규화 점수 40%의 가중 평균
        coverage * 0.6 + normalized_score * 0.4
    } else {
        0.0
    };

    final_score.min(1.0)
}

/// 제목에 대한 추가 가중치 적용
pub fn calculate_title_boost(candidate: &DocumentCandidate, signals: &QuerySignals) -> f32 {
    let query_text = match &signals.query_text {
        Some(text) => text,
        None => return 0.0,
    };

    let query_terms = tokenize_text(query_text);
    let title_terms = tokenize_text(&candidate.title);

    // 제목에서의 일치율 계산
    let mut matches = 0;
    for term in query_terms.keys() {
        if title_terms.contains_key(term) {
            matches += 1;
        }
    }

    if query_terms.is_empty() {
        0.0
    } else {
        (matches as f32 / query_terms.len() as f32) * 0.2 // 최대 20% 추가 부스트
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::composer::types::DocumentCandidate;
    use std::collections::HashMap;

    fn create_test_candidate(title: &str, content: Option<&str>) -> DocumentCandidate {
        DocumentCandidate {
            doc_id: "test".to_string(),
            sha: "sha".to_string(),
            title: title.to_string(),
            locale: "ko".to_string(),
            trust: 0.8,
            freshness: "2024-01-01".to_string(),
            confidence: 0.9,
            path: "test.md".to_string(),
            facets: HashMap::new(),
            tokens: 100,
            content: content.map(|s| s.to_string()),
        }
    }

    fn create_test_signals(query_text: &str) -> QuerySignals {
        QuerySignals {
            query_text: Some(query_text.to_string()),
            required_facets: vec![],
            required_axes: None,
        }
    }

    #[test]
    fn test_keyword_score_exact_match() {
        let candidate = create_test_candidate(
            "Rust Programming Guide",
            Some("This comprehensive guide covers Rust programming language fundamentals"),
        );
        let signals = create_test_signals("Rust programming guide");

        // 디버깅을 위해 토큰화 결과 확인
        let query_terms = tokenize_text("Rust programming guide");
        let doc_text = format!(
            "{} {}",
            candidate.title,
            candidate.content.as_deref().unwrap_or("")
        );
        let doc_terms = tokenize_text(&doc_text);

        println!("Query terms: {:?}", query_terms);
        println!("Doc terms: {:?}", doc_terms);

        let score = calculate_keyword_score(&candidate, &signals);
        assert!(
            score > 0.0,
            "Should have positive score for matching terms, got {}",
            score
        );
    }

    #[test]
    fn test_keyword_score_no_match() {
        let candidate = create_test_candidate("Python Tutorial", Some("Learn Python programming"));
        let signals = create_test_signals("Java development");

        let score = calculate_keyword_score(&candidate, &signals);
        assert_eq!(score, 0.0, "Should have zero score for non-matching terms");
    }

    #[test]
    fn test_keyword_score_partial_match() {
        let candidate = create_test_candidate(
            "Rust and Go Comparison",
            Some("Comparing Rust and Go languages"),
        );
        let signals = create_test_signals("Rust Python comparison");

        let score = calculate_keyword_score(&candidate, &signals);
        assert!(
            score > 0.0,
            "Should have positive score for partial matches"
        );
        assert!(
            score < 1.0,
            "Should not have perfect score for partial matches"
        );
    }

    #[test]
    fn test_empty_query() {
        let candidate = create_test_candidate("Any Title", Some("Any content"));
        let signals = create_test_signals("");

        let score = calculate_keyword_score(&candidate, &signals);
        assert_eq!(score, 0.0, "Should return 0 for empty query");
    }

    #[test]
    fn test_tokenize_text() {
        let terms = tokenize_text("Hello, world! Programming is a test.");
        assert!(terms.contains_key("hello"));
        assert!(terms.contains_key("world"));
        assert!(terms.contains_key("programming"));
        assert!(terms.contains_key("test"));

        // 2글자 이하와 불용어는 제외
        assert!(!terms.contains_key("is"));
        assert!(!terms.contains_key("a"));
    }

    #[test]
    fn test_title_boost() {
        let candidate = create_test_candidate("Rust Programming", Some("Guide content"));
        let signals = create_test_signals("Rust");

        let boost = calculate_title_boost(&candidate, &signals);
        assert!(boost > 0.0, "Should provide title boost for matching terms");
        assert!(boost <= 0.2, "Title boost should not exceed 20%");
    }
}
