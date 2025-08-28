# CtxSet: Curated Context Set Generator

[![Rust](https://github.com/axient/ctxset/actions/workflows/rust.yml/badge.svg)](https://github.com/axient/ctxset/actions/workflows/rust.yml)

`ctxset`은 AI 프롬프트 엔지니어링을 위한 컨텍스트 관리/생성 도구입니다. 문서의 의미를 온톨로지로 분류하고, 규칙과 의존성을 고려해 안정적으로 인덱싱/조합할 수 있게 합니다.

## ✨ 주요 기능

- 지능형 문서 분류: `ontology.yaml` + 규칙(`rules.yaml`) 기반 다차원 패싯 추출
- SQLite 인덱스: 문서/패싯/로그 영속화 및 빠른 후보 조회
- 안전한 파일 IO: 경로 정규화로 경로 탐색 차단, 테스트 포함
- 강력한 CLI: 분류, 파싱, 가져오기, 인덱싱, 검증, GitHub 연동, MCP 서버

## 🚀 시작하기

### 요구사항

- Rust (stable)
- Git (일부 명령 및 GitHub 연동 시 필요)
- GitHub CLI `gh` (GitHub 연동 사용 시)

### 설치

```bash
cargo install --path .
```

또는 개발 모드로 실행:

```bash
cargo run -- <subcommand> [...args]
```

## 📦 설정 파일 (선택)

프로젝트 루트의 `ctxset.toml`로 기본 경로를 지정할 수 있습니다.

```toml
# ctxset.toml
contexts_dir = "./contexts"
ontology = "./ontology.yaml"
rules = "./rules.yaml"
db = "./ctxindex.db"
```

명시적 CLI 인자는 설정 파일보다 우선합니다.

## 🧰 CLI 명령어

실제 코드에 존재하는 명령만 나열합니다.

### parse
- 설명: Markdown에서 frontmatter를 파싱하고 출력
- 예시:
  - `ctxset parse ./contexts/EXAMPLE.md --format yaml`

### classify
- 설명: 파일 내용을 온톨로지/규칙 기반으로 분류
- 주요 옵션: `--ontology <file>`, `--format human|json|yaml`, `--confidence-threshold <f32>`
- 예시:
  - `ctxset classify ./documents/note.md --format human`

### import
- 설명: 비표준 Markdown을 표준 `context.v1` 문서로 변환하여 `contexts` 디렉토리에 저장
- 기본: 원래 파일명을 유지, 충돌 시 스킵; `--interactive`로 덮어쓰기/이름변경/중단 선택 가능
- 주요 옵션: `--contexts-dir <dir>`, `--interactive`, `--force`, `--type`, `--domain`, `--tags`, `--dry-run`
- 예시:
  - `ctxset import ./documents --contexts-dir ./contexts --interactive`

### index
- 설명: 디렉토리의 Markdown을 재귀적으로 스캔/분류하여 SQLite 인덱스에 배치 업서트
- 주요 옵션: `--path <dir>`, `--db <file>`, `--ontology <file>`, `--rules <file>`
- 예시:
  - `ctxset index --path ./contexts --db ./ctxindex.db`

### validate
- 설명: 단일 문서를 `context.v1` 스키마로 검증 (오류/경고 출력)
- 주요 옵션: `--format human|json|yaml`
- 예시:
  - `ctxset validate ./contexts/MY-DOC.md --format human`

### github
- 설명: GitHub CLI(`gh`) 래핑 + 분류 기반 확장 기능
- 서브커맨드:
  - `issue --title <t> [--body <b>] [--labels <csv>] [--repo org/repo]`
  - `pr --title <t> [--body <b>] [--base main] [--draft] [--repo org/repo]`
  - `label-sync --file labels.json [--repo org/repo]` (기존)
  - `create-issue-from-file --from-file <path> [--repo org/repo] [--ontology <file>] [--rules <file>]`
    - 파일을 분류해 `ns:value` 라벨을 자동 적용하여 이슈 생성
  - `sync-labels-from-ontology [--repo org/repo] [--ontology <file>]`
    - `ontology.yaml`의 값들로 레이블을 생성/동기화

### mcp
- 설명: MCP(Model Context Protocol) 서버(stdio) 시작
- 예시:
  - `ctxset mcp`
  - `echo '{"jsonrpc":"2.0","id":1,"method":"ping"}' | ctxset mcp`

### server (feature=server)
- 설명: HTTP API 서버 시작
- 예시:
  - `ctxset server --port 3000 --host 127.0.0.1`

## 📄 문서 포맷: context.v1

모든 컨텍스트 문서는 YAML frontmatter + Markdown 본문을 따릅니다.

```yaml
---
id: "RUST-API-GUIDE"
title: "Rust API Development Guide"
version: "1.0.0"
schema: "context.v1"
type: "guide"
sections:
  - id: "intro"
    name: "Introduction"
    marker: "## Introduction"
    priority: 90
---

## Introduction
... 본문 ...
```

유효성 검사는 `ctxset validate`로 수행할 수 있습니다.

## 🧪 개발 가이드

- 빌드: `cargo build` (릴리즈: `cargo build --release`)
- 테스트: `cargo test`
- 포맷: `cargo fmt --all`
- 린트: `cargo clippy --all-targets --all-features -- -D warnings`

## 🔐 보안 노트

- 로컬 스토리지 경로는 정규화되어 베이스 경로 이탈을 차단합니다. 테스트 포함.
- 규칙 적용은 경고/오류를 분리하여 안전하게 수행합니다.

## 🧱 아키텍처 개요

- 레이어드 아키텍처: `types → knowledge → doc → core → data → app`
- 핵심 구성요소
  - 분류 엔진: 온톨로지/규칙/정규식 기반, 신뢰도는 증거 가중 포화 함수로 계산
  - 인덱스: SQLite(rusqlite) + 풀링(r2d2), 배치 업서트/통계/최적화 지원
  - 검증기: `context.v1` 스키마 검증(오류/경고)
  - CLI: 명령 단위의 작은 모듈로 구성

자세한 구조는 `docs/` 및 소스 모듈을 참고하세요.

## 🤝 기여

PR 전 체크리스트:

- `cargo test` 통과 (unit + integration)
- `cargo clippy -- -D warnings` 무경고
- `cargo fmt --all` 적용
- CLI 변경 시 README/docs 업데이트

