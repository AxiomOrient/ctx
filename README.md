# ctx — FS-First, Markdown-Centric Context Composer

[![Rust](https://github.com/axient/ctx/actions/workflows/rust.yml/badge.svg)](https://github.com/axient/ctx/actions/workflows/rust.yml)

ctx는 파일시스템(문서·온톨로지·룰) 기반으로 안정적이고 결정론적인 프롬프트 컨텍스트를 생성합니다. 분류(ontology/rules) → 후보 검색/점수화 → MMR 선택 → 예산 컷 → 템플릿 합성을 단일 바이너리에서 제공합니다.

**모드**
- CLI: 로컬 파이프라인/도구 실행
- MCP: rmcp SDK 기반 stdio 서버 (단일 경로)
- UI(Tauri + Svelte 5): 문서 뷰어/Prompt 패널/지식/설정 (feature-gated)

**핵심 원칙**
- FS-First: 문서/온톨로지/룰은 파일로 관리, Git 추적 최적화
- Deterministic: 동일 입력 → 동일 출력 (정렬/중복/예산 엄격)
- Secure-by-default: 서버측 렌더링 + sanitizer, 원자 저장(tmp→fsync→rename), 워크스페이스 스코프

## Install / Run

- Build: `cargo build` (release: `cargo build --release`)
- Run (CLI): `cargo run -- <command> [...args]`
- Run (MCP): `cargo run -- mcp`
- Run (UI dev):
  - Frontend: `cd apps/ui && npm i && npm run dev`
  - Tauri window: `cargo run --features ui_tauri,ui_ipc,ui_render_rust -- ui`

## CLI Commands (subset)
- `parse <file> --format yaml|json`
- `classify <file> --format human|json|yaml`
- `import <input> [--contexts-dir <dir>] [--interactive]`
- `index --path <dir> --db <file> [--ontology <file>] [--rules <file>]`
- `validate <file> --format human|json|yaml`
- `server --port 3000 --host 127.0.0.1` (feature=http)
- `mcp` (rmcp SDK stdio server)

## UI (P1)
- 레이아웃: 좌 Documents / 중앙 Viewer / 우 Prompt(기본) 또는 Ontology/Rules(토글)
- 상태: SSOT(Single Source Of Truth) `apps/ui/src/lib/ssot.ts`
- 단축키: ⌘/Ctrl+Enter Generate, P 패널 전환, E 편집 토글, J JSON 메타, ⌘/Ctrl+O 폴더 변경
- IPC: `select_workspace`, `list_documents`, `read_document`, `update_document`, `compose`, `read_ontology/update_ontology`, `read_rules/update_rules`, `get_settings/set_settings`
- 렌더링(feature=ui_render_rust): comrak + syntect + ammonia → 안전한 HTML만 WebView로 전달

## Features
- `mcp_sdk` (default): rmcp 기반 MCP 서버
- `http`: 경량 HTTP 모드
- `ui_ipc`: UI IPC 백엔드
- `ui_render_rust`: 서버측 Markdown→HTML 렌더 + Sanitizer
- `ui_tauri`: Tauri 창/커맨드 바인딩 (UI 실행)

## Testing / Golden
- Compose goldens: `UPDATE_GOLDEN=1 cargo test -q --test compose_golden`
- MCP schema goldens: `UPDATE_GOLDEN=1 cargo test -q --test mcp_schema`
- UI renderer goldens: `UPDATE_GOLDEN=1 cargo test -q --test ui_render_golden --features ui_render_rust`
- Lint/format: `cargo fmt --check`, `cargo clippy -D warnings`

## Security / Determinism
- 경로 정규화·워크스페이스 스코프, 허용 확장자/크기 제한(구성), 원자 저장 루틴 준수
- 서버측 렌더링 + sanitizer(허용 태그/클래스 최소화), 상대 URL 차단
- 동일 입력 → 동일 출력, 시계/랜덤 의존 제거

## File Structure
- 최신 구조는 `docs/FILE_STRUCTURE.md` 참조

## Contributing
- PR 전: `cargo test`, `cargo clippy -D warnings`, `cargo fmt --all`
- CLI/IPC/스키마 변경 시 README와 `docs/`를 갱신하세요
