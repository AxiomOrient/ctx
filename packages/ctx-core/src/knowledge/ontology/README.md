# src/knowledge/ontology

`ontology.yaml`을 로딩/정규화/상속(부모)/동의어 관리.

## 공개 API
- `OntologyRegistry::from_yaml(yaml_content) -> Result<Self>`: YAML 문자열에서 온톨로지 레지스트리를 생성합니다.
- `OntologyRegistry::normalize(namespace, value) -> Option<String>`: 주어진 패싯 값을 정규화된 대표 값으로 변환합니다.

## 구조
- `schema.rs`: `ontology.yaml`의 구조에 매핑되는 `Ontology`, `Namespace`, `OntoValue` 등 데이터 구조를 정의합니다.
- `mod.rs`: `OntologyRegistry`를 통해 온톨로지 데이터에 접근하고 사용하는 인터페이스를 제공합니다.

## 의존
- `common/*` 모듈만 의존합니다 (상향 의존 금지).