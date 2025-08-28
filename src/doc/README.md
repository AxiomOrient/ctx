# src/doc

표준 문서 형식(`ContextDocument`)의 파싱, 스키마 정의, 검증을 담당하는 모듈입니다.

이 모듈은 마크다운 파일과 그 안에 포함된 frontmatter를 구조화된 데이터로 변환하고, 정의된 규칙에 따라 유효성을 보장하는 핵심 역할을 합니다.

## 하위 모듈
- `parse/`: 마크다운 파일의 frontmatter와 본문을 파싱하여 `ContextDocument`와 `ExtractedSection`을 생성합니다.
- `schema/`: `ContextDocument`의 데이터 구조와 버전별 스키마(`v1`)를 정의하고, 스키마 자체의 유효성을 검증하는 로직을 포함합니다.
- `validate/`: 여러 검증 단계(메타데이터, 스키마, 구조 등)를 조합하여 실행하는 파이프라인을 제공합니다.

## 파일별 개요

다음 내용은 `packages/ctx-core/src/doc` 폴더의 모든 주요 파일을 요약한 것입니다.

- `mod.rs`: `parse`, `schema`, `validate` 서브모듈을 공개합니다.

### parse/
- `parse/mod.rs`: `document`, `frontmatter`, `section` 서브모듈을 공개하고 re-export 합니다.
- `parse/frontmatter.rs`:
  - `FrontmatterParser` 구현. `parse`(frontmatter+본문 분리→`ContextMetadata`), `extract_frontmatter`(구분자 기반 분리), `parse_yaml`(serde_yaml), `validate_metadata`(제목/버전/섹션/중복 검사) 제공.
  - 테스트: 정상/비정상 케이스 검증(semver, 빈 제목, 섹션 중복 등).
- `parse/document.rs`:
  - `DocumentParser` 구현. `parse_file_with_body`/`parse_file`/`parse_content_with_body`/`parse_content` 제공.
  - Frontmatter만 1회 읽고 `ContextDocument`로 역직렬화, 빈 `id`는 파일명에서 대문자로 보정, `path` 저장, `validate_schema()` 호출로 스키마 검증.
  - 테스트: 파일/문자열 입력, ID fallback(파일명), 섹션·본문 추출 검증.
- `parse/section.rs`:
  - `SectionExtractor` 구현. 문서 정의의 `sections[].marker`를 기준으로 본문에서 섹션 블록 추출, 간단한 토큰 수 추정.
  - 테스트: 다중 섹션 추출 및 내용/순서 확인.

### schema/
- `schema/mod.rs`:
  - 스키마 공통 타입 정의: `SchemaValidator` 트레이트, `ValidationResult`, `ValidationError(…Type)`, `ValidationWarning`.
  - `FacetDocument`(차세대 패싯 기반 메타데이터) 정의: `facets`/`trust`/`freshness`/`aliases`/`requires`/`conflicts`/`sections` 등.
- `schema/v1.rs`:
  - `V1SchemaValidator` 구현. 허용 타입(guide/persona/workflow/domain), 필수 필드 집합(내부 준비용), ID/버전 형식 검사, 스키마 호환성, 섹션 중복/마커/우선순위 검사, 권고성 경고(tags/author/updated/estimated_tokens) 생성.
  - 테스트: 유효/무효 문서, 중복 섹션, 경고 생성 시나리오 검증.
- `schema/document.rs`:
  - `FacetDocument` 세부 동작: `has_facet`/`add_facet`/`deduplicate_facets`/`days_since_freshness` 등 유틸 제공.

### validate/
- `validate/mod.rs`:
  - `pipeline`/`stages` 서브모듈 공개 및 re-export.
- `validate/pipeline.rs`:
  - `ValidationStage` 트레이트(단계명/검증 함수), `ValidationPipeline`(스테이지 추가/실행/카운트), `ValidationReport`(성공·실패 단계/요약) 구현.
  - 테스트: 모든 단계 통과/부분 실패 시나리오 검증.
- `validate/stages/metadata.rs`:
  - 기본 메타데이터(ID/제목/버전 포맷) 검증.
- `validate/stages/schema.rs`:
  - 섹션 존재/ID 중복/우선순위 상한(<=100) 검증.
- `validate/stages/structure.rs`:
  - 본문 공백 금지, 정의된 `marker`가 실제 본문에 존재하는지 확인.

## 주요 보장(인변트) 요약
- 문서는 frontmatter 구분자(`---`)로 시작·종료되어야 하며, YAML은 파싱 가능해야 합니다.
- `ContextDocument` 필수 필드(ID, 제목, 버전, 스키마, 타입, 섹션)를 갖추고 스키마(`context.v1`)와 호환되어야 합니다.
- 섹션은 최소 1개 이상이며, 중복 ID는 허용되지 않고, 마커는 `## `로 시작해야 합니다.
- 본문에는 정의된 모든 섹션 마커가 실제로 존재해야 합니다.
- 파일명 기반 ID 보정(빈 ID인 경우 파일명 대문자)이 적용됩니다.

## 가이드 준수 현황 (RULES.md / RUST.md)
- 에러 처리: `Result` 전파와 구체 오류 타입(`ContextError`, 검증 에러 구조) 사용. 패닉 회피.
- 테스트: 각 유닛 모듈에 정상/비정상 경로 포함 테스트 존재. AAA 형태 준수. 파일 IO는 임시 디렉터리 활용.
- 네이밍/스타일: snake_case/SCREAMING_SNAKE_CASE 준수, 공개 API에 요약 주석 제공(보강 여지 있음).
- 안전성: `unsafe` 미사용. 입력값 검증 철저(frontmatter/스키마/구조 단계 분리).
- 성능/단순성: 파일 1회 읽기, 필요한 수준의 토큰 추정 등 KISS/KYAGNI 준수.

참고: 워크스페이스 전체 `clippy -D warnings` 실행 시 `core/determinism.rs`의 `fn default()` 명명 경고가 감지됩니다(스키마/문서 모듈과 직접 관련 없음). 해당 경고는 `Default` 트레이트 구현 또는 메서드명 변경으로 해결 가능합니다.

## 사용 예시
1) 파일 파싱 → 문서+본문 획득
```
let (doc, body) = DocumentParser::new().parse_file_with_body(&storage, path)?;
```
2) 섹션 추출
```
let sections = SectionExtractor::new().extract_sections(&doc, &body)?;
```
3) 검증 파이프라인 실행
```
let mut pipeline = ValidationPipeline::new();
pipeline.add_stage(MetadataValidationStage);
pipeline.add_stage(SchemaValidationStage);
pipeline.add_stage(StructureValidationStage);
let report = pipeline.run(&doc, &body)?;
assert!(report.is_valid);
```

## 개선 제안(경미)
- 공개 타입/함수의 문서 주석을 조금 더 보강하면(RUST.md 권고) 가독성과 IDE 헬프가 향상됩니다.
- `V1SchemaValidator.required_fields`는 현재 내부 준비용으로만 선언되어 있으므로, 실제 사용 또는 제거 중 하나로 정리하면 좋습니다.
