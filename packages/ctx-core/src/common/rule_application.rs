//! 규칙 적용 결과 타입 정의
//!
//! 아키텍처 의존성을 해결하기 위해 core의 Classification 타입 대신 사용

use serde::{Deserialize, Serialize};

/// 규칙 적용 작업 타입
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuleAction {
    /// 패싯 추가
    AddFacet { namespace: String, value: String },
    /// 경고 추가
    AddWarning(String),
    /// 오류 추가
    AddError(String),
}

/// 규칙 적용 결과
#[derive(Debug, Clone, Default)]
pub struct RuleApplicationResult {
    /// 적용할 작업 목록
    pub actions: Vec<RuleAction>,
    /// 적용된 규칙 개수
    pub rules_applied: usize,
}

impl RuleApplicationResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_action(&mut self, action: RuleAction) {
        self.actions.push(action);
    }

    pub fn add_facet(&mut self, namespace: String, value: String) {
        self.add_action(RuleAction::AddFacet { namespace, value });
    }

    pub fn add_warning(&mut self, message: String) {
        self.add_action(RuleAction::AddWarning(message));
    }

    pub fn add_error(&mut self, message: String) {
        self.add_action(RuleAction::AddError(message));
    }

    pub fn increment_applied(&mut self) {
        self.rules_applied += 1;
    }

    pub fn merge(&mut self, other: RuleApplicationResult) {
        self.actions.extend(other.actions);
        self.rules_applied += other.rules_applied;
    }
}
