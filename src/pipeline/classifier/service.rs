use super::{engine::ClassifierEngine, facet::Classification};
use crate::domain::errors::Result;
use crate::knowledge::{ontology::schema::OntologyRegistry, rules::schema::RuleSet};

/// 앱 레이어에서 knowledge를 직접 보지 않도록 하는 분류 서비스 파사드
pub struct ClassifierService {
    ontology: Option<OntologyRegistry>,
    rules: Option<RuleSet>,
}

impl ClassifierService {
    pub fn new() -> Self {
        Self {
            ontology: None,
            rules: None,
        }
    }

    pub fn set_ontology(&mut self, ontology: OntologyRegistry) {
        self.ontology = Some(ontology);
    }

    pub fn set_rules(&mut self, rules: RuleSet) {
        self.rules = Some(rules);
    }

    pub fn classify_text(&self, text: &str) -> Result<Vec<String>> {
        // 간단한 분류 구현 (나중에 실제 엔진으로 교체)
        let words: Vec<String> = text
            .split_whitespace()
            .filter(|word| word.len() > 2)
            .map(|word| word.to_lowercase())
            .collect();
        Ok(words)
    }

    pub fn classify_with_yaml(
        ontology_yaml: &str,
        _rules_yaml: Option<&str>,
        title: &str,
        body: &str,
    ) -> Result<Classification> {
        // knowledge 사용은 core 내부에서만
        let onto = crate::knowledge::ontology::OntologyRegistry::from_yaml(ontology_yaml)?;

        if let Some(ryaml) = _rules_yaml {
            if !ryaml.trim().is_empty() {
                // 새로운 YAML 형식으로 파싱하여 엔진 생성
                match serde_yaml::from_str::<crate::knowledge::rules::schema::YamlRuleSet>(ryaml) {
                    Ok(yaml_rules) => {
                        let rules = yaml_rules.to_rule_set();
                        let engine = ClassifierEngine::new_with_yaml(&onto, &rules, yaml_rules);
                        return Ok(engine.classify(title, body));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse rules.yaml: {}, using builtin rules", e);
                    }
                }
            }
        }

        // 기본 규칙 사용
        let rules = crate::knowledge::rules::builtin_rules();
        let engine = ClassifierEngine::new(&onto, &rules);
        Ok(engine.classify(title, body))
    }
}
