# src/core/classifier

패싯 분류 엔진 (키워드/정규식/링크/코드펜스 + 룰 적용).

## 공개 API (서비스 파사드)
- `ClassifierService::classify_with_yaml(ontology_yaml, rules_yaml, title, body) -> Result<Classification>`
  - 애플리케이션 레이어에서 사용하는 유일한 진입점입니다.
  - YAML 문자열을 직접 받아 `knowledge` 모듈 의존성을 숨깁니다.

## 구조
- `service.rs`: 애플리케이션 레이어를 위한 파사드.
- `engine.rs`: 키워드, 정규식 등을 이용한 실제 분류 로직 수행.
- `facet.rs`: 분류 결과(`Classification`)와 패싯(`Facets`) 데이터 구조 정의.
- `normalizer.rs`: 온톨로지를 기반으로 토큰, 동의어 등을 정규화.

## 설계
- 신뢰도 계산은 상수/정책 기반(`common/constants/scoring.rs`)을 따릅니다.
- 결과는 `facet.rs`의 `Classification` 타입으로 표준화됩니다.
- `ClassifierService`를 통해 `core`와 `app` 레이어 간의 의존성을 분리합니다.