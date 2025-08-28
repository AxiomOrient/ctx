# src/common/constants

도메인 상수 모음 (유일 출처).

- `composition.rs`: 문서 조합(composition) 관련 상수 (MMR, 토큰 예산 등).
- `defaults.rs`: 시스템 전반의 기본값 상수.
- `determinism.rs`: 결정성 보장을 위한 상수 (유사도, 해시 등).
- `graph.rs`: 의존성 그래프 분석 관련 상수.
- `parsing.rs`: Frontmatter, 섹션 등 파싱 관련 상수.
- `scoring.rs`: 문서 점수 계산 가중치 및 파라미터.
- `validation.rs`: 검증 로직에서 사용하는 임계값 및 정책.
- `versioning.rs`: 시스템의 각 구성 요소 버전 관리.

**규칙:** 코드 내 하드코딩 값 금지 → 반드시 여기로 이동.