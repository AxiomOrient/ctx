use crate::domain::types::DocumentCandidate;
use chrono::{DateTime, Utc};

/// 신선도 점수 계산 (Exponential Decay)
///
/// 공식: exp(-days_old / tau_days)
/// - days_old: 문서가 마지막으로 업데이트된 이후 경과 일수
/// - tau_days: 감쇠 상수 (기본값: 365일)
///
/// 결과:
/// - 1.0: 오늘 업데이트된 문서
/// - ~0.37: tau_days 경과한 문서 (1/e)
/// - 0.0에 근접: 매우 오래된 문서
pub fn calculate_freshness_score(candidate: &DocumentCandidate, tau_days: f32) -> f32 {
    // tau_days는 양수여야 함
    if tau_days <= 0.0 {
        return get_default_freshness_score();
    }

    let days_old = match calculate_days_since_freshness(candidate) {
        Some(days) => {
            if days < 0 {
                // 미래 날짜인 경우 (시계 동기화 문제 등)
                0.0 // 최신으로 간주
            } else {
                days as f32
            }
        }
        None => return get_default_freshness_score(), // 날짜 정보 없음
    };

    // 개선된 exponential decay with minimum score
    let raw_score = (-days_old / tau_days).exp();

    // 최소 점수 보장 (매우 오래된 문서도 완전히 배제되지 않도록)
    let min_score = 0.05;
    let adjusted_score = raw_score.max(min_score);

    // 점진적 감쇠를 위한 스무딩 (30일 이내는 페널티 완화)
    let smoothed_score = if days_old <= 30.0 {
        let smoothing_factor = 1.0 - (days_old / 30.0) * 0.1; // 최대 10% 감소
        adjusted_score * smoothing_factor
    } else {
        adjusted_score
    };

    smoothed_score.clamp(min_score, 1.0)
}

/// 문서의 신선도 날짜로부터 경과 일수 계산 (개선된 버전)
fn calculate_days_since_freshness(candidate: &DocumentCandidate) -> Option<i64> {
    // 먼저 DocumentCandidate의 기존 메서드 사용 시도
    if let Some(days) = candidate.days_since_freshness() {
        return Some(days);
    }

    // 다양한 날짜 형식 지원
    let freshness_str = candidate.freshness.trim();

    // RFC3339 형식 시도
    if let Ok(freshness_date) = DateTime::parse_from_rfc3339(freshness_str) {
        let now = Utc::now();
        let duration = now.signed_duration_since(freshness_date.with_timezone(&Utc));
        return Some(duration.num_days());
    }

    // ISO 8601 형식 시도 (Z 없는 경우)
    if let Ok(naive_dt) = chrono::NaiveDateTime::parse_from_str(freshness_str, "%Y-%m-%dT%H:%M:%S")
    {
        let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
        let now = Utc::now();
        let duration = now.signed_duration_since(utc_dt);
        return Some(duration.num_days());
    }

    // 날짜만 있는 경우 (YYYY-MM-DD)
    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(freshness_str, "%Y-%m-%d") {
        let naive_dt = naive_date.and_hms_opt(0, 0, 0).unwrap_or_default();
        let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
        let now = Utc::now();
        let duration = now.signed_duration_since(utc_dt);
        return Some(duration.num_days());
    }

    // 다른 일반적인 형식들 시도
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d",
        "%d/%m/%Y",
        "%m/%d/%Y",
        "%Y.%m.%d",
    ];

    for format in &formats {
        if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(freshness_str, format) {
            let naive_dt = naive_date.and_hms_opt(0, 0, 0).unwrap_or_default();
            let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
            let now = Utc::now();
            let duration = now.signed_duration_since(utc_dt);
            return Some(duration.num_days());
        }
    }

    None
}

/// 신선도 정보가 없을 때의 기본 점수
fn get_default_freshness_score() -> f32 {
    0.5 // 중간값으로 설정하여 신선도를 알 수 없는 문서가 불이익받지 않도록 함
}

/// 신선도 점수의 품질 평가
pub fn assess_freshness_quality(score: f32) -> FreshnessQuality {
    match score {
        s if s >= 0.9 => FreshnessQuality::Excellent, // 최근 36일 이내 (tau=365기준)
        s if s >= 0.7 => FreshnessQuality::Good,      // 최근 130일 이내
        s if s >= 0.5 => FreshnessQuality::Fair,      // 최근 253일 이내
        s if s >= 0.3 => FreshnessQuality::Poor,      // 최근 438일 이내
        _ => FreshnessQuality::VeryPoor,              // 매우 오래됨
    }
}

/// 신선도 품질 등급
#[derive(Debug, Clone, PartialEq)]
pub enum FreshnessQuality {
    Excellent, // 매우 신선함
    Good,      // 신선함
    Fair,      // 보통
    Poor,      // 오래됨
    VeryPoor,  // 매우 오래됨
}

/// 신선도 점수에 따른 가중치 조정 제안
pub fn suggest_weight_adjustment(quality: &FreshnessQuality) -> f32 {
    match quality {
        FreshnessQuality::Excellent => 1.2, // 20% 가중치 증가
        FreshnessQuality::Good => 1.1,      // 10% 가중치 증가
        FreshnessQuality::Fair => 1.0,      // 기본 가중치
        FreshnessQuality::Poor => 0.9,      // 10% 가중치 감소
        FreshnessQuality::VeryPoor => 0.7,  // 30% 가중치 감소
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::DocumentCandidate;
    use chrono::{Duration, Utc};
    use std::collections::HashMap;

    fn create_test_candidate_with_freshness(days_old: i64) -> DocumentCandidate {
        let freshness_date = Utc::now() - Duration::days(days_old);

        DocumentCandidate {
            doc_id: "test".to_string(),
            sha: "sha".to_string(),
            title: "Test Document".to_string(),
            locale: "ko".to_string(),
            trust: 0.8,
            freshness: freshness_date.to_rfc3339(),
            confidence: 0.9,
            path: "test.md".to_string(),
            facets: HashMap::new(),
            tokens: 100,
            content: Some("Test content".to_string()),
        }
    }

    #[test]
    fn test_freshness_score_current_day() {
        let candidate = create_test_candidate_with_freshness(0); // 오늘
        let tau_days = 365.0;

        let score = calculate_freshness_score(&candidate, tau_days);
        assert!(
            (score - 1.0).abs() < 0.01,
            "Current day should have score ~1.0, got {}",
            score
        );
    }

    #[test]
    fn test_freshness_score_tau_days_old() {
        let tau_days = 365.0;
        let candidate = create_test_candidate_with_freshness(tau_days as i64); // tau_days 경과

        let score = calculate_freshness_score(&candidate, tau_days);
        let expected = (-1.0f32).exp(); // e^(-1) ≈ 0.368
        assert!(
            (score - expected).abs() < 0.01,
            "Should be ~0.368 after tau days, got {}",
            score
        );
    }

    #[test]
    fn test_freshness_score_very_old() {
        let candidate = create_test_candidate_with_freshness(1000); // 1000일 전
        let tau_days = 365.0;

        let score = calculate_freshness_score(&candidate, tau_days);
        assert!(
            score < 0.1,
            "Very old document should have low score, got {}",
            score
        );
        assert!(score >= 0.0, "Score should not be negative");
    }

    #[test]
    fn test_invalid_tau_days() {
        let candidate = create_test_candidate_with_freshness(10);

        let score = calculate_freshness_score(&candidate, 0.0); // Invalid tau
        assert_eq!(score, 0.5, "Invalid tau should return default score");

        let score = calculate_freshness_score(&candidate, -1.0); // Negative tau
        assert_eq!(score, 0.5, "Negative tau should return default score");
    }

    #[test]
    fn test_invalid_freshness_date() {
        let mut candidate = create_test_candidate_with_freshness(10);
        candidate.freshness = "invalid-date".to_string();

        let score = calculate_freshness_score(&candidate, 365.0);
        assert_eq!(score, 0.5, "Invalid date should return default score");
    }

    #[test]
    fn test_freshness_quality_assessment() {
        assert_eq!(assess_freshness_quality(0.95), FreshnessQuality::Excellent);
        assert_eq!(assess_freshness_quality(0.8), FreshnessQuality::Good);
        assert_eq!(assess_freshness_quality(0.6), FreshnessQuality::Fair);
        assert_eq!(assess_freshness_quality(0.4), FreshnessQuality::Poor);
        assert_eq!(assess_freshness_quality(0.1), FreshnessQuality::VeryPoor);
    }

    #[test]
    fn test_weight_adjustment_suggestions() {
        assert_eq!(suggest_weight_adjustment(&FreshnessQuality::Excellent), 1.2);
        assert_eq!(suggest_weight_adjustment(&FreshnessQuality::Good), 1.1);
        assert_eq!(suggest_weight_adjustment(&FreshnessQuality::Fair), 1.0);
        assert_eq!(suggest_weight_adjustment(&FreshnessQuality::Poor), 0.9);
        assert_eq!(suggest_weight_adjustment(&FreshnessQuality::VeryPoor), 0.7);
    }

    #[test]
    fn test_score_bounds() {
        let candidate = create_test_candidate_with_freshness(0);
        let tau_days = 365.0;

        let score = calculate_freshness_score(&candidate, tau_days);
        assert!(
            (0.0..=1.0).contains(&score),
            "Score in [0,1], got {}",
            score
        );
    }

    #[test]
    fn test_monotonic_decay() {
        let tau_days = 365.0;

        let score_0_days =
            calculate_freshness_score(&create_test_candidate_with_freshness(0), tau_days);
        let score_30_days =
            calculate_freshness_score(&create_test_candidate_with_freshness(30), tau_days);
        let score_365_days =
            calculate_freshness_score(&create_test_candidate_with_freshness(365), tau_days);

        assert!(
            score_0_days > score_30_days,
            "More recent should have higher score"
        );
        assert!(
            score_30_days > score_365_days,
            "Score should decay monotonically"
        );
    }
}
