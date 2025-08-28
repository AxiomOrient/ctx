# src/knowledge/rules

`rules.yaml` 파일에 정의된 규칙을 로드하고 적용하는 모듈입니다.

## 구조
- `schema.rs`: `rules.yaml`의 구조에 매핑되는 `YamlRuleSet`, `RegexRule`, `ProhibitedRule` 등 데이터 구조를 정의합니다.
- `loader.rs`: YAML 파일에서 규칙을 로드하는 함수를 제공합니다.
- `applier.rs`: `ClassificationLike` 트레이트를 사용하여 분류 결과에 규칙을 적용하는 로직을 구현합니다.
- `builtin.rs`: 코드에 내장된 기본 규칙 집합을 제공합니다.

## 설계 원칙
- **느슨한 결합**: `ClassificationLike` 트레이트를 통해 `core::classifier::Classification` 타입에 직접 의존하지 않고, 규칙을 적용할 수 있도록 설계되었습니다.
- **확장성**: 새로운 규칙 타입을 `schema.rs`에 추가하고 `applier.rs`에서 처리 로직을 구현하여 쉽게 확장할 수 있습니다.