# ctx 아키텍처 (v3.1)

**마지막 업데이트: 2025-08-29**

## 1. 개요 (Overview)

`ctx`는 AI 프롬프트 엔지니어링의 **결정론성(Determinism)**과 **재현성(Reproducibility)**을 보장하기 위해 설계된 컨텍스트 관리 및 생성 시스템입니다. 이 문서는 `ctx`의 아키텍처 원칙, 각 모듈의 상세 분석, 그리고 시스템의 전체적인 데이터 흐름을 기술합니다.

### 1.1. 목표

- **동일 입력, 동일 출력**: 동일한 문서, 규칙, 코드 버전에 대해 항상 동일한 프롬프트 결과를 생성합니다.
- **유지보수 및 확장성**: 명확한 계층과 규칙을 통해 코드의 복잡성을 관리하고 기능 확장을 용이하게 합니다.
- **다중 인터페이스 지원**: 단일 바이너리 내에서 CLI, HTTP 서버, Tauri 데스크톱 UI, MCP 등 다양한 인터페이스를 지원합니다.

### 1.2. 핵심 설계 원칙

- **계층형 아키텍처 (Layered Architecture)**: 외부 세계와의 상호작용(`drivers`), 애플리케이션의 진입점 및 UI(`app`), 핵심 비즈니스 로직(`services`, `pipeline`), 그리고 순수 데이터 모델(`domain`)을 명확히 분리합니다.
- **단방향 의존성 (Unidirectional Dependency)**: 의존성은 항상 외부 계층에서 내부 계층으로 흐릅니다 (`app` → `services` → `pipeline` → `domain`). 이 규칙은 컴파일 타임에 강제됩니다.
- **관심사 분리 (Separation of Concerns)**: 외부 기술(DB, Web, AI API)은 `drivers`에, 핵심 로직은 `pipeline`과 `domain`에 캡슐화하여 기술 변화에 유연하게 대응합니다.

## 2. 시스템 아키텍처

`ctx`은 계층형 아키텍처를 따르며, 각 모듈은 명확한 책임을 가집니다.

### 2.1. 아키텍처 다이어그램

```mermaid
graph TD
    subgraph "User Interfaces & Entry Points"
        direction LR
        CLI[CLI<br>(app/mod.rs)]
        TauriUI[Tauri UI<br>(apps/ui)]
        HTTPServer[HTTP Server<br>(drivers/http)]
        MCPServer[MCP Server<br>(drivers/mcp)]
    end

    subgraph "Application Layer"
        App[Application Host<br>(app/mod.rs, app/ui)]
        Services[Services Facade<br>(services/mod.rs)]
    end

    subgraph "Core Business Logic"
        Pipeline[7-Stage Pipeline<br>(pipeline/mod.rs)]
    end

    subgraph "Domain Model"
        Domain[Domain Objects & Rules<br>(domain/mod.rs)]
    end

    subgraph "Infrastructure & Drivers"
        direction LR
        Storage[Storage Driver<br>(drivers/storage)]
        AI_Driver[AI Driver<br>(drivers/ai)]
        Knowledge[Knowledge Loader<br>(knowledge)]
    end

    %% Flow
    CLI --> App
    TauriUI --> App
    HTTPServer --> Services
    MCPServer --> Services
    App --> Services

    Services --> Pipeline
    Services --> AI_Driver

    Pipeline --> Domain
    Pipeline --> Knowledge
    Pipeline --> Storage
```

### 2.2. 모듈별 상세 분석

| 모듈 경로 | 책임 | 핵심 분석 및 설계 패턴 | 상태 |
| --- | --- | --- | --- |
| `src/main.rs` | **최종 진입점** | `tokio::main`을 사용하여 비동기 `app::run_app`을 실행하고, 사용자 친화적인 에러 메시지를 처리하는 단일 책임만 가집니다. | **안정적** |
| `src/app` | **애플리케이션 진입 및 UI 백엔드** | **Mode Switcher**: `clap`을 이용해 CLI 인자를 파싱하여 CLI, 서버, UI 등 다양한 모드로 분기합니다. **Facade Pattern**: `services` 계층의 함수를 호출하여 UI 및 CLI의 요청을 처리합니다. **보안**: Tauri UI의 Markdown 렌더링을 백엔드에서 수행하고 `ammonia`로 살균하여 XSS를 방지합니다. | **안정적** |
| `apps/ui` | **프론트엔드 UI** | **Component-Based**: Svelte 5를 사용한 모던 SPA. **SSOT (Single Source of Truth)**: `svelte/store`를 사용해 중앙에서 상태를 관리하여 예측 가능성을 높입니다. **보안**: 백엔드에서 살균된 HTML만 렌더링하고, 파일 시스템 접근은 Tauri IPC를 통해서만 수행하여 프론트엔드 보안을 강화합니다. | **안정적** |
| `src/services` | **유스케이스 파사드** | **Facade Pattern**: `Pipeline`과 `drivers`의 복잡한 로직을 `compose_prompt`, `work` 등 단순하고 의미있는 유스케이스로 캡슐화하여 제공합니다. **설정 관리**: `config.toml`과 지식 베이스(`ontology.yaml`, `rules.yaml`)를 유연하게 로드하여 파이프라인을 설정합니다. | **안정적** |
| `src/drivers` | **외부 시스템 연동** | **Ports and Adapters**: `Storage` 트레이트(Port)와 `LocalFsStorage`(Adapter) 구현을 통해 스토리지 기술을 추상화합니다. **보안**: `LocalFsStorage`는 경로 순회 공격을 방지하는 로직을 포함합니다. **성능**: `storage/cache` 모듈은 LRU/TTL 기반의 정교한 인메모리 캐싱 시스템을 제공합니다. | **안정적** |
| `src/pipeline` | **핵심 처리 파이프라인** | **Pipeline Pattern**: 7단계(정규화, 분류, 점수화, 선택, 트리밍, 템플릿화, 전송)의 명확한 데이터 처리 흐름을 정의합니다. **Strategy Pattern**: `scorer`는 키워드, 신선도, 유사도 등 다양한 점수 계산 전략을 조합하여 사용합니다. **결정론**: MMR 알고리즘과 배낭 알고리즘을 조합하여 토큰 예산 내에서 최적의 문서 조합을 결정론적으로 선택합니다. | **안정적** |
| `src/domain` | **핵심 데이터 및 규칙** | **Rich Domain Model**: `ContextDocument`, `Rule` 등 핵심 비즈니스 개념을 타입으로 명확히 정의합니다. **Newtype Pattern**: `ContextHash`와 같은 타입을 사용하여 타입 안정성을 높이고 버그를 방지합니다. **결정론의 원천**: `constants` 모듈은 시스템의 모든 동작(가중치, 임계값 등)을 상수로 중앙 관리하여 결과의 재현성을 보장하는 핵심 역할을 합니다. | **안정적** |
| `src/knowledge` | **외부 지식 베이스** | **지식과 코드의 분리**: `ontology.yaml`과 `rules.yaml` 파일을 로드하여, 코드 변경 없이 시스템의 분류 및 추론 로직을 수정할 수 있는 유연성을 제공합니다. `ClassifierEngine`은 이 지식을 기반으로 작동합니다. | **안정적** |
| `src/doc` | **문서 파싱 및 검증** | **Parser & Validator**: 마크다운과 Frontmatter를 `ContextDocument`로 변환하고, 여러 단계의 검증(`Metadata`, `Schema`, `Structure`)을 통해 데이터의 정합성을 보장하는 파이프라인을 갖추고 있습니다. | **안정적** |
| `src/util` | **공용 유틸리티** | 결정론적 해시 생성, 파일 시스템 유틸리티, Git 연동 등 프로젝트 전반에서 사용되는 저수준 유틸리티를 제공합니다. | **안정적** |

## 3. 핵심 처리 흐름: `compose_prompt` (UI 기준)

UI에서 프롬프트를 생성하는 흐름은 아키텍처의 모든 계층을 명확하게 보여줍니다.

1.  **`apps/ui` (프론트엔드)**: 사용자가 프롬프트 입력 후 'Generate' 버튼을 클릭합니다. `RightDockPrompt.svelte` 컴포넌트가 `lib/ipc.ts`의 `compose` 함수를 호출합니다.
2.  **`ipc.ts` (IPC 래퍼)**: `invoke('compose', ...)`를 통해 Tauri의 IPC 채널로 백엔드의 `compose` 함수 호출을 요청합니다.
3.  **`app/ui/tauri_app.rs` (Tauri 핸들러)**: `#[tauri::command]`로 노출된 `compose` 함수가 요청을 받아, `app/ui/ipc.rs`의 `compose` 함수를 호출합니다.
4.  **`app/ui/ipc.rs` (UI 백엔드 로직)**: IPC 요청을 받아 `services::compose_prompt`를 호출하여 핵심 로직을 실행합니다.
5.  **`services` (파사드)**: `compose_prompt` 함수는 `Pipeline` 객체를 생성하고, 설정과 지식 베이스를 로드한 뒤 `pipeline.execute()`를 호출합니다.
6.  **`pipeline` (핵심 로직)**:
    - **Stage 1-6**: 쿼리 분류, 문서 검색, 점수 계산, MMR 선택, 병합 등 전체 컨텍스트 생성 과정을 수행합니다.
7.  **결과 반환**: 생성된 `PromptBundle`이 `services` → `app` → Tauri IPC를 거쳐 프론트엔드로 반환되고, UI가 업데이트됩니다.

이러한 흐름은 모든 의존성이 외부(UI)에서 내부(도메인)로 향하며, 각 계층이 명확한 책임을 갖는 전형적인 클린 아키텍처(Clean Architecture) 구조를 보여줍니다.
