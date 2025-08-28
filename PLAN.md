1. 현재 프로젝트의 주요 구조 및 구성 요소
이 프로젝트는 **두 개의 주요 크레이트(ctx-core, ctx-cli)**로 구성된 워크스페이스이며, 명확한 6계층 아키텍처를 따르고 있습니다. 각 계층은 단방향으로 의존하여 코드의 유지보수성과 확장성을 높입니다.

ctx-core (핵심 로직 크레이트): 애플리케이션의 모든 비즈니스 로직을 담당합니다.

common: ContextDocument, BuildQuery 와 같은 핵심 데이터 구조, 전역 에러 타입(ContextError), 그리고 시스템 전반에서 사용되는 상수(scoring, determinism 등)를 정의합니다.

knowledge: ontology.yaml(개념/동의어 정의)과 rules.yaml(분류 규칙)을 로드하고 관리하여 시스템의 "지식 베이스" 역할을 합니다.

doc: 마크다운 문서를 파싱하고(parse), 정의된 스키마(schema)에 따라 유효성을 검증(validate)합니다.

core: 가장 핵심적인 비즈니스 로직이 위치합니다.

classifier: 온톨로지와 규칙을 기반으로 문서를 자동으로 분류하고 신뢰도 점수를 매깁니다.

composer: 점수화된 문서들을 MMR(Maximal Marginal Relevance) 알고리즘을 사용해 관련성과 다양성을 모두 고려하여 최적의 프롬프트를 조합합니다.

determinism: 입력값을 정규화하고 해시를 생성하여, 동일 입력에 대해 항상 동일한 출력을 보장하는 결정성을 구현합니다.

data: 데이터 영속성을 관리합니다.

index: SQLite를 사용해 문서 메타데이터와 분류 결과를 인덱싱하여 빠른 검색을 지원합니다.

storage: 로컬 파일 시스템 접근을 추상화합니다.

ctx-cli (인터페이스 크레이트): ctx-core의 기능을 사용자나 외부 시스템에 노출하는 역할을 합니다.

CLI (app/cli): classify, index, compose 등 다양한 커맨드라인 명령어를 제공합니다.

HTTP 서버 (app/server): Axum 프레임워크를 기반으로 REST API와 웹 UI를 제공하며, 외부 시스템과의 연동을 위한 Webhook 엔드포인트를 포함합니다.

MCP 서버 (app/cli/commands/mcp.rs): stdio를 통해 JSON-RPC 형식으로 통신하는 MCP(Model Context Protocol) 서버 기능을 수행하여 AI 에이전트와의 연동을 지원합니다.

2. AI 연동 모델 또는 API
현재 코드베이스와 설계 문서에서는 특정 AI 모델(예: OpenAI, Gemini, Claude)의 API를 직접 호출하는 부분은 없습니다.

이 시스템의 핵심은 AI 모델에 **"입력으로 제공될 컨텍스트를 생성"**하는 데 초점이 맞춰져 있습니다. 즉, AI 모델의 종류에 구애받지 않고 최적화된 프롬프트를 만들어주는 전처리 시스템으로 설계되었습니다. docs/SYSTEM_OVERVIEW.md 와 docs/INTEGRATION_GUIDE.md 에서는 생성된 프롬프트를 gemini, claude 같은 외부 AI CLI 도구에 전달하여 사용하는 워크플로우를 명시하고 있습니다.

3. gemini-cli, Claude 등과의 연동 계획 수준
문서에 따르면, gemini-cli, claude-code 등과의 연동은 명령줄 명령(Command-line commands) 수준으로 계획되어 있습니다.

시스템은 API를 직접 호출하는 대신, 다음과 같은 흐름으로 작동합니다:

ctxset이 최적화된 프롬프트(컨텍스트)를 생성합니다.

사용자 또는 자동화 스크립트가 이 생성된 프롬프트를 gemini나 claude 같은 외부 CLI 도구의 인자로 전달하여 실행합니다.

예를 들어, docs/SYSTEM_OVERVIEW.md 에는 다음과 같은 예시가 있습니다:

Bash

# gemini CLI 연동 예시
contextworks work STORY-001 Task-1.2 --ai-cli gemini
# → 내부적으로 gemini -p "[생성된_프롬프트] + [작업_의도]" 와 같은 명령을 실행

# Claude CLI 연동 예시
contextworks work STORY-001 Task-1.2 --ai-cli claude
# → 내부적으로 claude-code "[생성된_프롬프트] + [작업_의도]" 와 같은 명령을 실행
이는 시스템이 특정 AI 모델에 종속되지 않고 유연하게 다양한 모델을 활용할 수 있도록 하는 설계 의도로 보입니다.

4. 현재 MCP의 주요 기능 및 UI/UX 흐름
현재 구현된 MCP(Model Context Protocol)는 stdio 기반의 JSON-RPC 서버로, AI 에이전트나 다른 프로그램이 ctxset의 핵심 기능을 호출할 수 있는 인터페이스를 제공합니다.

주요 기능:

ping: 서버의 상태를 확인합니다.

classifyText: 주어진 텍스트(제목, 본문)를 분석하여 자동으로 패싯(facets)을 분류하고 신뢰도 점수를 반환합니다.

composePrompt: 패싯, Git 커밋 정보, 토큰 예산 등을 기반으로 관련 문서를 조합하여 최적의 프롬프트를 생성합니다.

listContexts: 인덱싱된 문서 목록을 페이지네이션하여 반환합니다.

getContext: 특정 문서 ID에 해당하는 상세 정보(내용, 패싯 등)를 반환합니다.

UI/UX 흐름:
MCP 자체는 UI가 없는 프로토콜이지만, 이 프로젝트는 웹 대시보드를 통해 MCP의 핵심 기능과 유사한 흐름을 사용자에게 제공합니다.

문서 분류 (/classify): 사용자가 파일을 업로드하거나 텍스트를 직접 입력하면, 시스템이 이를 분석하여 분류된 패싯과 신뢰도 점수를 보여줍니다.

결과 확인 및 활용: 사용자는 분류 결과를 확인하고, "패싯 복사", "프롬프트 조합하기", "빌더로 보내기" 등의 액션을 통해 다음 단계로 나아갈 수 있습니다.

조합 빌더 (/compose/builder): 분류된 패싯을 기반으로, 키워드, 신뢰도, 토큰 예산 등 추가 조건을 설정하여 최종 프롬프트를 생성합니다.

생성된 프롬프트 확인: 조합 결과로 생성된 프롬프트와 함께 사용된 토큰 수, 선택된 문서 목록, 선택 근거 등의 메타데이터를 확인할 수 있습니다.

5. 주요 사용 시나리오 예시
이 시스템을 사용하는 주요 시나리오는 **"개발자가 특정 작업에 필요한 최적의 컨텍스트를 AI에게 제공하여, 더 정확하고 일관된 결과물을 얻는 것"**입니다.

예시 시나리오: "Rust와 Axum을 사용한 백엔드 API 인증 기능 구현"

1단계: 컨텍스트 분류 및 조합 조건 설정

개발자는 먼저 ctxset의 웹 UI나 CLI를 사용하여 자신의 요구사항을 시스템에 전달합니다.

웹 UI (/compose/builder) 사용:

"플랫폼"에서 API를, "프로그래밍 언어"에서 Rust를 선택합니다.

"키워드 검색"에 "authentication, JWT" 등을 입력합니다.

"작업 유형"으로 구현(implement)을 선택합니다.

필요한 토큰 예산(예: 4K 토큰)과 신뢰도 임계값(예: 70%)을 설정합니다.

2단계: 시스템의 프롬프트 생성

ctxset의 **BuildComposer**가 요청을 받아 작업을 수행합니다.

**scorer**가 SQLite 인덱스에서 "rust", "axum", "authentication" 태그를 가진 문서들의 점수를 매깁니다. backend-rust-axum.md 와 같은 문서가 높은 점수를 받을 것입니다.

**selector**가 점수가 높은 문서들 중에서 MMR 알고리즘을 사용해 토큰 예산 내에서 가장 관련성 높고 다양한 문서들을 선택합니다.

**merger**가 선택된 문서들의 내용을 조합하여, 최종적으로 다음과 같은 구조의 프롬프트를 생성합니다.

3단계: 생성된 프롬프트를 AI에 전달

개발자는 생성된 프롬프트를 복사하여 자신이 사용하는 AI 모델(Gemini, Claude 등)에 입력합니다.

# Context Composition

**Repository:** local
**Branch:** main
**Commit:** latest

## Document 1: Rust Backend Stack with Axum

**Source:** `documents/backend-rust-axum.md`
**Confidence:** 0.95
**Trust:** 0.80
**Freshness:** 2025-08-27

Axum은 Rust로 작성된 현대적이고 성능이 뛰어난 웹 프레임워크입니다.
... (backend-rust-axum.md 내용) ...

---

## Document 2: (다른 관련 문서)
...

---

**Composition Summary:**
- Documents: 2
- Total Tokens: ~3800
- Generated: 2025-08-28 14:30:00 UTC
이러한 과정을 통해 AI는 단편적인 질문이 아닌, 프로젝트의 기술 스택과 요구사항이 잘 정리된 풍부한 컨텍스트를 기반으로 고품질의 코드나 답변을 생성하게 됩니다.

6. 핵심 기능 단순화 및 정확성 향상을 위한 개선점

이 MCP 중계 시스템을 더 간단하면서도 정확하게 만들기 위해 고려할 수 있는 개선사항들은 다음과 같습니다:

아키텍처 단순화 (단일 크레이트 모듈화): 기존에 ctx-core와 ctx-cli로 나뉘어 있던 구조를 하나의 크레이트로 통합하는 방안을 이미 검토 중입니다. 통합된 단일 크레이트 내에서 cli, core, contexts, ai 등 모듈로 구분하면 순환 의존성 제거와 빌드 시간 단축, 배포 단순화 등의 효과가 있습니다
GitHub
. 실제로 최신 설계에서는 통합 모듈형 아키텍처를 채택하여 하나의 바이너리로 모든 기능을 제공하도록 방향을 잡았습니다.

명확한 모듈 경계 및 관심사 분리: 현재 시스템은 레이어별 역할 분담이 잘 되어 있습니다(예: 문서 파싱 doc, 분류 로직 classifier, 데이터 저장 storage 등). 앞으로도 이러한 관심사 분리 원칙을 철저히 지켜, 각 모듈이 핵심 역할만 담당하도록 유지해야 합니다
GitHub
. 예를 들어, 분류 로직은 온톨로지/룰 엔진에만 집중하고, 데이터 접근은 index/storage 계층에만 맡기는 식입니다. 이를 통해 핵심 기능이 불필요하게 비대해지는 것을 막고 유지보수를 단순화할 수 있습니다.

룰 기반 분류 로직의 정교화 및 간소화: 현재는 YAML 온톨로지와 규칙 기반으로 문서를 분류하고 있습니다. 이 접근은 결정적이고 투명하지만, 룰셋이 복잡해질 경우 관리 부담이 늘 수 있습니다. 개선책으로는 온톨로지 사전 관리 도구나 시각화 도입, 불필요하게 중복된 규칙 통합 등이 있습니다. 또한 분류 정확도를 높이기 위해 유사도 기반 임베딩 검색을 보조적으로 활용하는 방안도 고려해볼 수 있습니다. 예를 들어, 규칙 기반 분류로 1차 필터링을 하고, 임베딩을 활용한 벡터 검색으로 미처 규칙에 잡히지 않은 관련 문서를 찾는 식입니다. 다만 이 경우 시스템이 복잡해질 수 있으므로, 옵션 기능으로 두어 필요할 때만 사용하거나, 사전에 벡터 인덱스를 구축해 성능 저하를 최소화하는 것이 좋습니다.

결정성(Determinism) 강화 및 캐싱: 동일한 입력에 항상 동일한 출력이 나오도록 하는 결정성은 이 시스템의 중요한 특징입니다. 이를 더욱 견고하게 유지하기 위해 입력 정규화 로직과 해싱(determinism 모듈)을 철저히 검증하고, 가능하면 단위 테스트를 통해 회귀 없이 항상 동일 동작을 보장해야 합니다. 추가로, 이미 설계에 언급된 캐시 계층을 적극 활용하는 것이 권장됩니다
GitHub
. 예를 들어, 한 번 분류/조합한 문서에 대해서는 결과를 디스크나 메모리에 캐시하여 반복 호출 시 즉시 반환함으로써 성능을 향상시키고 불필요한 재연산을 줄일 수 있습니다.

토큰 예산 관리 및 프롬프트 품질 향상: 프롬프트 조합기의 알고리즘(MMR 등)이 토큰 예산 내 최적 문서를 선정하는데, 향후 토큰 카운팅의 정확성을 높이고 AI 모델의 컨텍스트 창 한계를 넘지 않도록 하는 것이 중요합니다. OpenAI 등의 토큰 계산 라이브러리를 활용하거나, 현재 사용하는 토크나이저에 기반해 정확한 토큰 수 추산 기능을 통합하세요. 이를 통해 선택된 문서들이 전체 토큰 한도 내에 들어오면서도 최대한 많은 핵심 정보를 전달하도록 프롬프트를 구성할 수 있습니다. 또한 프롬프트 템플릿(예: 현재 PromptBuilder의 work_prompt 템플릿)을 지속적으로 다듬어, 명확한 섹션 구분, 일관된 스타일, AI 모델이 이해하기 쉬운 형식을 유지하도록 개선합니다.

외부 도구 연동 에러 처리 및 로깅: AI 도구(예: gemini, Claude)를 CLI로 호출하거나 MCP로 통신할 때 발생할 수 있는 에러 상황에 대비하여 에러 처리 로직을 강화해야 합니다. 이미 CLI 실행 결과의 exit_code와 stderr를 잡아주는 구조가 있지만
GitHub
GitHub
, 여기서 한 발 더 나아가 사용자 친화적인 오류 메시지, 재시도 로직, 폴백(fallback) 전략을 확충할 수 있습니다. 예를 들어, Gemini CLI 호출이 실패하면 자동으로 Claude CLI로 대체하거나
GitHub
, MCP 서버 연결이 끊어지면 재연결을 시도하는 등의 방안입니다. 이러한 탄탄한 예외 처리는 시스템을 더 신뢰성 있고 정확하게 만들어 줄 것입니다.

지속적인 테스트와 검증: 핵심 기능들이 단순하고 정확하게 동작하는지를 보장하려면 꾸준한 테스트가 필수입니다. 유닛 테스트와 통합 테스트 (/tests 디렉토리 활용)를 통해 분류 정확도, 문서 인덱싱 성능, 프롬프트 조합의 완성도를 검증하세요. 또한 예시 시나리오(예: Rust/Axum 인증 기능 구현 사례 등)를 다양한 변주로 테스트하여, 다양한 상황에서도 일관된 품질의 프롬프트가 생성되는지 확인해야 합니다. 필요하다면 실제 AI 모델을 통합한 엔드투엔드 테스트(예: 생성된 프롬프트를 실제 Claude API에 넣어보기 등)도 고려하여, AI 결과물까지 기대대로 나오는지를 검증하면 좋습니다.

7. AI 연동 및 MCP 통합 전략, 향후 개발 계획

앞으로 이 프로젝트를 더 발전시키고자 한다면, AI 연동 방식과 MCP 통합을 중심으로 몇 가지 전략 및 개발 계획을 세울 수 있습니다:

다양한 AI 모델 연동을 위한 유연한 구조: 현재 설계에는 Gemini, Claude 같은 외부 AI CLI를 통해 프롬프트를 전달하도록 되어 있으며, MCP 및 직접 API 호출 옵션까지 고려되고 있습니다
GitHub
GitHub
. 이처럼 모델에 종속되지 않는 유연성을 지속 유지하는 것이 중요합니다. 구체적으로, AiCliExecutor의 available_clis 설정을 환경설정 파일(예: ~/.contextworks/config.toml)로 분리하여 사용자가 사용하는 AI 도구의 경로나 명령어를 손쉽게 지정하도록 개선할 수 있습니다. 또한 새로운 AI 도구가 등장할 경우 코드 수정 없이 설정 추가만으로 연동될 수 있게 플러그인 방식이나 동적 로딩 구조를 도입하면 확장성이 높아집니다.

MCP 서버 완성도 향상: MCP를 통한 AI 연동은 Claude Desktop 등의 플랫폼과 깊은 통합을 가능케 합니다. 이를 위해 MCP 서버(contextworks mcp 커맨드)가 MCP 표준을 완벽히 준수하도록 만드는 것이 중요합니다. 예를 들어, JSON-RPC handshake 단계에서 서버가 제공하는 툴 목록과 **리소스(Resource)**를 명시적으로 광고하고, Claude와 같은 클라이언트 측에서 이 정보를 사용할 수 있게 해야 합니다. 현재 MCP 서버에서 제공하는 기능(classifyText, composePrompt 등)은 Claude Desktop 입장에선 각각 하나의 “툴”로 간주될 수 있으므로, 툴 명세서(name, params, description)를 명확히 정의하세요. Claude Desktop의 MCP 설정 파일에 이 서버를 추가할 때, 사용자에게 이해하기 쉬운 이름과 아이콘을 지정하면 UX도 향상됩니다. 또한 STDIO 기반 통신의 안정성도 중요한데, 서버 프로세스의 에러로 스트림이 끊어지지 않도록 예외 처리에 유의하고, 필요한 경우 Heartbeat(ping) 응답이나 진행 상황(progress) Notification도 구현하여 클라이언트가 안정적으로 사용할 수 있게 합니다.

gemini-cli 및 Claude-cli 연동 간소화: CLI를 통해 AI를 호출하는 모드의 경우, 내부적으로 std::process::Command를 사용해 외부 프로세스를 실행하게 됩니다
GitHub
. 이때 OS 별 경로 문제나 권한 이슈가 없도록 사전 설정을 안내하고, 출력 스트림 파싱을 통해 AI의 응답을 곧바로 사용자에게 보여주는 기능을 추가할 수 있습니다. 예를 들어, 현재는 단순히 AI CLI를 실행만 하지만, 개선된 버전에서는 AI 응답을 캡처하여 CLI 모드에서도 결과를 콘솔에 출력하거나 파일로 저장해주는 것입니다. 이를 통해 사용자는 한 곳에서 컨텍스트 생성부터 AI 답변 획득까지 일련의 흐름을 완료할 수 있어 편의성이 높아집니다.

직접 API 연동 (클라우드 AI): 향후 OpenAI나 Anthropic API를 직접 호출하는 기능(--ai-cli api)도 고려되어 있는데
GitHub
, 이를 구현할 때는 API 키 관리와 요금 문제 등을 염두에 두어야 합니다. 예를 들어, config에 API 키를 안전하게 저장하고 불러오는 로직, 그리고 API 호출 실패 시 재시도나 대체 CLI로 폴백하는 로직이 필요합니다. 직접 API 호출은 속도 면에서 CLI 호출보다 빠를 수 있으므로, 특정 상황(예: CI 파이프라인에서 자동으로 문서 생성)에선 API 모드를 사용하고, 사용자가 상호작용하는 상황에선 CLI/MCP 모드를 사용하는 식으로 시나리오별 최적 모드 선택 기능을 제공하면 좋습니다.

확장 기능 개발 (IDE 플러그인, 웹 UI 등): 현재 Axum 기반 웹 대시보드가 기본적인 기능을 제공하고 있으나, 장기적으로는 IDE 통합이나 팀 협업 측면의 확장이 유용합니다
GitHub
. 예를 들어 VS Code 확장이나 JetBrains 플러그인을 개발하여, IDE 내에서 개발자가 바로 contextworks의 분류/프롬프트 조합 기능을 쓸 수 있게 할 수 있습니다. MCP를 활용하면 이러한 IDE 통합이 비교적 수월하며, 로컬 MCP 서버를 IDE 플러그인이 클라이언트로서 호출하도록 구현하면 됩니다. 또 하나의 방향은 웹 인터페이스 고도화로, 현재 계획된 Web UI를 더욱 발전시켜 문서 업로드부터 AI 응답 확인까지 엔드투엔드 경험을 제공하는 것입니다. 팀 협업의 경우, 여러 개발자가 공유하는 문맥을 관리하기 위해 원격 서버 모드나 클라우드 서비스를 고려할 수 있고, 권한 관리나 변경 이력을 시각화하는 기능도 추가 가능할 것입니다
GitHub
.

프로젝트 발전 로드맵 수립: 이상의 개선점을 토대로 단기/중장기 로드맵을 설정하면 개발 우선순위를 정하기 쉽습니다. 단기적으로는, 현재 구현된 MCP 서버와 CLI 연동의 버그를 잡고 안정화하는데 집중하세요. 예컨대 예외 처리를 보강하고, 다양한 OS 환경에서 테스트하여 호환성을 높입니다. 중기적으로는, IDE 플러그인이나 웹 UI 통합처럼 사용성이 드러나는 기능을 개발함과 동시에, 분류 정확도 향상(온톨로지 튜닝, 임베딩 도입 실험)을 병행합니다. 이 시기에는 사용자의 피드백을 받아 UI/UX를 개선하는 것도 중요합니다. 장기적으로는, 팀/기업 단위로 쓸 수 있는 컨텍스트 협업 플랫폼으로 발전시키는 비전을 가져갈 수 있습니다. 예를 들어, 사내 문서를 자동으로 인덱싱하여 AI 컨텍스트로 활용하거나, 프로젝트별로 AI 전문가 프로파일을 커스터마이징할 수 있게 하는 등입니다. 이러한 방향으로 나아가기 위해 오픈소스로 커뮤니티 피드백을 받거나, 사내 POC를 통해 실제 현업 적용 결과를 점진적으로 축적하면, 궁극적으로 심플하면서도 강력한 MCP 중계 도구로 완성될 수 있을 것입니다.

좋아요. 지금 코드베이스(6계층, SQLite 인덱스, MMR 조합, Axum/CLI/MCP 연동 계획)를 **더 단순하고, 더 정확하게** 만드는 방향으로 “바로 적용 가능한” 설계·개발 방안을 정리했습니다.
아래 내용은 **즉시 리팩터링에 착수할 수 있는 수준**으로, 모듈 경계·인터페이스·데이터 모델·에러/로그·테스트·로드맵까지 포함합니다.

---

# 목표와 원칙 (한 문장 요약)

* **하나의 바이너리**가 **세 가지 모드**(CLI / HTTP / MCP stdio)를 노출하고, **단일 프롬프트 파이프라인**을 중심으로 **결정성**(Determinism)·**토큰 예산 준수**·**신뢰도 임계값**을 보장한다. 외부 AI는 **Transport 플러그인**(CLI, API, MCP)으로 느슨하게 결합한다.

---

# 제안 1) 구조 간소화 — “단일 바이너리 · 다중 모드”

## 1-1. 크레이트/모듈 재구성

> 현재 `ctx-core`/`ctx-cli` 2 크레이트 → **단일 크레이트**로 합치되, 내부 모듈로 관심사 분리

```
/src
  /app           # 엔트리포인트(모드 스위처)
  /pipeline      # 프롬프트 파이프라인(분류 → 검색&스코어링 → MMR → 트리밍 → 템플릿)
  /domain        # 도메인 모델(문서, 패싯, 점수, 설정), 에러
  /drivers
    /storage     # SQLite 인덱스, 파일 스토리지, 캐시
    /http        # Axum 라우트(선택)
    /mcp         # MCP stdio 서버
    /ai          # Transport(플러그인): gemini-cli, claude-cli, openai, anthropic...
  /services      # 유즈케이스(파사드): classify_text, compose_prompt, work(=compose+send)
  /util          # 토큰 카운팅, 해싱(결정성), 공통 헬퍼
```

### Cargo features

* `features = ["http","mcp","openai","anthropic","embeddings"]`

  * 배포 타깃/용도별 최소 빌드(컴팩트) 가능
  * 기본은 `["mcp"]`만 켜고, 필요 시 확장

---

# 제안 2) “단일 파이프라인” 명세 (구체)

## 2-1. 파이프라인 단계

1. **Normalize & Hash**: 입력 정규화(공백/마크다운 블록/헤더 정리) → `DeterministicKey` 생성
2. **Classify**: YAML 온톨로지+룰 → 주요 패싯/신뢰도 산출
3. **Retrieve & Score**: SQLite 인덱스에서 후보 수집 → 점수화(가중합 공식)
4. **MMR Select**: 관련성+다양성 균형(λ)으로 선택
5. **Token Trim**: 예산 초과 시 문서 단락 단위로 자르기(“핵심/근거/제약” 섹션 우선)
6. **Template**: 표준 프롬프트 템플릿에 주입(메타·출처·토큰사용량 포함)
7. **(옵션) Send**: Transport에 전달(“work” 유즈케이스에서만)

## 2-2. 점수화(Scoring) — **가중합 공식**

```
final_score(d) = w_rel*rel(d,q) + w_trust*trust(d) + w_fresh*fresh(d, now) + w_conf*confidence(d)
```

* `rel(d,q)` : 키워드/패싯 일치도(룰 기반, 필요 시 BM25/TF-IDF)
* `trust(d)` : 문서 출처/승인 레벨(인증된 내부 위키 > 외부 블로그)
* `fresh(d)` : 날짜 감쇠(예: 180일 half-life)
* `confidence(d)` : 분류 신뢰도

## 2-3. MMR 선택 (λ=0.7 권장)

```
S ← ∅
while |S| < k:
  pick argmax_{d∈R\S} [ λ*rel(d,q) - (1-λ)*max_{s∈S} sim(d,s) ]
```

* `sim(d,s)`는 임베딩(옵션 features=`embeddings`) 또는 키워드 자카드 유사도
* `k`는 토큰 예산 전 탐욕적으로 증가 → 2-4의 트리밍 단계가 최종 예산을 보장

## 2-4. 토큰 예산 트리밍

* 단락(헤딩·리스트·코드블록) 단위로 잘라내며, **근거/제약/API 계약** 섹션은 마지막까지 보존
* `count_tokens(str) → usize` 제공(예: `tokenizers` 활용)
* **Fail-fast**: 예산 초과 시 경고와 함께 조합 요약(“근거 요약”) 자동 삽입

## 2-5. 표준 템플릿 (핵심 섹션 고정)

* `# Objective` (유저 요청/의도, 제약)
* `# Context` (선택 문서 요약+출처+날짜+신뢰도)
* `# Rules` (불변 규칙: 언어/스타일/인용/금칙어)
* `# Deliverable` (정확한 출력 포맷·검증 기준)
* `# Safety/Edge cases`
* `# Appendix` (원문 일부/링크)

---

# 제안 3) Transport 플러그인 (AI 연동을 “더 간단하고 완벽하게”)

## 3-1. 공통 인터페이스

```rust
// src/drivers/ai/mod.rs
#[derive(Clone)]
pub struct AiPrompt { pub system: String, pub user: String, pub max_tokens: usize }

pub struct AiResponse { pub text: String, pub usage_tokens: usize }

#[async_trait::async_trait]
pub trait AiTransport: Send + Sync {
    async fn send(&self, p: &AiPrompt) -> Result<AiResponse, AiError>;
    fn name(&self) -> &'static str;
}
```

## 3-2. 구현체

* **CliTransport**: `gemini`, `claude-code` 등 바이너리 호출

  * `Command::new("gemini").args(["-p", prompt])...`
  * stdout 파싱/타임아웃/exit code 해석(표준화된 `AiError`)
* **ApiTransport**(옵션): OpenAI/Anthropic API

  * 키 로딩: `~/.ctx/config.toml` 또는 ENV
  * 재시도/지수백오프/레이트 리미트
* **McpTransport**(옵션): 다른 MCP provider에 “tool call”로 전달

## 3-3. 폴백 체인 (순서 기반)

```rust
pub struct AiChain { pub order: Vec<Arc<dyn AiTransport>> }
impl AiChain {
  pub async fn try_send(&self, p: &AiPrompt) -> Result<AiResponse, AiError> {
    let mut last = None;
    for t in &self.order {
      match t.send(p).await { Ok(r)=>return Ok(r), Err(e)=>{last=Some(e)} }
    }
    Err(last.unwrap_or_else(|| AiError::NoTransport))
  }
}
```

---

# 제안 4) MCP 서버 — **툴 명세/에러 규약/진행표시 포함**

## 4-1. 노출 툴(Methods)

* `ctx.ping()`
* `ctx.classifyText({title, body}) -> facets[] + scores`
* `ctx.composePrompt({goal, selectors, budget, min_conf}) -> {prompt, meta}`
* `ctx.work({goal, selectors, budget, ai}) -> {prompt, ai_reply, meta}`
* `ctx.listContexts({page,size,filters})` / `ctx.getContext({id})`
* `ctx.indexFiles({paths[]}) -> {added, skipped, errors[]}`

> **원클릭 사용성**을 위해 **`ctx.work`** 추가: 조합→전달→응답까지 한번에.

## 4-2. JSON-RPC 예시

**request**

```json
{"jsonrpc":"2.0","id":"42","method":"ctx.work",
 "params":{"goal":"Rust Axum 인증 미들웨어 구현",
           "selectors":{"lang":["rust"],"topic":["auth","jwt"]},
           "budget":4000,"ai":"claude"}}
```

**result**

```json
{"jsonrpc":"2.0","id":"42","result":{
  "prompt":"# Objective ...",
  "ai_reply":"여기에 코드/설명...",
  "meta":{"tokens":{"prompt":3820,"reply":950},
          "docs":[{"id":"backend-rust-axum.md","confidence":0.92}],
          "transport":"claude-cli"}
}}
```

## 4-3. 진행/로그/에러 규약

* **Progress Notification(선택)**: `ctx.progress` 0\~100 단계 이벤트
* **에러 스키마 통일**

```json
{"code":"E_AI_TIMEOUT","message":"gemini timed out","data":{"transport":"gemini","timeout_ms":30000}}
```

* 공통 코드: `E_AI_TIMEOUT`, `E_AI_EXEC`, `E_BUDGET_EXCEEDED`, `E_INDEX_IO`, `E_PARSE`

---

# 제안 5) 저장소/인덱스/캐시 — **정확성과 성능을 동시에**

## 5-1. SQLite 스키마(핵심)

```sql
CREATE TABLE docs(
  id TEXT PRIMARY KEY,
  title TEXT, path TEXT, source TEXT, created_at TEXT, updated_at TEXT,
  facets JSON, trust REAL, confidence REAL
);
CREATE VIRTUAL TABLE docs_fts USING fts5(content, content_rowid='rowid');
CREATE TABLE idx_meta(key TEXT PRIMARY KEY, value TEXT);

-- 검색용 View: 점수 계산이 필요하면 앱단에서 weights 적용
```

## 5-2. 캐시

* 키: `DeterministicKey(goal + selectors + budget + ontology_version)`
* 값: `PromptBundle { prompt, docs, tokens, ts }`
* 정책: LRU + TTL(기본 24h) / **버전 핀**(ontology/rules 변경 시 무효화)

---

# 제안 6) 결정성(Determinism) — **입력 정규화 & 해시 고정**

```rust
pub fn normalize(s: &str) -> String {
  // 헤딩 canonicalize, 공백/코드블록 정리, 마크다운 링크 표준화 등
}
pub fn det_key(input: &ComposeInput, ontology_ver: &str) -> String {
  use blake3::Hasher;
  let mut h = Hasher::new();
  h.update(normalize(&serde_json::to_string(input).unwrap()).as_bytes());
  h.update(ontology_ver.as_bytes());
  h.finalize().to_hex().to_string()
}
```

* 동일 입력 → 동일 `det_key` → 동일 캐시 히트/동일 선택 순서
* 랜덤요소 금지(정렬·동점 처리 규칙 고정)

---

# 제안 7) CLI/HTTP UX — **한 명령으로 끝**

## 7-1. CLI 명령

```bash
# 조합만
ctx compose --goal "Axum JWT 미들웨어" \
  --lang rust --topic auth jwt --budget 4000 --min-conf 0.7 \
  --out prompt.md

# 한 방에 실행(폴백 체인 적용)
ctx work --goal "Axum JWT 미들웨어" --selectors lang=rust topic=auth,jwt \
  --budget 4000 --ai-order "claude,gemini,api" --show-usage
```

## 7-2. HTTP (선택)

* `POST /v1/compose` / `POST /v1/work`
* 응답에 `meta.tokens`/`meta.docs`/`transport` 포함

---

# 제안 8) 설정 파일 (TOML)

```toml
# ~/.ctx/config.toml
[defaults]
budget = 4000
min_confidence = 0.7
mmr_lambda = 0.7

[paths]
index_dir = "~/.ctx/index"
cache_dir = "~/.ctx/cache"

[ai]
order = ["claude-cli","gemini-cli","openai"]

[ai.claude-cli]
cmd = "claude"
args = ["-p","{prompt}"]
timeout_ms = 30000

[ai.gemini-cli]
cmd = "gemini"
args = ["-p","{prompt}"]
timeout_ms = 30000

[ai.openai]        # features=["openai"] 일 때만
api_key_env = "OPENAI_API_KEY"
model = "gpt-4o-mini"
timeout_ms = 20000
```

---

# 제안 9) 에러 처리·로깅·관측(Observability)

* **에러**: `thiserror`로 도메인 에러 계층화(`AiError`, `IndexError`, `PipelineError`)
* **로깅**: `tracing` + `tracing-subscriber`

  * 기본 출력: human-readable
  * `RUST_LOG=ctx=debug,sqlx=warn`
* **구조화 로그 필드**: `request_id`, `det_key`, `budget`, `selected_docs`, `transport`, `latency_ms`, `cache_hit`
* **메트릭(선택)**: `prometheus` exporter (선택 feature)

---

# 제안 10) 테스트 전략 (구체)

## 10-1. 유닛 테스트

* 분류기: 동일 입력 → 동일 패싯·신뢰도
* 점수화: 가중치 변경에 따른 순위 변동 검증
* MMR: λ=0.0/1.0 경계 테스트, 음수/NaN 방지
* 토큰 트리밍: 예산 초과 시 섹션 우선순위 지킴

## 10-2. 통합 테스트

* **Golden tests**: 입력 YAML/문서셋 → 고정된 `prompt.md` 대비
* SQLite 인덱싱 → FTS 검색·점수화 연동
* 캐시 적중/무효화(ontology/rules 버전업) 시나리오

## 10-3. E2E (옵션)

* `ctx work` → MockTransport로 응답 캡처
* 실제 CLI(claude/gemini) 연결은 **opt-in** 플래그로 별도 러너

---

# 제안 11) 보안·신뢰성

* **경로 화이트리스트**: 인덱싱 가능 디렉터리 제한
* **비밀키 관리**: ENV 또는 OS Keychain 연동(가능 시)
* **비정상 입력 방어**: JSON-RPC 사이즈 제한, 타임아웃, 동시 실행 제한(세마포어)
* **결함 격리**: Transport 타임아웃 시 폴백, 인덱스 손상 시 자동 재구축 경고

---

# 제안 12) 선택적 임베딩(벡터) 경로 (뒤늦게도 쉽게 추가되게)

* features=`embeddings` 인 경우만 활성화
* `drivers/storage/emb.rs`에 HNSW(외부 엔진 or 라이브러리) 어댑터
* 파이프라인 3단계에서 **룰 기반 후보 + 벡터 근접 후보**를 **합집합**으로 구성 후 점수화

---

# 제안 13) 마이그레이션 계획 (실행 순서)

1. **모듈 재배치**: `/pipeline`, `/drivers`, `/services`, `/domain` 생성 → 기존 코드 이동
2. **Transport 인터페이스 도입**: `CliTransport` 부터 적용, 기존 CLI 호출부 치환
3. **파이프라인 고정화**: 단계별 함수로 쪼개고 시그니처/에러 통일
4. **MCP 툴셋 정리**: `ctx.work` 추가, 에러 코드/프로그래스 이벤트 반영
5. **토큰 카운팅/트리밍 정확화**: 템플릿 확정, Golden tests 작성
6. **캐시 레이어 도입**: det\_key·버전핀·TTL 적용
7. **설정 파일 도입**: TOML 로드 + ENV 오버라이드
8. (옵션) **API Transport** 추가, rate-limit/재시도
9. (옵션) **HTTP 모드**/웹대시보드 연동 정리

각 단계 완료 시 **가시적 산출물**: `ctx work` 데모, Golden test 통과 보고, 구조화 로그 캡처.

---

# 제안 14) 샘플 코드 스니펫

## 14-1. 파이프라인 파사드

```rust
// src/services/work.rs
pub struct WorkInput {
  pub goal: String,
  pub selectors: Selectors, // { lang:[], topic:[], ... }
  pub budget: usize,
  pub min_conf: f32
}

pub struct WorkOutput {
  pub prompt: String,
  pub ai_reply: Option<String>,
  pub meta: Meta
}

pub async fn work(i: WorkInput, chain: &AiChain, cfg: &Cfg) -> Result<WorkOutput, Error> {
  let det = det_key(&i, cfg.ontology.version.as_str());
  if let Some(hit) = Cache::get(&det)? { return Ok(hit.into()); }

  let facets = classify(&i.goal, &i.selectors, &cfg.knowledge).await?;
  let cand   = retrieve_score(&facets, &cfg.index).await?;
  let picked = mmr_select(cand, cfg.mmr_lambda, i.budget, &cfg)?;
  let prompt = render_prompt(&i, &picked, &cfg)?;
  let out = if cfg.work_send {
      let ai = chain.try_send(&AiPrompt{ system: cfg.tpl.system(), user: prompt.clone(), max_tokens: cfg.reply_max }).await?;
      WorkOutput { prompt, ai_reply: Some(ai.text), meta: Meta::from(&picked, &ai) }
  } else {
      WorkOutput { prompt, ai_reply: None, meta: Meta::from(&picked, &None::<AiResponse>) }
  };
  Cache::put(&det, &out)?;
  Ok(out)
}
```

## 14-2. MCP 핸들러(요지)

```rust
// src/drivers/mcp/server.rs
loop {
  let req = read_jsonrpc(stdin).await?;
  match req.method.as_str() {
    "ctx.ping" => reply_ok(req.id, json!({"ok":true})).await?,
    "ctx.composePrompt" => { /* WorkInput with work_send=false */ }
    "ctx.work" => { /* WorkInput with work_send=true, ai from params */ }
    _ => reply_err(req.id, "E_METHOD", "unknown method", None).await?,
  }
}
```

---

# 제안 15) 품질 게이트 (Done의 정의)

* Golden tests 20+ 케이스 통과 (언어/도메인/예산/임계값 조합)
* 같은 입력에서 **동일 det\_key, 동일 문서 순서, 동일 prompt 바이트스트림**
* 예산 초과 없음(±1% 허용), 실패 시 `E_BUDGET_EXCEEDED`
* `ctx work` 경로에서 **최소 1개 Transport 성공 + 폴백 검증**
* MCP 툴 6종 동작, 에러 코드/프로그레스 이벤트 수신 확인
* 구조화 로그 필드 채움률 ≥ 95%

---

## 마무리 — 핵심 포인트만 다시

* **단일 파이프라인**에 모든 경로(CLI/HTTP/MCP)를 태우고, **Transport 플러그인**으로 AI 연동을 분리
* **결정성/토큰예산/신뢰도 임계값**을 코드 레벨에서 강제
* \*\*MCP `ctx.work`\*\*로 “조합→전달→응답” 원샷 사용성 제공
* **캐시+버전핀**으로 빠르고 안정적인 반복 실행
* **Golden tests**로 프롬프트 품질을 회귀 없이 고정
