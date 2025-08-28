use super::schema::RuleSet;
use crate::domain::rule_application::RuleApplicationResult;

/// 분류 결과 추상화 - 아키텍처 의존성 해결을 위한 트레이트
pub trait ClassificationLike {
    fn has_facet(&self, namespace: &str, value: &str) -> bool;
}

/// 규칙 적용기 - implies/conflicts 검증 및 적용 (아키텍처 원칙 준수)
pub struct RuleApplier<'a> {
    rules: &'a RuleSet,
}

impl<'a> RuleApplier<'a> {
    pub fn new(rules: &'a RuleSet) -> Self {
        Self { rules }
    }

    /// 분류 결과에 적용할 규칙들을 계산하여 변경사항 목록 반환
    pub fn calculate_rule_applications<T: ClassificationLike>(
        &self,
        classification: &T,
    ) -> RuleApplicationResult {
        let mut result = RuleApplicationResult::new();

        self.calculate_requires(classification, &mut result);
        self.calculate_prohibits(classification, &mut result);

        result
    }

    /// 필수 동반 규칙 계산 (if A then B)
    fn calculate_requires<T: ClassificationLike>(
        &self,
        classification: &T,
        result: &mut RuleApplicationResult,
    ) {
        for rule in &self.rules.requires {
            if classification.has_facet(&rule.a_ns, &rule.a_val)
                && !classification.has_facet(&rule.b_ns, &rule.b_val)
            {
                result.add_facet(rule.b_ns.clone(), rule.b_val.clone());
                result.add_warning(format!(
                    "Auto-applied '{}:{}' due to rule: {}",
                    rule.b_ns, rule.b_val, rule.reason
                ));
                result.increment_applied();
            }
        }
    }

    /// 금지된 조합 검증
    fn calculate_prohibits<T: ClassificationLike>(
        &self,
        classification: &T,
        result: &mut RuleApplicationResult,
    ) {
        for rule in &self.rules.prohibited {
            if classification.has_facet(&rule.a_ns, &rule.a_val)
                && classification.has_facet(&rule.b_ns, &rule.b_val)
            {
                result.add_error(format!(
                    "Prohibited combination: '{}:{}' + '{}:{}' - {}",
                    rule.a_ns, rule.a_val, rule.b_ns, rule.b_val, rule.reason
                ));
                result.increment_applied();
            }
        }
    }
}
