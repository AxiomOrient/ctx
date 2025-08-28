use ctxset::doc::validate::ValidationPipeline;
use ctxset::{
    BuildComposer, BuildQuery, Classification, DocumentCandidate, OntologyRegistry, builtin_rules,
};
use std::collections::HashMap;

/// 핵심 분류 기능 테스트
#[test]
fn test_classification_system() -> ctxset::Result<()> {
    // 1. 온톨로지 로드
    let ontology_content = r#"
namespaces:
  platform:
    values:
      ios:
        synonyms: ["iOS"]
        parents: ["mobile"]
        deprecated: false
      web:
        synonyms: ["browser"]
        parents: ["internet"]
        deprecated: false
  framework:
    values:
      swiftui:
        synonyms: []
        parents: ["swift"]
        deprecated: false
      react:
        synonyms: ["reactjs"]
        parents: ["typescript"]
        deprecated: false
"#;

    let _ontology = OntologyRegistry::from_yaml(ontology_content)?;
    let _rules = builtin_rules();

    // 2. 분류 테스트
    let mut classification = Classification::new();
    classification.add_facet("framework".to_string(), "swiftui".to_string());
    classification.add_facet("platform".to_string(), "ios".to_string());
    classification.set_confidence(0.9);

    // 3. 검증
    assert!(classification.has_facet("framework", "swiftui"));
    assert!(classification.has_facet("platform", "ios"));
    assert_eq!(classification.confidence(), 0.9);
    assert!(classification.is_valid());

    Ok(())
}

/// 문서 조합 시스템 테스트
#[test]
fn test_composition_system() -> ctxset::Result<()> {
    // 1. 빌드 쿼리 생성
    let query = BuildQuery::new(
        "test-repo".to_string(),
        "main".to_string(),
        "abc123".to_string(),
    )
    .with_facet("platform".to_string(), vec!["ios".to_string()])
    .with_confidence_threshold(0.7);

    // 2. 문서 후보 생성
    let mut facets = HashMap::new();
    facets.insert("platform".to_string(), vec!["ios".to_string()]);
    facets.insert("framework".to_string(), vec!["swiftui".to_string()]);

    let candidate = DocumentCandidate {
        doc_id: "test-doc".to_string(),
        sha: "abc123".to_string(),
        title: "SwiftUI Guide".to_string(),
        locale: "ko".to_string(),
        trust: 0.8,
        freshness: "2025-08-12T00:00:00Z".to_string(),
        confidence: 0.9,
        path: "/tmp/test.md".to_string(),
        facets,
        tokens: 500,
        content: None,
    };

    // 3. 조합 시스템 테스트
    let _composer = BuildComposer::new();
    let _candidates = vec![candidate.clone()];

    // 실제 조합은 파일이 필요하므로 기본 검증만
    assert!(candidate.has_facet("platform", "ios"));
    assert!(candidate.has_facet("framework", "swiftui"));
    assert_eq!(candidate.tokens, 500);
    assert_eq!(query.confidence_threshold, 0.7);

    Ok(())
}

/// 검증 파이프라인 테스트
#[test]
fn test_validation_pipeline() -> ctxset::Result<()> {
    use ctxset::doc::validate::stages::{
        MetadataValidationStage, SchemaValidationStage, StructureValidationStage,
    };

    // 1. 파이프라인 구성
    let mut pipeline = ValidationPipeline::new();
    pipeline.add_stage(MetadataValidationStage);
    pipeline.add_stage(SchemaValidationStage);
    pipeline.add_stage(StructureValidationStage);

    // 2. 유효한 문서 테스트 (섹션 포함)
    let mut valid_doc = ctxset::ContextDocument::new_test("TEST-DOC", "Test Document");
    valid_doc.sections.push(ctxset::SectionDef {
        id: "intro".to_string(),
        name: "Introduction".to_string(),
        marker: "## Introduction".to_string(),
        priority: 10,
        tokens: Some(100),
    });
    let body = "## Introduction\nTest content";

    let report = pipeline.run(&valid_doc, body)?;

    // 디버깅 정보 출력
    if !report.is_valid {
        eprintln!("Validation failed:");
        for failure in &report.failed_stages {
            eprintln!("  Stage '{}': {}", failure.stage_name, failure.error);
        }
    }

    assert!(
        report.is_valid,
        "Expected valid document to pass validation"
    );
    assert_eq!(report.passed_count(), 3);
    assert_eq!(report.failed_count(), 0);

    // 3. 무효한 문서 테스트 (빈 ID와 제목)
    let invalid_doc = ctxset::ContextDocument::new_test("", ""); // 빈 ID, 제목

    let report = pipeline.run(&invalid_doc, body)?;
    assert!(!report.is_valid);
    assert!(report.failed_count() > 0);
    // 메타데이터 검증에서 실패해야 함
    assert!(
        report
            .failed_stages
            .iter()
            .any(|f| f.stage_name == "metadata")
    );

    Ok(())
}

/// 상수 시스템 테스트
#[test]
fn test_constants_system() {
    use ctxset::common::constants::{defaults::*, parsing::*, scoring::*, validation::*};

    // 스코어링 상수들
    assert_eq!(DEFAULT_MMR_LAMBDA, 0.7);
    assert_eq!(DEFAULT_CONFIDENCE_THRESHOLD, 0.65);
    assert_eq!(KNAPSACK_BUDGET_THRESHOLD, 20_000);

    // 파싱 상수들
    assert_eq!(FRONTMATTER_DELIMITER, "---");
    assert_eq!(HEADER_PREFIX, "##");
    assert_eq!(MAX_CATEGORY_DEPTH, 3);

    // 검증 상수들
    assert_eq!(SIMILARITY_THRESHOLD, 0.7);
    assert_eq!(MAX_PRIORITY, 100);

    // 기본값 상수들
    assert_eq!(DEFAULT_TRUST, 0.8);
    assert_eq!(DEFAULT_CLASSIFICATION_CONFIDENCE, 0.8);
    assert_eq!(DEFAULT_LOCALE, "ko");
}

// 레거시 호환성 테스트 제거됨 - classifier::engine::Engine 더 이상 필요 없음
