# ctx

`ctx`는 로컬 문서에서 **연결 관계, 위반·불확실 상태, 근거 위치**를 모델 없이 재현하는 작은 지식 그래프입니다.

핵심 질문은 세 가지입니다.

1. 무엇이 연결되어 있는가?
2. 무엇이 `Violated` 또는 `Unknown`인가?
3. 그 판단의 근거가 어디에 있는가?

## Canonical workspace

정본은 두 종류뿐입니다.

```text
workspace/
├── schema.yaml
└── knowledge/
    ├── release.md
    ├── privacy.md
    └── ...
```

- `schema.yaml`: 타입, 관계, 제약
- `knowledge/**/*.md`: 엔티티, 관계 선언, 사람이 읽는 지식
- 관계가 참조하는 텍스트 파일: evidence source

별도 topology 파일은 없습니다. 그래프는 로드할 때 Markdown frontmatter에서 결정적으로 컴파일됩니다.

```text
schema.yaml + knowledge/**/*.md
              │
              ▼
        GraphSnapshot
              │
              ▼
     deterministic validation
              │
              ▼
 Satisfied / Violated / Unknown
```

## Schema

지원하는 제약은 현재 필요한 것만 유지합니다.

- relation `from` / `to`
- `min` / `max`
- `acyclic`
- `symmetric`

```yaml
version: 1

types:
  Step:
    constraints:
      requires:
        min: 1
  Artifact: {}

relations:
  requires:
    from: [Step]
    to: [Artifact]
    acyclic: true
```

`symmetric: true`인 관계는 `from`과 `to`의 타입 집합이 같아야 합니다.

엔티티 ID, 타입명, 관계명은 안정적인 machine key입니다. 허용 문자는 ASCII 영숫자와 `.`, `_`, `:`, `-`입니다. 제목과 alias는 Unicode를 사용할 수 있습니다.

## Knowledge document

각 Markdown 파일은 하나의 엔티티를 선언합니다.

```markdown
---
id: step.release
type: Step
title: Release
aliases: [publish]

relations:
  - relation: requires
    target: artifact.privacy
    evidence:
      - exact: "Publishing requires the privacy notice to be reviewed."
        prefix: "# Release\n\n"
        suffix: "\nThe checklist remains blocked."
        hint:
          start: 14
          end: 14
---
# Release

Publishing requires the privacy notice to be reviewed.
The checklist remains blocked.
```

관계는 추론된 사실이 아니라 명시적 assertion입니다.

## Evidence

Evidence selector는 다음 필드를 사용합니다.

- `exact`: 필수. 근거가 되는 정확한 텍스트
- `prefix`: 선택. `exact` 직전 문맥
- `suffix`: 선택. `exact` 직후 문맥
- `hint`: 선택. 1-based line range 힌트
- `source`: 선택. 생략하면 관계를 선언한 Markdown 파일

`exact`가 정본입니다. `prefix` 또는 `suffix`를 선언했다면 그것도 selector의 일부로 반드시 일치해야 합니다. `hint`는 위치 힌트일 뿐 근거 판정 권한이 없습니다.

상태는 여섯 가지입니다.

- `valid`: selector가 유일하게 일치
- `relocated`: 근거는 유효하지만 line hint와 실제 위치가 다름
- `stale`: exact 또는 선언한 문맥이 더 이상 일치하지 않음
- `ambiguous`: 둘 이상의 위치가 일치
- `missing`: source가 없음
- `invalid`: selector, 경로, line hint 또는 UTF-8 입력이 잘못됨

Markdown frontmatter는 evidence 검색 대상에서 제외합니다. 텍스트 비교와 query lookup은 Unicode NFC로 정규화합니다.

## Validation

검증 결과는 단순 boolean이 아니라 근거를 포함합니다.

```text
Satisfied + satisfaction trace
Violated  + failure witness
Unknown   + unresolved evidence
```

근거가 없거나 찾을 수 없다는 사실은 `false`나 성공으로 변환하지 않습니다.

`check`는 `Violated` 또는 `Unknown`이 하나라도 있으면 non-zero로 종료합니다. `explain`, `path`, `query`는 구조적 `Violated`가 없으면 실행할 수 있습니다.

## CLI

```bash
ctx --workspace ./workspace check
ctx --workspace ./workspace explain step.release --depth 2
ctx --workspace ./workspace path step.release artifact.privacy
ctx --workspace ./workspace query publish
```

`--json`을 추가하면 구조화된 JSON을 출력합니다.

### check

schema version, 타입·관계 참조, domain/range, cardinality, duplicate assertion, cycle, evidence를 검증합니다.

### explain

엔티티 주변 그래프와 각 관계의 실제 evidence resolution을 함께 출력합니다.

### path

선언된 방향 그래프에서 최단 경로를 찾습니다. `symmetric` 관계만 양방향으로 탐색합니다.

### query

다음 순서로 결정적 lookup을 수행합니다.

1. ID
2. title
3. alias
4. substring

입력의 바깥 공백을 제거하고 NFC 정규화 후 비교합니다.

## Optional decision adapter

자연어 질의가 결정적 lookup으로 하나의 엔티티에 수렴하지 않을 때만 외부 executable을 사용할 수 있습니다.

```bash
ctx --workspace ./workspace query "배포 개인정보 문서" \
  --decider ./my-adapter
```

adapter는 stdin으로 한 개의 JSON object를 받습니다.

```json
{
  "version": 1,
  "task": "select_entity",
  "query": "배포 개인정보 문서",
  "candidates": [
    {
      "id": "artifact.privacy",
      "kind": "Artifact",
      "title": "Privacy notice",
      "aliases": []
    }
  ]
}
```

stdout에는 한 개의 JSON object만 반환합니다.

```json
{
  "entity_id": "artifact.privacy",
  "confidence": 0.91
}
```

규칙:

- `entity_id`는 `null`일 수 있음
- 반환 ID는 반드시 전달된 candidate 안에 있어야 함
- `confidence`는 선택이며 finite `0...1`
- confidence는 graph/evidence truth가 아님
- exact ID/title/alias가 하나면 adapter를 호출하지 않음
- exact match가 여러 개면 그 집합만 candidate로 전달
- exact match가 없으면 substring 결과와 무관하게 전체 entity를 candidate로 전달
- 30초가 지나면 adapter를 종료
- stdout이 64 KiB를 넘으면 실패
- 로그는 stderr 사용

모델·토크나이저·런타임·체크포인트는 adapter 책임이며 `ctx` 데이터 계약에 포함되지 않습니다.

## Scope

권위 있는 core에는 파일 기반 정본, graph compile, validation, evidence resolution, deterministic lookup만 있습니다. 캐시·검색 인덱스·모델 런타임은 core의 진실 소유자가 아닙니다.

## Development

`Cargo.lock`을 정본에 포함하며 검증은 lockfile 기준으로 수행합니다.

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings

cargo run --locked -- --workspace examples check
cargo run --locked -- --workspace examples explain step.release --depth 2
cargo run --locked -- --workspace examples path step.release artifact.privacy
cargo run --locked -- --workspace examples query publish
```
