# src/doc/schema

문서 스키마(버전드) 정의 및 검증.

## 구조
- `mod.rs`: `SchemaValidator` 트레이트와 `ValidationResult` 등 공통 타입을 정의합니다.
- `document.rs`: 새로운 패싯 기반 문서 스키마인 `FacetDocument`를 정의합니다. (향후 주력 스키마)
- `v1.rs`: `context.v1` 스키마에 대한 `SchemaValidator` 구현체(`V1SchemaValidator`)를 제공합니다.

## 원칙
- 새로운 스키마 버전 추가 시, `v_X_.rs` 파일을 생성하고 `SchemaValidator`를 구현합니다.