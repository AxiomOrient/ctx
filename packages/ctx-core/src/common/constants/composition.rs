//! 프롬프트 조합(Composition) 관련 상수들
//!
//! 이 모듈은 MMR 알고리즘, 토큰 예산 관리, 선택 전략 등
//! 문서 조합 과정에서 사용되는 모든 상수를 정의합니다.

/// MMR(Maximal Marginal Relevance) 알고리즘의 다양성 가중치
/// - 0.0: 완전한 관련성 우선 (다양성 무시)
/// - 1.0: 완전한 다양성 우선 (관련성 무시)  
/// - 0.3: 관련성 70%, 다양성 30% (권장값)
pub const DEFAULT_MMR_LAMBDA: f32 = 0.3;

/// MMR 람다 (호환성을 위한 별칭)
pub const COMPOSITION_MMR_LAMBDA: f32 = DEFAULT_MMR_LAMBDA;

/// 기본 토큰 예산 (사용자 미지정시) - usize 타입
pub const DEFAULT_TOKEN_BUDGET: usize = 2000;

/// 기본 토큰 예산 (호환성을 위한 별칭)
pub const COMPOSITION_TOKEN_BUDGET: usize = DEFAULT_TOKEN_BUDGET;

/// 기본 토큰 예약량 (메타데이터, 헤더용)
pub const DEFAULT_TOKEN_RESERVE: usize = 200;

/// 점수 계산 시 고려할 최대 후보 문서 수 (성능 최적화)
pub const MAX_CANDIDATES_TO_SCORE: usize = 1000;

/// 그리디 선택으로 전환하는 문서 수 임계값
/// 이 수 이하면 단순 그리디, 이상이면 MMR 알고리즘 적용
pub const GREEDY_SELECTION_THRESHOLD: usize = 10;

/// 문서 병합 시 섹션 간 구분자
pub const SECTION_SEPARATOR: &str = "\n\n---\n\n";

/// 최소 토큰 예산 (너무 작으면 의미있는 조합 불가)
pub const MIN_TOKEN_BUDGET: usize = 100;

/// 최대 토큰 예산 (메모리 보호)
pub const MAX_TOKEN_BUDGET: usize = 100_000;

/// 단일 문서 최대 토큰 수 (너무 큰 문서 제한)
pub const MAX_DOCUMENT_TOKENS: usize = 50_000;

/// 병합된 문서의 최대 섹션 수
pub const MAX_MERGED_SECTIONS: usize = 50;

/// 조합 결과 신뢰도 가중치
pub const COMPOSITION_CONFIDENCE_WEIGHTS: [f32; 3] = [
    0.4, // 문서 품질 (평균 신뢰도)
    0.3, // 선택 품질 (MMR 점수)
    0.3, // 커버리지 (요구사항 충족도)
];

/// 문서 선택 시 다양성 측정을 위한 최소 유사도 임계값
pub const DIVERSITY_SIMILARITY_THRESHOLD: f32 = 0.7;

/// 조합 메타데이터 생성 시 포함할 최대 소스 문서 수
pub const MAX_SOURCE_DOCS_IN_METADATA: usize = 20;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_constants_validity() {
        // MMR lambda 범위 체크
        assert!(COMPOSITION_MMR_LAMBDA >= 0.0 && COMPOSITION_MMR_LAMBDA <= 1.0);

        // 토큰 예산 일관성 체크
        assert!(DEFAULT_TOKEN_RESERVE < COMPOSITION_TOKEN_BUDGET);
        assert!(MIN_TOKEN_BUDGET < COMPOSITION_TOKEN_BUDGET);
        assert!(COMPOSITION_TOKEN_BUDGET < MAX_TOKEN_BUDGET);

        // 신뢰도 가중치 합계 체크
        let sum: f32 = COMPOSITION_CONFIDENCE_WEIGHTS.iter().sum();
        assert!(
            (sum - 1.0).abs() < 0.01,
            "Confidence weights should sum to 1.0"
        );
    }
}
