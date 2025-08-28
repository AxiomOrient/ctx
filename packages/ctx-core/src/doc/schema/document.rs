use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 새로운 패싯 기반 문서 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetDocument {
    pub id: String,
    pub title: Option<String>,
    pub version: Option<u32>,
    pub locale: Option<String>,
    /// 패싯: 네임스페이스 -> 값들의 배열
    pub facets: HashMap<String, Vec<String>>,
    /// 신뢰도 점수 (0.0 ~ 1.0)
    pub trust: Option<f32>,
    /// 문서 신선도 (ISO 날짜)
    pub freshness: Option<String>,
    /// 별칭들
    pub aliases: Option<Vec<String>>,
    /// 의존성 (다른 문서 ID들)
    pub requires: Option<Vec<String>>,
    /// 충돌 (함께 사용할 수 없는 문서 ID들)
    pub conflicts: Option<Vec<String>>,
    /// 섹션 정의들 (기존 호환성)
    pub sections: Vec<crate::SectionDef>,
}

impl FacetDocument {
    pub fn new(id: String) -> Self {
        Self {
            id,
            title: None,
            version: Some(1),
            locale: Some("ko".to_string()),
            facets: HashMap::new(),
            trust: None,
            freshness: None,
            aliases: None,
            requires: None,
            conflicts: None,
            sections: Vec::new(),
        }
    }

    /// 특정 패싯 값이 있는지 확인
    pub fn has_facet(&self, namespace: &str, value: &str) -> bool {
        self.facets
            .get(namespace)
            .map(|values| values.contains(&value.to_string()))
            .unwrap_or(false)
    }

    /// 패싯 추가
    pub fn add_facet(&mut self, namespace: String, value: String) {
        self.facets.entry(namespace).or_default().push(value);
    }

    /// 패싯 값들을 중복 제거하여 정리
    pub fn deduplicate_facets(&mut self) {
        for values in self.facets.values_mut() {
            values.sort();
            values.dedup();
        }
    }

    /// 신선도 체크 (일 단위)
    pub fn days_since_freshness(&self) -> Option<i64> {
        if let Some(freshness_str) = &self.freshness {
            if let Ok(freshness_date) = DateTime::parse_from_rfc3339(freshness_str) {
                let now = Utc::now();
                let duration = now.signed_duration_since(freshness_date.with_timezone(&Utc));
                return Some(duration.num_days());
            }
        }
        None
    }

    /// 문서가 빌드에 포함될 수 있는지 확인
    pub fn is_buildable(&self, max_age_days: i64, min_trust: f32) -> bool {
        // 신선도 체크
        if let Some(days) = self.days_since_freshness() {
            if days > max_age_days {
                return false;
            }
        }

        // 신뢰도 체크
        if let Some(trust) = self.trust {
            if trust < min_trust {
                return false;
            }
        }

        true
    }
}

/// 기존 ContextDocument에서 FacetDocument로 변환
impl From<crate::ContextDocument> for FacetDocument {
    fn from(doc: crate::ContextDocument) -> Self {
        let mut facet_doc = FacetDocument::new(doc.id);
        facet_doc.title = Some(doc.title);
        facet_doc.version = Some(1);
        facet_doc.sections = doc.sections;

        // 기존 태그를 'tag' 네임스페이스로 변환
        if !doc.tags.is_empty() {
            facet_doc.facets.insert("tag".to_string(), doc.tags);
        }

        // 기존 타입을 'artifact' 네임스페이스에 매핑
        facet_doc.add_facet("artifact".to_string(), doc.r#type);

        // 기존 도메인을 'domain' 네임스페이스에 매핑
        if let Some(domain) = doc.domain {
            facet_doc.add_facet("domain".to_string(), domain);
        }

        facet_doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_facet_document_basic() {
        let mut doc = FacetDocument::new("test-doc".to_string());
        doc.add_facet("platform".to_string(), "ios".to_string());
        doc.add_facet("tech".to_string(), "swiftui".to_string());

        assert!(doc.has_facet("platform", "ios"));
        assert!(doc.has_facet("tech", "swiftui"));
        assert!(!doc.has_facet("platform", "android"));
    }

    #[test]
    #[allow(clippy::expect_used)] // 테스트 코드에서는 허용
    fn test_freshness_check() {
        let mut doc = FacetDocument::new("test-doc".to_string());
        doc.freshness = Some("2025-01-01T00:00:00Z".to_string());

        let days = doc.days_since_freshness();
        assert!(days.is_some());
        assert!(days.expect("days should be Some for valid date") > 0); // 현재 날짜가 2025-01-01 이후라고 가정
    }
}
