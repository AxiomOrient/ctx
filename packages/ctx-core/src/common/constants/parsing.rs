//! 파싱 관련 상수들
//!
//! 프론트매터, 섹션 추출, 마크다운 파싱 등에 사용되는 상수값들

/// 프론트매터 구분자
pub const FRONTMATTER_DELIMITER: &str = "---";

/// 헤더 접두사 (섹션 마커)
pub const HEADER_PREFIX: &str = "##";

/// 기본 토큰 추정 비율 (문자 수 대비)
pub const DEFAULT_TOKENS_PER_CHAR: f32 = 0.25;

/// 최대 섹션 깊이
pub const MAX_SECTION_DEPTH: usize = 6;

/// 기본 우선순위 값
pub const DEFAULT_PRIORITY: u8 = 50;

/// 최대 카테고리 깊이 (contexts/ 기준)
pub const MAX_CATEGORY_DEPTH: u32 = 3;
