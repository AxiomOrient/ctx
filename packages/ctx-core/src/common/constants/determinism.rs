//! 결정성(Determinism)과 유사-결정성(Similarity-stable) 관련 상수들
//!
//! 이 모듈은 동일 입력 → 동일 출력, 유사 입력 → 유사 출력을 보장하기 위한
//! 임계값, 가중치, 캐시 설정 등의 상수를 정의합니다.

/// 유사성 임계값: 이 값 이상이면 이전 결과를 재사용
/// - 0.85: 85% 이상 유사하면 재사용 (권장값)
/// - 높을수록 엄격한 매칭, 낮을수록 관대한 매칭
pub const SIMILARITY_THRESHOLD: f32 = 0.85;

/// 결정성 유사성 임계값 (호환성을 위한 별칭)
pub const DETERMINISM_SIMILARITY_THRESHOLD: f32 = SIMILARITY_THRESHOLD;

/// 유사성 계산 가중치들 (합계 = 1.0)
pub const FACET_SIMILARITY_WEIGHT: f32 = 0.4; // 패싯 매칭 중요도
pub const TEXT_SIMILARITY_WEIGHT: f32 = 0.4; // 텍스트 내용 유사도
pub const STRUCT_SIMILARITY_WEIGHT: f32 = 0.2; // 구조적 유사도

/// 텍스트 정규화 관련 상수들
pub const MAX_NORMALIZED_TEXT_LENGTH: usize = 100_000;
pub const TRIGRAM_SIMILARITY_THRESHOLD: f32 = 0.6;

/// 실행 ID 해시 길이 (16진수 문자 수)
pub const EXECUTION_ID_LENGTH: usize = 16;

/// 캐시 관련 설정
pub const DETERMINISM_CACHE_SIZE: usize = 1000; // 메모리 캐시 크기
pub const CACHE_EXPIRY_HOURS: i64 = 24; // 캐시 만료 시간 (시간)
pub const SIMILARITY_CACHE_MAX_ENTRIES: usize = 10_000; // 유사성 캐시 최대 항목수

/// 정규화 정책 상수들
pub const NORMALIZE_WHITESPACE: bool = true;
pub const NORMALIZE_MARKDOWN_HEADERS: bool = true;
pub const NORMALIZE_CODE_FENCES: bool = true;
pub const NORMALIZE_FILE_PATHS: bool = true;
pub const NORMALIZE_URLS: bool = true;

/// 입력 정규화 시 최대 처리 크기 (DoS 방지)
pub const MAX_INPUT_SIZE_BYTES: usize = 1_000_000; // 1MB

/// 유사성 계산 시 최대 비교 문서 수 (성능 최적화)
pub const MAX_SIMILARITY_COMPARISONS: usize = 100;

/// 버전 해시 길이
pub const VERSION_HASH_LENGTH: usize = 16;

/// 캐노니컬 입력 해시 길이  
pub const CANONICAL_INPUT_HASH_LENGTH: usize = 16;

/// 컨텍스트 해시 길이
pub const CONTEXT_HASH_LENGTH: usize = 16;

/// 텍스트 정규화용 정규표현식 패턴들
pub mod regex_patterns {
    /// 연속된 공백을 하나로 통합
    pub const WHITESPACE_NORMALIZE: &str = r"\s+";

    /// 마크다운 헤더를 표준화 (레벨 무시)
    pub const MARKDOWN_HEADERS: &str = r"#{1,6}\s+";

    /// 코드 펜스 언어 지정자 제거
    pub const CODE_FENCES: &str = r"```\w*\n";

    /// URL 끝의 슬래시 제거용
    pub const TRAILING_SLASHES: &str = r"https?://[^\s]+/+";

    /// 날짜 형식 감지 (간단한 패턴)
    pub const DATE_PATTERNS: &str = r"\b\d{4}-\d{2}-\d{2}\b";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_similarity_weights_sum() {
        let sum = FACET_SIMILARITY_WEIGHT + TEXT_SIMILARITY_WEIGHT + STRUCT_SIMILARITY_WEIGHT;
        assert!(
            (sum - 1.0).abs() < 0.01,
            "Similarity weights should sum to 1.0"
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_constants_validity() {
        // 유사성 임계값 범위 체크
        assert!(DETERMINISM_SIMILARITY_THRESHOLD >= 0.0 && DETERMINISM_SIMILARITY_THRESHOLD <= 1.0);
        assert!(TRIGRAM_SIMILARITY_THRESHOLD >= 0.0 && TRIGRAM_SIMILARITY_THRESHOLD <= 1.0);

        // 해시 길이 유효성 체크
        assert!(EXECUTION_ID_LENGTH > 0);
        assert!(VERSION_HASH_LENGTH > 0);
        assert!(CANONICAL_INPUT_HASH_LENGTH > 0);
        assert!(CONTEXT_HASH_LENGTH > 0);

        // 캐시 설정 유효성 체크
        assert!(DETERMINISM_CACHE_SIZE > 0);
        assert!(CACHE_EXPIRY_HOURS > 0);
        assert!(SIMILARITY_CACHE_MAX_ENTRIES > DETERMINISM_CACHE_SIZE);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_regex_patterns_not_empty() {
        assert!(!regex_patterns::WHITESPACE_NORMALIZE.is_empty());
        assert!(!regex_patterns::MARKDOWN_HEADERS.is_empty());
        assert!(!regex_patterns::CODE_FENCES.is_empty());
    }
}
