# src/knowledge

애플리케이션의 도메인 지식(Knowledge Base)을 관리하는 모듈입니다.

이 모듈은 외부 설정 파일(YAML)을 통해 주입되는 데이터와 로직을 구조화하고, `core` 레이어에서 사용할 수 있는 형태로 제공하는 역할을 담당합니다.

## 설정 파일 위치
설정 파일들은 패키지 내부의 `config/` 디렉토리에 위치합니다:
- `packages/ctx-core/config/ontology.yaml`: 온톨로지 정의
- `packages/ctx-core/config/rules.yaml`: 분류 규칙 정의

## 하위 모듈
- `ontology/`: `config/ontology.yaml` 파일을 파싱하여 개념, 동의어, 상속 관계를 정의하는 온톨로지 레지스트리를 생성합니다.
- `rules/`: `config/rules.yaml` 파일을 파싱하여 키워드, 정규식, 제약 조건 등 분류 및 검증에 사용되는 규칙 집합을 생성합니다.

## 사용 예시

### 기본 설정 파일에서 로드
```rust
use ctx_core::knowledge::{ontology::OntologyRegistry, rules::load_from_config_dir};

// config 디렉토리에서 자동 로드
let ontology = OntologyRegistry::from_config_dir()?;
let rules = load_from_config_dir()?;
```

### 커스텀 파일에서 로드
```rust
use ctx_core::knowledge::{ontology::OntologyRegistry, rules::load_from_file};
use std::path::Path;

// 커스텀 경로에서 로드
let ontology = OntologyRegistry::from_file(Path::new("custom/ontology.yaml"))?;
let rules = load_from_file(Path::new("custom/rules.yaml"))?;
```

### YAML 문자열에서 직접 로드
```rust
use ctx_core::knowledge::{ontology::OntologyRegistry, rules::load_yaml_rules_from_string};

let yaml_content = r#"
namespaces:
  platform:
    values:
      ios:
        synonyms: ["iphone"]
        parents: []
        deprecated: false
"#;

let ontology = OntologyRegistry::from_yaml(yaml_content)?;
```