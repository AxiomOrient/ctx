# src/doc/validate/stages

`ValidationStage` 트레이트를 구현하는 개별 검증 단계 모음입니다.

- `metadata.rs`: `MetadataValidationStage`가 문서의 ID, 제목 등 기본 메타데이터를 검증합니다.
- `schema.rs`: `SchemaValidationStage`가 섹션 정의의 유효성(중복, 우선순위 등)을 검증합니다.
- `structure.rs`: `StructureValidationStage`가 문서 본문에 섹션 마커가 실제로 존재하는지 등 구조적 무결성을 검증합니다.

**규칙:** 새로운 검증 로직은 `ValidationStage`를 구현하여 이 디렉토리에 추가합니다.