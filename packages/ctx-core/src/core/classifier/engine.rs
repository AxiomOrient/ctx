use super::{facet::Classification, normalizer::Normalizer};
use crate::common::constants::scoring::{
    CLASSIFIER_KEYWORD_EVIDENCE_WEIGHT, CLASSIFIER_LINK_HOST_EVIDENCE_WEIGHT,
    CLASSIFIER_REGEX_MIN_WEIGHT, CLASSIFIER_MIN_CONFIDENCE, CLASSIFIER_MAX_CONFIDENCE,
    CLASSIFIER_CONFIDENCE_SATURATION_K,
};
use crate::knowledge::{
    ontology::OntologyRegistry,
    rules::schema::{RuleSet, YamlRuleSet},
};
use regex::Regex;

/// 컴파일된 정규식 규칙
#[derive(Debug)]
struct CompiledRegexRule {
    namespace: String,
    value: String,
    regex: Regex,
    weight: f32,
}

/// 규칙 기반 분류 엔진 (정규식 미리 컴파일)
pub struct ClassifierEngine<'a> {
    ontology: &'a OntologyRegistry,
    rules: &'a RuleSet,
    compiled_regexes: Vec<CompiledRegexRule>,
    yaml_rules: Option<YamlRuleSet>,
}

impl<'a> ClassifierEngine<'a> {
    pub fn new(ontology: &'a OntologyRegistry, rules: &'a RuleSet) -> Self {
        let compiled_regexes = Self::compile_regex_rules(rules);
        Self {
            ontology,
            rules,
            compiled_regexes,
            yaml_rules: None,
        }
    }

    /// YAML 규칙과 함께 새 엔진 생성 (권장)
    pub fn new_with_yaml(
        ontology: &'a OntologyRegistry,
        rules: &'a RuleSet,
        yaml_rules: YamlRuleSet,
    ) -> Self {
        let compiled_regexes = Self::compile_yaml_regex_rules(&yaml_rules);
        Self {
            ontology,
            rules,
            compiled_regexes,
            yaml_rules: Some(yaml_rules),
        }
    }

    /// YAML 규칙에서 정규식 규칙들을 미리 컴파일 (권장)
    fn compile_yaml_regex_rules(yaml_rules: &YamlRuleSet) -> Vec<CompiledRegexRule> {
        let mut compiled = Vec::new();

        for rule in &yaml_rules.regex_rules {
            match Regex::new(&rule.pattern) {
                Ok(regex) => {
                    compiled.push(CompiledRegexRule {
                        namespace: rule.ns.clone(),
                        value: rule.value.clone(),
                        regex,
                        weight: rule.weight,
                    });
                }
                Err(e) => {
                    eprintln!("Warning: Failed to compile regex '{}': {}", rule.pattern, e);
                }
            }
        }

        compiled
    }

    /// 레거시 정규식 규칙들을 미리 컴파일 (하위 호환성)
    fn compile_regex_rules(rules: &RuleSet) -> Vec<CompiledRegexRule> {
        let mut compiled = Vec::new();

        for (namespace, patterns) in &rules.regexes {
            for pattern_with_value in patterns {
                // 마지막 '|'를 기준으로 분할 (정규식 내부의 '|'와 구분하기 위해)
                let (pattern, value) = if let Some(last_pipe_pos) = pattern_with_value.rfind('|') {
                    let pattern = &pattern_with_value[..last_pipe_pos];
                    let value = &pattern_with_value[last_pipe_pos + 1..];
                    (pattern, value)
                } else {
                    // 패턴에서 값을 추출할 수 없는 경우 기본값 사용
                    (pattern_with_value.as_str(), "unknown")
                };

                match Regex::new(pattern) {
                    Ok(regex) => {
                        compiled.push(CompiledRegexRule {
                            namespace: namespace.clone(),
                            value: value.to_string(),
                            regex,
                            weight: 1.0, // 기본 가중치
                        });
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to compile regex '{}': {}", pattern, e);
                    }
                }
            }
        }

        compiled
    }

    /// 제목/본문에서 패싯 분류 수행
    pub fn classify(&self, title: &str, body: &str) -> Classification {
        let mut c = Classification::new();
        let content = format!("{}\n\n{}", title, body);
        let content_lower = content.to_lowercase();
        let mut normalizer = Normalizer::new(self.ontology);

        // 증거 기반 신뢰도 계산을 위한 누적 점수
        let mut evidence_score: f32 = 0.0;
        // 동일 증거 중복 집계를 피하기 위한 키 셋 (ns:value)
        let mut evidence_seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        // 1) 키워드 매칭 (YAML 규칙 우선, 레거시 규칙 보조)
        if let Some(yaml_rules) = &self.yaml_rules {
            for rule in &yaml_rules.keyword_rules {
                for keyword in &rule.keywords {
                    let needle = keyword.to_lowercase();
                    if content_lower.contains(&needle) {
                        let canon = normalizer.normalize_facet(&rule.ns, &rule.value);
                        let key = format!("{}:{}", rule.ns, canon);
                        if evidence_seen.insert(key) {
                            c.add_facet(rule.ns.clone(), canon);
                            // 키워드 매칭은 보통 약한 신호로 가정
                            evidence_score += CLASSIFIER_KEYWORD_EVIDENCE_WEIGHT;
                        }
                    }
                }
            }
        } else {
            // 레거시 키워드 매칭
            for (ns, values) in &self.rules.keywords {
                for v in values {
                    let needle = v.to_lowercase();
                    if content_lower.contains(&needle) {
                        let canon = normalizer.normalize_facet(ns, v);
                        let key = format!("{}:{}", ns, canon);
                        if evidence_seen.insert(key) {
                            c.add_facet(ns.clone(), canon);
                            evidence_score += CLASSIFIER_KEYWORD_EVIDENCE_WEIGHT;
                        }
                    }
                }
            }
        }

        // 2) 정규식 매칭 (미리 컴파일된 정규식 사용, 가중치 적용)
        let mut weighted_matches = Vec::new();
        for compiled_rule in &self.compiled_regexes {
            if compiled_rule.regex.is_match(&content) {
                let canon =
                    normalizer.normalize_facet(&compiled_rule.namespace, &compiled_rule.value);
                weighted_matches.push((
                    compiled_rule.namespace.clone(),
                    canon,
                    compiled_rule.weight,
                ));
            }
        }

        // 가중치가 높은 매치를 우선적으로 추가
        weighted_matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        for (namespace, value, weight) in weighted_matches {
            let key = format!("{}:{}", namespace, value);
            if evidence_seen.insert(key) {
                c.add_facet(namespace, value);
                // 정규식 매칭은 가중치 기반 신호
                evidence_score += weight.max(CLASSIFIER_REGEX_MIN_WEIGHT);
            }
        }

        // 3) 링크 호스트 매칭 (YAML 규칙 우선, 레거시 규칙 보조)
        if let Some(yaml_rules) = &self.yaml_rules {
            for rule in &yaml_rules.link_rules {
                for host in &rule.host_contains {
                    if content.contains(host) {
                        let canon = normalizer.normalize_facet(&rule.ns, &rule.value);
                        let key = format!("{}:{}", rule.ns, canon);
                        if evidence_seen.insert(key) {
                            c.add_facet(rule.ns.clone(), canon);
                            // 링크 호스트는 중간 강도의 신호로 가정
                            evidence_score += CLASSIFIER_LINK_HOST_EVIDENCE_WEIGHT;
                        }
                    }
                }
            }
        } else {
            // 레거시 링크 호스트 매칭
            for (ns, hosts) in &self.rules.link_hosts {
                for h in hosts {
                    if content.contains(h) {
                        let key = format!("{}:{}", ns, h);
                        if evidence_seen.insert(key) {
                            c.add_facet(ns.clone(), h.clone());
                            evidence_score += CLASSIFIER_LINK_HOST_EVIDENCE_WEIGHT;
                        }
                    }
                }
            }
        }

        // 4) 정합성 검사 — 경고만 추가 (자동 수정은 검증/수정 파이프라인에서 수행)
        for r in &self.rules.prohibited {
            if c.has_facet(&r.a_ns, &r.a_val) && c.has_facet(&r.b_ns, &r.b_val) {
                c.add_warning(format!(
                    "prohibited: {}:{} with {}:{} — {}",
                    r.a_ns, r.a_val, r.b_ns, r.b_val, r.reason
                ));
            }
        }
        for r in &self.rules.requires {
            if c.has_facet(&r.a_ns, &r.a_val) && !c.has_facet(&r.b_ns, &r.b_val) {
                c.add_warning(format!(
                    "requires: {}:{} requires {}:{} — {}",
                    r.a_ns, r.a_val, r.b_ns, r.b_val, r.reason
                ));
            }
        }

        // 증거 기반 신뢰도 추정: 포화 함수 사용 (1 - e^{-k * score})
        let saturated = 1.0 - (-CLASSIFIER_CONFIDENCE_SATURATION_K * evidence_score).exp();
        let confidence = (CLASSIFIER_MIN_CONFIDENCE + (CLASSIFIER_MAX_CONFIDENCE - CLASSIFIER_MIN_CONFIDENCE) * saturated)
            .clamp(CLASSIFIER_MIN_CONFIDENCE, CLASSIFIER_MAX_CONFIDENCE);
        c.set_confidence(confidence);
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::constants::scoring::{CLASSIFIER_MIN_CONFIDENCE, CLASSIFIER_MAX_CONFIDENCE};
    use crate::knowledge::ontology::schema::Ontology;
    use crate::knowledge::{ontology::OntologyRegistry, rules::schema::RuleSet};
    use std::collections::HashMap;

    #[allow(clippy::unwrap_used)]
    fn create_test_ontology() -> OntologyRegistry {
        let ontology = Ontology {
            namespaces: HashMap::new(),
        };
        OntologyRegistry::from_ontology(ontology).unwrap()
    }

    fn create_test_rules() -> RuleSet {
        let mut rules = RuleSet::new();

        // 키워드 규칙 추가
        rules.keywords.insert(
            "framework".to_string(),
            vec!["React".to_string(), "Vue".to_string()],
        );
        rules.keywords.insert(
            "language".to_string(),
            vec!["Rust".to_string(), "Python".to_string()],
        );

        // 정규식 규칙 추가 (pattern|value 형식)
        rules
            .regexes
            .insert("framework".to_string(), vec!["(?i)react|react".to_string()]);
        rules
            .regexes
            .insert("language".to_string(), vec!["(?i)rust|rust".to_string()]);

        rules
    }

    #[test]
    fn test_classify_with_keywords() {
        let ontology = create_test_ontology();
        let rules = create_test_rules();
        let engine = ClassifierEngine::new(&ontology, &rules);

        let result = engine.classify("React Tutorial", "This is a React application guide");

        // normalizer가 "React"를 "react"로 정규화하므로 소문자로 확인
        assert!(result.has_facet("framework", "react"));
        assert!(result.confidence() > 0.0);
    }

    #[test]
    fn test_classify_with_regex() {
        let ontology = create_test_ontology();
        let rules = create_test_rules();
        let engine = ClassifierEngine::new(&ontology, &rules);

        let result = engine.classify("Programming Guide", "This guide covers RUST programming");

        assert!(result.has_facet("language", "rust"));
    }

    #[test]
    fn test_confidence_calculation() {
        let ontology = create_test_ontology();
        let rules = create_test_rules();
        let engine = ClassifierEngine::new(&ontology, &rules);

        let result = engine.classify("Empty", "");
        assert!((result.confidence() - CLASSIFIER_MIN_CONFIDENCE).abs() < 1e-6);

        let result = engine.classify("React Rust Guide", "React and Rust programming");
        assert!(result.confidence() > CLASSIFIER_MIN_CONFIDENCE);
        assert!(result.confidence() <= CLASSIFIER_MAX_CONFIDENCE);
    }

    #[test]
    fn test_yaml_regex_compilation() {
        use crate::knowledge::rules::schema::{RegexRule, YamlRuleSet};

        let yaml_rules = YamlRuleSet {
            regex_rules: vec![RegexRule {
                ns: "framework".to_string(),
                value: "swiftui".to_string(),
                pattern: "\\bSwiftUI\\b".to_string(),
                weight: 1.2,
            }],
            keyword_rules: vec![],
            link_rules: vec![],
            codefence_rules: vec![],
        };

        let compiled = ClassifierEngine::compile_yaml_regex_rules(&yaml_rules);
        assert_eq!(compiled.len(), 1);
        assert_eq!(compiled[0].namespace, "framework");
        assert_eq!(compiled[0].value, "swiftui");
        assert_eq!(compiled[0].weight, 1.2);
    }
}
