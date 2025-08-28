//! 기본값 상수들
//!
//! 시스템 전반에서 사용되는 기본값들을 중앙화

/// 기본 신뢰도 값
pub const DEFAULT_TRUST: f32 = 0.8;

/// 기본 신뢰도 값 (분류)
pub const DEFAULT_CLASSIFICATION_CONFIDENCE: f32 = 0.8;

/// 기본 로케일
pub const DEFAULT_LOCALE: &str = "ko";

/// 높은 신뢰도 임계값
pub const HIGH_CONFIDENCE_THRESHOLD: f32 = 0.8;

/// 중간 신뢰도 임계값  
pub const MEDIUM_CONFIDENCE_THRESHOLD: f32 = 0.6;

/// 기본 캐시 크기
pub const DEFAULT_CACHE_SIZE: usize = 1000;

/// 전체 시스템이 공유하는 정확 해법 전환 임계치
pub const EXACT_ALGORITHM_THRESHOLD: usize = 32;

/// 기본 토큰 예산
pub const DEFAULT_TOKEN_BUDGET: u32 = 2000;

/// 기본 스키마 버전
pub const DEFAULT_SCHEMA_VERSION: &str = "context.v1";

/// 요약 최대 길이
pub const SUMMARY_MAX_LENGTH: usize = 200;

/// 토큰 추정 계수 (단어 수 * 계수)
pub const TOKEN_ESTIMATION_MULTIPLIER: f32 = 1.3;

/// 데이터베이스 커넥션 풀 설정
pub const DEFAULT_MAX_DB_CONNECTIONS: u32 = 10;
pub const DEFAULT_MIN_IDLE_CONNECTIONS: u32 = 2;
pub const DEFAULT_CONNECTION_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 600; // 10분
pub const DEFAULT_MAX_LIFETIME_SECS: u64 = 1800; // 30분

/// 캐시 설정
pub const DEFAULT_SIMILARITY_CACHE_SIZE: usize = 10000;
pub const DEFAULT_SCORE_CACHE_SIZE: usize = 5000;
pub const DEFAULT_CACHE_TTL_SECS: u64 = 3600; // 1시간

/// 쿼리 최적화 설정
pub const MAX_SIMILARITY_RESULTS: usize = 100;
pub const MAX_CACHE_HISTORY_SIZE: usize = 1000;
