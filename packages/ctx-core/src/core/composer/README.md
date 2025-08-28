# src/core/composer

분류 결과를 점수화/선택/병합하여 최종 프롬프트/문서 생성.

## 공개 API (파사드)
- `BuildComposer::new()`: 컴포저 생성.
- `BuildComposer::compose(candidates, query, budget, reserve) -> Result<CompositionResult>`: 전체 조합 파이프라인 실행.

## 구조 (파이프라인)
1. **`scorer/`**: `DocumentScorer`가 후보 문서(`DocumentCandidate`)들의 점수를 다양한 기준(키워드, 신선도, 패싯 커버리지 등)으로 계산.
2. **`selector.rs`**: `DocumentSelector`가 점수가 매겨진 후보들 중에서 MMR(Maximal Marginal Relevance)과 배낭 알고리즘을 이용해 예산 내에서 최적의 문서를 선택.
3. **`merger.rs`**: `DocumentMerger`가 선택된 문서들을 하나의 최종 문서(`MergedDocument`)로 병합.

- `mod.rs`: 위 모듈들을 조합하여 전체 파이프라인을 실행하는 `BuildComposer` 파사드를 정의.
- `types.rs`, `query.rs`: `common` 모듈의 `BuildQuery`, `DocumentCandidate` 등 핵심 타입을 재내보내기.

## 입력/출력
- **입력**: `Vec<DocumentCandidate>`, `&BuildQuery`, `budget: usize`, `reserve: usize`
- **출력**: `CompositionResult` (병합된 문서, 선택 근거, 사용된 토큰, 신뢰도 점수 등 포함)

## 설계 원칙
- 모든 매직 넘버와 상수는 `common/constants/` 디렉토리의 파일들 (예: `composition.rs`, `scoring.rs`)에서 관리.