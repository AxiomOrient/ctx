use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Ontology YAML 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ontology {
    pub namespaces: HashMap<String, Namespace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    pub values: HashMap<String, OntoValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntoValue {
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub description: Option<String>,
}

/// Ontology Registry - YAML 파일에서 온톨로지를 로드하고 정규화 기능 제공
#[derive(Debug, Clone)]
pub struct OntologyRegistry {
    pub ontology: Ontology,
}

impl OntologyRegistry {
    /// YAML 문자열에서 온톨로지 로드
    pub fn from_yaml(yaml_content: &str) -> Result<Self> {
        let ontology: Ontology = serde_yaml::from_str(yaml_content)?;
        Ok(Self { ontology })
    }

    /// 파일에서 온톨로지 로드
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(crate::ContextError::Io)?;
        Self::from_yaml(&content)
    }

    /// config 디렉토리에서 ontology.yaml 로드 (기본 설정 파일)
    pub fn from_config_dir() -> Result<Self> {
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config/ontology.yaml");
        Self::from_file(&config_path)
    }

    /// 내부 테스트/리팩토링용: 이미 구성된 Ontology에서 레지스트리 생성
    pub fn from_ontology(ontology: Ontology) -> Result<Self> {
        Ok(Self { ontology })
    }

    /// 값을 정규화 (동의어 → 표준 값)
    pub fn normalize(&self, namespace: &str, value: &str) -> Option<String> {
        let ns = self.ontology.namespaces.get(namespace)?;

        // 직접 매치
        if ns.values.contains_key(value) {
            return Some(value.to_string());
        }

        // 동의어 검색
        for (canonical, onto_value) in &ns.values {
            if onto_value.synonyms.contains(&value.to_string()) {
                return Some(canonical.clone());
            }
        }

        None
    }

    /// 네임스페이스의 모든 유효한 값들 반환
    pub fn get_valid_values(&self, namespace: &str) -> Vec<String> {
        self.ontology
            .namespaces
            .get(namespace)
            .map(|ns| ns.values.keys().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ontology_registry_from_yaml() {
        let yaml_content = r#"
namespaces:
  platform:
    values:
      ios:
        synonyms: ["iphone"]
        parents: []
        deprecated: false
      android:
        synonyms: []
        parents: []
        deprecated: false
"#;
        let registry = OntologyRegistry::from_yaml(yaml_content).unwrap();
        
        // 직접 매치 테스트
        assert_eq!(registry.normalize("platform", "ios"), Some("ios".to_string()));
        
        // 동의어 매치 테스트
        assert_eq!(registry.normalize("platform", "iphone"), Some("ios".to_string()));
        
        // 존재하지 않는 값
        assert_eq!(registry.normalize("platform", "nonexistent"), None);
        
        // 유효한 값들 조회
        let valid_values = registry.get_valid_values("platform");
        assert!(valid_values.contains(&"ios".to_string()));
        assert!(valid_values.contains(&"android".to_string()));
    }

    #[test]
    fn test_config_dir_loading() {
        // config 디렉토리에서 실제 파일 로딩 테스트
        // 파일이 존재하는 경우에만 테스트
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config/ontology.yaml");
        
        if config_path.exists() {
            let result = OntologyRegistry::from_config_dir();
            assert!(result.is_ok(), "Failed to load ontology from config dir: {:?}", result.err());
        }
    }
}
