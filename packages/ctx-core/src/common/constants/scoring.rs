//! 스코어링 관련 상수들
//!
//! 문서 스코어링, MMR 선택, 신뢰도 계산 등에 사용되는 상수값들을 중앙화

/// 키워드 매칭 가중치 (기본값)
pub const DEFAULT_KEYWORD_WEIGHT: f32 = 0.8;

/// 신선도 가중치 (기본값)
pub const DEFAULT_FRESHNESS_WEIGHT: f32 = 0.8;

/// 신뢰도 가중치 (기본값)
pub const DEFAULT_TRUST_WEIGHT: f32 = 0.6;

/// 분류 신뢰도 가중치 (기본값)
pub const DEFAULT_CONFIDENCE_WEIGHT: f32 = 1.0;

/// 패싯 커버리지 가중치 (기본값)
pub const DEFAULT_FACET_COVERAGE_WEIGHT: f32 = 1.2;

/// 축 매칭 가중치 (기본값)
pub const DEFAULT_AXES_MATCH_WEIGHT: f32 = 0.5;

/// MMR 람다 파라미터 (기본값)
pub const DEFAULT_MMR_LAMBDA: f32 = 0.7;

/// 신선도 감쇠 상수 (일 단위)
pub const DEFAULT_FRESHNESS_TAU_DAYS: f32 = 365.0;

/// Knapsack 알고리즘 사용 임계값 (토큰 수)
pub const KNAPSACK_BUDGET_THRESHOLD: usize = 20_000;

/// 기본 신뢰도 임계값
pub const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 0.65;

// 분류기 증거 가중치 상수들
/// 키워드 매칭 증거 가중치
pub const CLASSIFIER_KEYWORD_EVIDENCE_WEIGHT: f32 = 0.5;

/// 링크 호스트 매칭 증거 가중치  
pub const CLASSIFIER_LINK_HOST_EVIDENCE_WEIGHT: f32 = 0.4;

/// 정규식 매칭 최소 가중치
pub const CLASSIFIER_REGEX_MIN_WEIGHT: f32 = 0.1;

// 분류기 신뢰도 계산 상수들
/// 최소 신뢰도
pub const CLASSIFIER_MIN_CONFIDENCE: f32 = 0.2;

/// 최대 신뢰도
pub const CLASSIFIER_MAX_CONFIDENCE: f32 = 0.99;

/// 신뢰도 포화 함수 상수 (k 값)
pub const CLASSIFIER_CONFIDENCE_SATURATION_K: f32 = 0.7;
