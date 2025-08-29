# PLAN — FS‑First, Markdown‑Centric ctx

본 계획은 파일시스템(파일) 우선 전략으로, 온톨로지/룰/문서 모두를 Markdown+YAML로 관리하며, 메모리 인덱스와 결정적 알고리즘으로 고품질 프롬프트를 생성하는 데 초점을 둡니다. SQLite는 컨텐츠 인덱스 용도가 아닌 로그/분석/감사에만 사용합니다. 하나의 바이너리(ctx)가 CLI/MCP/경량 HTTP UI를 제공합니다.

## 0) 목표(Goals)
- 작은 규모(≈ 10–100개의 MD)에서 DB 없이도 일관되고 고품질의 컨텍스트/프롬프트를 생성
- 온톨로지/룰/문서 전부 파일로 관리 → Git 추적과 코드리뷰가 쉬움, 투명성 극대화
- 뷰/편집/검증/조합/AI 호출을 단일 바이너리(ctx, CLI/MCP/HTTP-UI)로 제공
- SQLite는 선택 사항이며, 로그·사용량·감사 이벤트 기록만 담당(컨텐츠 저장/색인 금지)

## 1) 핵심 원칙(Principles)
- FS‑First: 모든 지식(ontology.yaml, rules.yaml)과 문서(documents/*.md)는 파일로만 관리
- 결정성(Determinism): 동일 입력 → 동일 출력. 정렬/중복제거/예산/템플릿을 코드 레벨에서 강제
- 단순성(Simple & Precise): 현재 필요한 기능만 작고 견고하게. 확장은 feature gate로 격리
- 투명성(Transparency): 프롬프트의 근거(소스/점수/선택근거)를 항상 메타데이터로 노출

## 2) 리포지토리 구조(파일)
- `documents/` … 컨텍스트 문서(Markdown). YAML 프론트매터(`context.v1`) + 본문 섹션
- `knowledge/ontology.yaml` … 온톨로지(네임스페이스/값/동의어)
- `knowledge/rules.yaml` … YAML 규칙(정규식/키워드/코드펜스/링크 규칙 + requires/prohibits)
- `templates/` … (선택) 출력 템플릿 조각
- `config.toml` … 경로/예산/가중치/기능 토글

## 3) 데이터 모델
### 3.1 Context Document(frontmatter: `context.v1`)
- id, title, version, schema, type, tags[]
- sections[]: { id, name, marker("## …"), priority, tokens(optional) }
- (선택) facets: { ns → [values] }, locale, trust, freshness

### 3.2 Ontology (ontology.yaml)
- namespaces: { name → { values: { value → { synonyms[], parents[], deprecated } } } }
- normalize(ns, value|synonym) → canonical value

### 3.3 Rules (rules.yaml)
- regex_rules: { ns, value, pattern, weight }
- keyword_rules: { ns, value, keywords[], weight }  // keyword|value 매핑 지원
- codefence_rules: { ns, value, languages[], weight }  // ```lang 감지
- link_rules: { ns, value, host_contains[], weight }
- prohibited[] / requires[]  // 조합 제약/필수 동반

## 4) 메모리 인덱스(Memory Index) & 캐시
- 시작 시 `documents/` 스캔 → frontmatter/본문 파싱 → `DocumentCandidate` 생성
- 토큰 추정(기본 문자 기반, 선택적으로 tokenizer feature)·신선도(mtime)·신뢰도 등 계산
- 온톨로지 기반 정규화 + 규칙 적용(가중치 포함) 후 메모리 상에 인덱스 구성
- 해시/버전 키 기반 결정적 캐시(선택). 파일 변경 시 무효화(추후 파일감시 feature)

## 5) 파이프라인(7단계)
1) Normalize & Hash … 질의/옵션 정규화 + 실행ID 생성
2) Classify … 온톨로지/룰(가중치 포함) + RuleApplier(requires/prohibits)로 패싯 생성
3) Retrieve & Score … 메모리 인덱스에서 후보 산출, `DocumentScorer`로 점수화
4) MMR Select … 다양성/관련성 균형, 문서 단위 예산 고려
5) Token Trim … 문서 예산 초과 금지(±0%), 초과 시 컷
6) Template … 섹션 기반 합성(명시적 `sections:`/`parts:` 또는 패싯 기반 추론), 섹션 단위 예산 엄격 적용, 안정 정렬
7) Send(옵션) … AI Transport(Mock/CLI 연동), 기본은 프롬프트만 출력

## 6) 조합(Composition) 정책
- 섹션 우선: 문서 전체가 아닌 섹션 단위로 정확하게 합성
- 섹션 선택: `sections: a,b` 또는 `parts:` 디렉티브, 없으면 패싯(`artifact:`/`doc_process:`/`section:`)에서 추론
- 정렬: priority desc → id asc, 소스 목록/메타 포함, 항상 결정적
- 예산: 문서/섹션 이중 단계로 엄격 준수(±0%)

## 7) 인터페이스(단일 바이너리)
- CLI: `compose`, `classify-text`, `validate`, `mcp`, `server`(경량 UI)
- MCP(stdio JSON-RPC): `ctx.ping`, `ctx.classifyText`, `ctx.composePrompt`, `ctx.work`
- HTTP UI-Lite(선택): Markdown 뷰어/검증/조합 미리보기. 에디터는 안전한 원자적 저장(临时파일→fsync→rename)

## 8) SQLite(선택) — 로그/분석/감사 전용
- 금지: 컨텐츠/색인 저장. 허용: 이벤트/사용량/감사 레코드만
- 스키마 예: `events(id, ts, request_id, action, tokens, sources_json, exec_hash, outcome)`
- 기능 토글: `feature = "logs_sqlite"`

## 9) 설정(Config)
- `config.toml` + ENV override. 경로/예산/가중치/MMR λ/섹션 정책/HTTP 포트 등
- 경로 해석 우선순위: config → 기본값. 모든 경로는 안전하게 정규화/검증

## 10) 보안/결정성(Security/Determinism)
- 파일 경로 정규화/상위 경로 탈출 금지/허용 폴더 화이트리스트
- 최대 파일/프론트매터 크기 제한, 허용 확장자(MD/YAML)
- 정렬/중복제거/해시/버전 고정, 랜덤/시간 의존 로직 배제
- 저장은 원자적(rename), 실패 시 rollback. 로그에는 PII 최소/마스킹

## 11) 테스트(품질 게이트)
- Golden tests(20+):
  - 분류: 동의어/정규식/코드펜스/링크 규칙 → 기대 패싯
  - 조합: 명시/추론 섹션 선택, 다양한 예산(1K/2K/4K), 다문서
  - 결정성: 동일 입력 동일 바이트, 소스/섹션/정렬 안정성
- Property tests: 정렬/중복제거/예산 불변식, 파서 라운드트립
- Doc-tests: 공개 API 예제 검증
- Lints: `clippy -D warnings`, `fmt --check`

## 12) 로드맵(Roadmap)
### P0 — FS‑First 결정판
- 파이프라인/메모리 인덱스/섹션 합성/결정성/에러코드/구성 완료
- Golden/Property 테스트 커버리지 달성
- MCP 안정화, CLI UX 정제

### P1 — Tauri UI (Reader + Prompt) + 안전 저장

핵심 원칙: 읽기 우선 → 즉시 프롬프트 → 필요 시 지식/설정 토글(90% 쉬움 / 10% 고급).

- 메인 레이아웃(기본): Documents(좌)/Viewer(중)/Right Dock(우)
  - 상단바: [← Folder] <현재 경로> [Change…] · 🔎 Search(⌘/Ctrl+K) · [Prompt⇄Ontology/Rules] · [⚙]
  - Documents: 트리/리스트 토글, 최근/수정 정렬, 태그/섹션/mtime 배지, [Open]/[Compose→]/[Reveal]
  - Viewer: Frontmatter 배지(id|type|tags|sections|trust|freshness), 섹션 앵커, 코드 하이라이트, [Edit] 토글(인라인 편집, 원자 저장)
  - Right Dock: Prompt(Simple/Pro), Output, Sources/Budget/Hash, [JSON] 메타 토글
- Ontology·Rules 레이아웃(우측 패널 스왑): Ontology.yaml / Rules.yaml, View/Edit, 상태/검증/미니검색
- Settings(모달/드로어): General/Workspace/Compose/Advanced(보안/결정성)
- 단축키: ⌘/Ctrl+Enter Generate/Recompute · P Prompt 전환 · E 편집 · J JSON 메타 · ⌘/Ctrl+C 복사 · ⌘/Ctrl+O 폴더 변경

렌더링(기본값, 보안·결정성 우선): Rust에서 HTML 완성 → WebView는 표시만
- Markdown: comrak · Code highlight: syntect · HTML Sanitizer: ammonia
- 파일 변경 감지: notify(선택: tauri-plugin-fs-watch)

IPC(백엔드) 시그니처(Tauri)
- Workspace: `select_workspace(path) -> WorkspaceSummary`, `reindex() -> IndexSummary`
- Docs: `list_documents(filter?) -> [DocMeta]`, `read_document(id) -> { frontmatter, sections[], html }`, `update_document(id, patch|full) -> SaveResult`
- Ontology/Rules: `read_ontology()/update_ontology(full)`, `read_rules()/update_rules(full)`
- Compose: `classify(input) -> Facets`, `compose({query, sections?, budget?, lambda?, facets?, template?}) -> PromptBundle{ prompt, sources[], budget, hash }`
- Settings: `get_settings()/set_settings(partial)`

보안/결정성: 저장 tmp→fsync→rename, 경합 시 해시/mtime 검출 → 병합/덮어쓰기/취소, 스코프 밖 접근 차단.

#### 사용자 경험(UX) 극대화 방안
- **즉각적인 피드백 강화 (Live Preview)**: 사용자가 쿼리를 입력하거나 문서를 선택/해제할 때마다 프롬프트 결과가 실시간으로 업데이트되어 빠른 실험과 최적화가 가능하도록 지원합니다.
- **'What-If' 시나리오**: 특정 규칙을 UI 상에서 임시로 비활성화하여 프롬프트 결과 변화를 즉시 확인함으로써, 각 규칙의 영향도를 직관적으로 분석하고 디버깅할 수 있는 환경을 제공합니다.
- **시각화를 통한 투명성 극대화**: 최종 프롬프트에 선택된 문서 섹션을 하이라이트하고, 각 문서의 점수 기여도를 시각적으로 표시하여 '왜 이 컨텍스트가 선택되었는지' 명확하게 전달합니다.
- **지식 탐색 및 발견 용이성 증대**: 프롬프트 입력창에서 `ontology.yaml` 기반으로 네임스페이스와 값을 자동완성하여 사용자의 정확한 입력을 유도합니다.

수용 기준(AC)
- 문서 뷰/섹션/프롬프트 미리보기 가능, Sources/Budget/Hash 항상 노출
- Generate/Copy/Save as… 동작, 폴더 변경/리인덱스 가능
- 저장은 원자적이며 경합/경로 위반 시 표준화된 에러코드 배너 제공

구현 체크리스트(Phase)
- Phase 1(핵심): 메인 레이아웃 + Prompt(Simple) + 폴더 변경/리인덱스
- Phase 2(고급): Prompt(Pro) + JSON 메타 토글 + 인라인 편집(원자 저장)
- Phase 3(지식/설정): Ontology/Rules 패널, Settings 모달
- Phase 4(다듬기): 전환 상태 유지, 통합 검색, 에러코드 배너/상세보기

### P2 — IDE 플러그인(경량)
- MCP 기반 단축키/명령 팔레트에서 `compose`/`work` 호출
- 문맥 파일 변경 감지 후 로컬 재인덱스 트리거(선택: 파일 감시 feature)

#### 추가 제안
- **컨텍스트 인지형 명령어**: 현재 열려있는 파일이나 선택된 코드 블록을 자동으로 `compose` 명령어의 입력 컨텍스트로 활용합니다.
- **인라인 진단(Inline Diagnostics)**: `validate` 기능을 활용하여, `ontology.yaml` 또는 `rules.yaml` 파일의 오류를 IDE 에디터 상에서 실시간으로 표시합니다.
- **CodeLens 연동**: 문서의 각 섹션별 관련 패싯 정보를 에디터 상에 직접 표시하여(예: `## Section [Facets: api, rust]`) 문서 관리 효율을 높입니다.

### P3 — 로그/분석 및 시스템 최적화 (선택)
단순 로깅을 넘어, 수집된 데이터를 통해 시스템이 스스로 학습하고 품질을 개선하는 기반을 마련합니다.

#### 1. 로그 스키마 확장
`events` 테이블에 아래 컬럼을 추가하여 사용자 상호작용을 더 구체적으로 기록합니다.
- `prompt_query` (TEXT): 사용자가 입력한 실제 쿼리 문자열.
- `prompt_options_json` (TEXT): `sections`, `budget` 등 쿼리와 함께 전달된 옵션을 JSON으로 저장.
- `user_feedback` (INTEGER): (선택) UI에 '좋아요/싫어요' (👍/👎) 버튼을 추가하여 생성된 프롬프트에 대한 피드백을 수집 (1: 좋아요, -1: 싫어요, 0: 피드백 없음).

#### 2. 수집된 데이터를 활용한 온톨로지/룰 개선 방안
- **규칙 사용 빈도 분석**: 가장 자주 활성화되는 규칙과 온톨로지 값을 식별하고, 반대로 전혀 사용되지 않는 규칙("Zero-Hit" Rules)을 찾아내어 룰베이스를 최적화합니다.
- **규칙 간 상관관계 분석**: 함께 자주 등장하는 패싯 쌍을 분석하여 `requires` 관계를 추가하거나 신규 규칙을 생성하는 등 룰베이스를 고도화합니다.
- **사용자 피드백 기반 점수 자동 튜닝 (RLHF)**: '좋아요'를 받은 프롬프트에 기여한 규칙의 가중치를 높이고, '싫어요'를 받은 규칙의 가중치를 낮추는 강화 학습 루프를 통해 시스템의 프롬프트 생성 품질을 점진적으로 향상시킵니다.
- **회귀 테스트 케이스 자동 발굴**: 사용 빈도와 긍정적 피드백이 많은 `(쿼리, 결과)` 쌍을 자동으로 'Golden Test' 케이스로 편입하여, 시스템 변경 후에도 품질이 유지되는지 검증하는 데 사용합니다.

## 13) 수용 기준(Acceptance)
- P0: 동일 입력 동일 출력, 예산 초과 0%, 섹션 선택 정확, 패싯 정규화 일관, Golden/Property 전부 통과
- P1: UI로 문서/섹션/프롬프트 미리보기 가능, 저장은 원자적/경합 안전
- P2: IDE에서 MCP 호출/프롬프트 삽입/소스 메타 표시
- P3: 이벤트 로그/간단 리포트, 컨텐츠 DB 저장 없음

## 14) 비범위(Non‑Goals)
- 컨텐츠/색인용 SQLite/Vektor DB 사용 금지(선택적 실험 feature 제외)
- 외부 AI SDK 종속(기본) 금지. CLI 연동/Mock 우선
- 과도한 자동 요약/변형. 사용자 문서의 의미 보존이 최우선

---
