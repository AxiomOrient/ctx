use super::schema::{ProhibitRule, RequireRule, RuleSet, YamlRuleSet};
use crate::Result;
use std::collections::HashMap;
use std::path::Path;

/// rules.yaml 파일에서 규칙 로드
pub fn load_from_file(path: &Path) -> Result<RuleSet> {
    let content = std::fs::read_to_string(path).map_err(crate::ContextError::Io)?;
    load_from_string(&content)
}

/// 문자열에서 규칙 로드 (새로운 YAML 형식 지원)
pub fn load_from_string(content: &str) -> Result<RuleSet> {
    // 먼저 새로운 YAML 형식으로 파싱 시도
    if let Ok(yaml_rules) = serde_yaml::from_str::<YamlRuleSet>(content) {
        return Ok(yaml_rules.to_rule_set());
    }

    // 실패하면 기존 형식으로 파싱 시도
    let rules: RuleSet = serde_yaml::from_str(content)
        .map_err(|e| crate::ContextError::InvalidFrontmatter(e.to_string()))?;
    Ok(rules)
}

/// YAML 문자열에서 규칙 로드 (새로운 형식 전용)
pub fn load_yaml_rules_from_string(content: &str) -> Result<RuleSet> {
    let yaml_rules: YamlRuleSet = serde_yaml::from_str(content)
        .map_err(|e| crate::ContextError::InvalidFrontmatter(e.to_string()))?;
    Ok(yaml_rules.to_rule_set())
}

/// config 디렉토리에서 rules.yaml 로드 (기본 설정 파일)
pub fn load_from_config_dir() -> Result<RuleSet> {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("config/rules.yaml");
    load_from_file(&config_path)
}

/// 내장 분류 규칙 생성
pub fn builtin_rules() -> RuleSet {
    let mut keywords: HashMap<String, Vec<String>> = HashMap::new();

    // 플랫폼 키워드
    keywords.insert(
        "platform".into(),
        vec![
            "ios".into(),
            "android".into(),
            "web".into(),
            "macos".into(),
            "tvos".into(),
            "watchos".into(),
        ],
    );

    // 기술 키워드
    keywords.insert(
        "tech".into(),
        vec![
            "swift".into(),
            "swiftui".into(),
            "uikit".into(),
            "kotlin".into(),
            "compose".into(),
            "react".into(),
            "svelte".into(),
            "rust".into(),
            "sqlite".into(),
            "objectbox".into(),
        ],
    );

    // 아키텍처 키워드
    keywords.insert(
        "arch".into(),
        vec![
            "mvvm".into(),
            "mvi".into(),
            "clean".into(),
            "tca".into(),
            "redux".into(),
        ],
    );

    // UI 키워드
    keywords.insert(
        "ui".into(),
        vec!["hig".into(), "material".into(), "a11y".into()],
    );

    // 도구 키워드
    keywords.insert(
        "tool".into(),
        vec![
            "xcode".into(),
            "gradle".into(),
            "tuist".into(),
            "swiftlint".into(),
            "swiftformat".into(),
            "github".into(),
            "figma".into(),
        ],
    );

    // 데이터베이스 키워드
    keywords.insert(
        "db".into(),
        vec![
            "sqlite".into(),
            "objectbox".into(),
            "realm".into(),
            "postgres".into(),
        ],
    );

    let mut regexes: HashMap<String, Vec<String>> = HashMap::new();

    // 기술 정규식
    regexes.insert(
        "tech".into(),
        vec![
            r"(?i)\bSwiftUI\b".into(),
            r"(?i)\bUIKit\b".into(),
            r"(?i)\bJetpack\s*Compose\b".into(),
            r"(?i)\bReact(\.js|JS)?\b".into(),
            r"(?i)\bSvelte(Kit)?\b".into(),
            r"(?i)\bObjectBox\b".into(),
            r"(?i)\bSQLite\b".into(),
        ],
    );

    // 아키텍처 정규식
    regexes.insert(
        "arch".into(),
        vec![
            r"(?i)\bMVVM\b".into(),
            r"(?i)\bMVI\b".into(),
            r"(?i)\bClean\s+Architecture\b".into(),
            r"(?i)\bTCA\b".into(),
            r"(?i)\bRedux\b".into(),
        ],
    );

    // UI 정규식
    regexes.insert(
        "ui".into(),
        vec![
            r"(?i)\bHuman\s*Interface\s*Guidelines\b".into(),
            r"(?i)\bHIG\b".into(),
            r"(?i)\bMaterial\s*Design( 3)?\b".into(),
            r"(?i)\bA11y\b".into(),
            r"(?i)\bAccessibility\b".into(),
        ],
    );

    // 도구 정규식
    regexes.insert(
        "tool".into(),
        vec![
            r"(?i)\bXcode\b".into(),
            r"(?i)\bGradle\b".into(),
            r"(?i)\bTuist\b".into(),
            r"(?i)\bSwiftLint\b".into(),
            r"(?i)\bSwiftFormat\b".into(),
            r"(?i)\bGitHub\b".into(),
            r"(?i)\bFigma\b".into(),
        ],
    );

    let mut link_hosts: HashMap<String, Vec<String>> = HashMap::new();
    link_hosts.insert(
        "platform".into(),
        vec!["developer.apple.com".into(), "developer.android.com".into()],
    );
    link_hosts.insert(
        "ui".into(),
        vec!["m3.material.io".into(), "developer.apple.com/design".into()],
    );
    link_hosts.insert("tool".into(), vec!["github.com".into(), "figma.com".into()]);

    // 정합성 규칙
    let prohibited = vec![
        ProhibitRule {
            a_ns: "platform".into(),
            a_val: "ios".into(),
            b_ns: "tech".into(),
            b_val: "compose".into(),
            reason: "Android Compose는 iOS와 비정합".into(),
        },
        ProhibitRule {
            a_ns: "platform".into(),
            a_val: "android".into(),
            b_ns: "tech".into(),
            b_val: "swiftui".into(),
            reason: "SwiftUI는 Android와 비정합".into(),
        },
        // 동일 네임스페이스 내 프레임워크 충돌 (SwiftUI vs UIKit)
        ProhibitRule {
            a_ns: "tech".into(),
            a_val: "swiftui".into(),
            b_ns: "tech".into(),
            b_val: "uikit".into(),
            reason: "동일 문맥에서 SwiftUI와 UIKit 동시 주 프레임워크 사용 금지".into(),
        },
        // UI 가이드 충돌 (HIG vs Material)
        ProhibitRule {
            a_ns: "ui".into(),
            a_val: "hig".into(),
            b_ns: "ui".into(),
            b_val: "material".into(),
            reason: "플랫폼 고유 UI 가이드 혼용은 비정합".into(),
        },
    ];

    let requires = vec![
        RequireRule {
            a_ns: "tech".into(),
            a_val: "swiftui".into(),
            b_ns: "platform".into(),
            b_val: "ios".into(),
            reason: "SwiftUI는 Apple 플랫폼과 함께 나타나야 함".into(),
        },
        RequireRule {
            a_ns: "tech".into(),
            a_val: "compose".into(),
            b_ns: "platform".into(),
            b_val: "android".into(),
            reason: "Jetpack Compose는 Android 플랫폼과 함께 나타나야 함".into(),
        },
    ];

    RuleSet {
        keywords,
        regexes,
        link_hosts,
        prohibited,
        requires,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_yaml_rules_from_string() {
        let yaml_content = r#"
regex_rules:
  - ns: "lang"
    value: "rust"
    pattern: "\\bRust\\b"
    weight: 1.0
keyword_rules:
  - ns: "platform"
    value: "ios"
    keywords: ["ios", "iphone"]
    weight: 0.8
"#;
        let rule_set = load_yaml_rules_from_string(yaml_content).unwrap();
        
        // 정규식 규칙이 변환되었는지 확인
        assert!(rule_set.regexes.contains_key("lang"));
        
        // 키워드 규칙이 변환되었는지 확인  
        assert!(rule_set.keywords.contains_key("platform"));
        let platform_keywords = &rule_set.keywords["platform"];
        assert!(platform_keywords.contains(&"ios".to_string()));
        assert!(platform_keywords.contains(&"iphone".to_string()));
    }

    #[test]
    fn test_builtin_rules() {
        let rules = builtin_rules();
        
        // 기본 규칙들이 포함되어 있는지 확인
        assert!(rules.keywords.contains_key("platform"));
        assert!(rules.keywords.contains_key("tech"));
        assert!(rules.regexes.contains_key("tech"));
        assert!(!rules.prohibited.is_empty());
        assert!(!rules.requires.is_empty());
    }

    #[test]
    fn test_config_dir_loading() {
        // config 디렉토리에서 실제 파일 로딩 테스트
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config/rules.yaml");
        
        if config_path.exists() {
            let result = load_from_config_dir();
            assert!(result.is_ok(), "Failed to load rules from config dir: {:?}", result.err());
        }
    }
}
