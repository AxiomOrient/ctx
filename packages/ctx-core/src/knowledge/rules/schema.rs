use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 분류 규칙 집합
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    /// namespace -> list of keyword rules (exact contains, case-insensitive)
    pub keywords: HashMap<String, Vec<String>>,
    /// namespace -> list of regex patterns
    pub regexes: HashMap<String, Vec<String>>,
    /// namespace -> domain-based mapping (by host contains)
    pub link_hosts: HashMap<String, Vec<String>>,
    /// cross-namespace consistency constraints (prohibit combos)
    pub prohibited: Vec<ProhibitRule>,
    /// required co-occurrence (if A then B)
    pub requires: Vec<RequireRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProhibitRule {
    pub a_ns: String,
    pub a_val: String,
    pub b_ns: String,
    pub b_val: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequireRule {
    pub a_ns: String,
    pub a_val: String,
    pub b_ns: String,
    pub b_val: String,
    pub reason: String,
}

impl RuleSet {
    pub fn new() -> Self {
        Self {
            keywords: HashMap::new(),
            regexes: HashMap::new(),
            link_hosts: HashMap::new(),
            prohibited: Vec::new(),
            requires: Vec::new(),
        }
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::new()
    }
}

/// YAML 파일에서 파싱되는 규칙 형식
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlRuleSet {
    #[serde(default)]
    pub regex_rules: Vec<RegexRule>,
    #[serde(default)]
    pub keyword_rules: Vec<KeywordRule>,
    #[serde(default)]
    pub link_rules: Vec<LinkRule>,
    #[serde(default)]
    pub codefence_rules: Vec<CodefenceRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexRule {
    pub ns: String,
    pub value: String,
    pub pattern: String,
    #[serde(default = "default_weight")]
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordRule {
    pub ns: String,
    pub value: String,
    pub keywords: Vec<String>,
    #[serde(default = "default_weight")]
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkRule {
    pub ns: String,
    pub value: String,
    pub host_contains: Vec<String>,
    #[serde(default = "default_weight")]
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodefenceRule {
    pub ns: String,
    pub value: String,
    pub languages: Vec<String>,
    #[serde(default = "default_weight")]
    pub weight: f32,
}

fn default_weight() -> f32 {
    1.0
}

impl YamlRuleSet {
    /// YAML 형식을 기존 RuleSet으로 변환
    pub fn to_rule_set(&self) -> RuleSet {
        let mut rule_set = RuleSet::new();

        // 정규식 규칙 변환 - 패턴과 값을 매핑하여 저장
        for rule in &self.regex_rules {
            // 패턴을 키로, 값을 값으로 하는 특별한 형식으로 저장
            let pattern_with_value = format!("{}|{}", rule.pattern, rule.value);
            rule_set
                .regexes
                .entry(rule.ns.clone())
                .or_default()
                .push(pattern_with_value);
        }

        // 키워드 규칙 변환
        for rule in &self.keyword_rules {
            for keyword in &rule.keywords {
                rule_set
                    .keywords
                    .entry(rule.ns.clone())
                    .or_default()
                    .push(keyword.clone());
            }
        }

        // 링크 규칙 변환
        for rule in &self.link_rules {
            for host in &rule.host_contains {
                rule_set
                    .link_hosts
                    .entry(rule.ns.clone())
                    .or_default()
                    .push(host.clone());
            }
        }

        // TODO: codefence_rules 처리 (필요시 추가)

        rule_set
    }
}
