//! 검증 관련 상수들
//!
//! 문서 검증, 스키마 검증, 유사도 계산 등에 사용되는 상수값들

/// 유사도 임계값 (중복 감지용)
pub const SIMILARITY_THRESHOLD: f32 = 0.7;

/// 자동 적용 신뢰도 임계값
pub const AUTO_APPLY_CONFIDENCE_THRESHOLD: f32 = 0.9;

/// 시맨틱 버전 파트 수
pub const SEMVER_PARTS_COUNT: usize = 3;

/// 최대 경고 수
pub const MAX_WARNINGS: usize = 100;

/// 최대 오류 수
pub const MAX_ERRORS: usize = 50;

/// 기본 검증 타임아웃 (초)
pub const DEFAULT_VALIDATION_TIMEOUT_SECS: u64 = 30;

/// 최대 우선순위 값
pub const MAX_PRIORITY: u32 = 100;
