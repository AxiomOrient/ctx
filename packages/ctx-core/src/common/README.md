# src/common

전역 타입/에러/상수 모듈. **모든 레이어가 참조** 가능.

## 포함
- `mod.rs`: 전역적으로 사용되는 핵심 데이터 타입 정의 (`ContextDocument`, `BuildQuery` 등).
- `errors.rs`: `thiserror` 기반의 전역 에러 타입(`ContextError`) 정의.
- `constants/`: 시스템 전반에서 사용되는 상수 모음.
- `rule_application.rs`: 규칙 적용 결과를 나타내는 타입 정의.
- `utils.rs`: 여러 모듈에서 공통으로 사용하는 유틸리티 함수 (텍스트 파싱 등).

## 금지
- 비즈니스 로직/IO 의존