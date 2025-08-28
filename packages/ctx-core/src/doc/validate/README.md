# src/doc/validate

검증 파이프라인(메타데이터 → 스키마 → 구조 → 카테고리 룰).

## 구조
- `pipeline.rs`: `ValidationStage` 트레이트와 이를 실행하는 `ValidationPipeline`을 정의합니다.
- `stages/` : 단계별 검증기

## 출력
- `ValidationReport`: 각 스테이지의 성공/실패 여부를 담은 보고서를 생성합니다.