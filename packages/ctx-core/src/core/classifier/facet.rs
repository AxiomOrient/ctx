use crate::common::rule_application::{RuleAction, RuleApplicationResult};
use crate::knowledge::rules::applier::ClassificationLike;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// 패싯 맵 타입 (기존 호환성)
pub type FacetMap = HashMap<String, Vec<String>>;

/// 패싯 컬렉션 (캡슐화된 접근)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Facets(BTreeMap<String, BTreeSet<String>>);

impl Facets {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// 패싯 값 추가
    pub fn insert(&mut self, namespace: &str, value: &str) {
        self.0
            .entry(namespace.to_string())
            .or_default()
            .insert(value.to_string());
    }

    /// 네임스페이스의 값들 조회
    pub fn get(&self, namespace: &str) -> Option<&BTreeSet<String>> {
        self.0.get(namespace)
    }

    /// 특정 패싯 값 존재 여부 확인
    pub fn contains(&self, namespace: &str, value: &str) -> bool {
        self.0
            .get(namespace)
            .map(|values| values.contains(value))
            .unwrap_or(false)
    }

    /// 모든 패싯 반복자
    pub fn iter(&self) -> impl Iterator<Item = (&String, &BTreeSet<String>)> {
        self.0.iter()
    }

    /// 네임스페이스 목록
    pub fn namespaces(&self) -> Vec<&String> {
        self.0.keys().collect()
    }

    /// 비어있는지 확인
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 패싯 개수
    pub fn len(&self) -> usize {
        self.0.values().map(|v| v.len()).sum()
    }

    /// HashMap 형태로 변환 (기존 호환성)
    pub fn to_hashmap(&self) -> HashMap<String, Vec<String>> {
        self.0
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
            .collect()
    }

    /// HashMap에서 생성 (기존 호환성)
    pub fn from_hashmap(map: HashMap<String, Vec<String>>) -> Self {
        let mut facets = Self::new();
        for (namespace, values) in map {
            for value in values {
                facets.insert(&namespace, &value);
            }
        }
        facets
    }

    /// HashSet 형태로 변환 (기존 호환성)
    pub fn to_hashset_map(&self) -> HashMap<String, HashSet<String>> {
        self.0
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
            .collect()
    }

    /// Vec 형태로 변환 (트레이트 구현용)
    pub fn to_vec_map(&self) -> HashMap<String, Vec<String>> {
        self.0
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
            .collect()
    }
}

/// 분류 결과 (캡슐화된 접근)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    /// 분류된 패싯들
    facets: Facets,
    /// 신뢰도 점수 (0.0 ~ 1.0)
    confidence: f32,
    /// 경고 메시지들
    warnings: Vec<String>,
    /// 오류 메시지들
    errors: Vec<String>,
}

impl Default for Classification {
    fn default() -> Self {
        Self {
            facets: Facets::new(),
            confidence: 0.0,
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl Classification {
    pub fn new() -> Self {
        Self::default()
    }

    /// 패싯 접근 (읽기 전용)
    pub fn facets(&self) -> &Facets {
        &self.facets
    }

    /// 패싯 소유권 이동
    pub fn into_facets(self) -> Facets {
        self.facets
    }

    /// 신뢰도 조회
    pub fn confidence(&self) -> f32 {
        self.confidence
    }

    /// 경고 목록 조회
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// 오류 목록 조회
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// 패싯 추가
    pub fn add_facet(&mut self, namespace: String, value: String) {
        self.facets.insert(&namespace, &value);
    }

    /// 특정 패싯 값 존재 여부 확인
    pub fn has_facet(&self, namespace: &str, value: &str) -> bool {
        self.facets.contains(namespace, value)
    }

    /// 신뢰도 설정
    pub fn set_confidence(&mut self, confidence: f32) {
        self.confidence = confidence.clamp(0.0, 1.0);
    }

    /// 경고 추가
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// 오류 추가
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    /// 유효한 분류인지 확인 (오류 없음)
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// 기존 호환성을 위한 HashMap 변환
    pub fn to_legacy_format(&self) -> HashMap<String, HashSet<String>> {
        self.facets.to_hashset_map()
    }

    /// 기존 형태에서 생성
    pub fn from_legacy_format(
        facets: HashMap<String, HashSet<String>>,
        confidence: f32,
        warnings: Vec<String>,
        errors: Vec<String>,
    ) -> Self {
        let facets_map: HashMap<String, Vec<String>> = facets
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect();

        Self {
            facets: Facets::from_hashmap(facets_map),
            confidence,
            warnings,
            errors,
        }
    }

    /// 규칙 적용 결과를 현재 분류에 적용
    pub fn apply_rule_result(&mut self, rule_result: RuleApplicationResult) {
        for action in rule_result.actions {
            match action {
                RuleAction::AddFacet { namespace, value } => {
                    self.add_facet(namespace, value);
                }
                RuleAction::AddWarning(warning) => {
                    self.add_warning(warning);
                }
                RuleAction::AddError(error) => {
                    self.add_error(error);
                }
            }
        }
    }
}

/// ClassificationLike 트레이트 구현 - 아키텍처 의존성 해결
impl ClassificationLike for Classification {
    fn has_facet(&self, namespace: &str, value: &str) -> bool {
        self.facets.contains(namespace, value)
    }
}
