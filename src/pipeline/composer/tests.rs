#[cfg(test)]
mod integration_tests {
    use crate::domain::constants::determinism::EXECUTION_ID_LENGTH;
    use crate::pipeline::composer::BuildComposer;
    use crate::domain::types::{BuildQuery, DocumentCandidate};

    fn create_test_candidate(id: &str, title: &str, confidence: f32) -> DocumentCandidate {
        DocumentCandidate {
            doc_id: id.to_string(),
            sha: "test_sha".to_string(),
            title: title.to_string(),
            locale: "en".to_string(),
            trust: 0.8,
            freshness: chrono::Utc::now().to_rfc3339(),
            confidence,
            path: format!("/test/{}.md", id),
            facets: std::collections::HashMap::new(),
            tokens: 100,
            content: Some(format!("Content of {}", title)),
        }
    }

    #[test]
    fn test_compose_pipeline_end_to_end_success() -> crate::domain::errors::Result<()> {
        let composer = BuildComposer::new()?;
        let query = BuildQuery::new("repo".into(), "main".into(), "abc123".into())
            .with_confidence_threshold(0.5);

        let c1 = create_test_candidate("doc1", "First Doc", 0.9);
        let c2 = create_test_candidate("doc2", "Second Doc", 0.8);
        let result = composer.compose(vec![c1, c2], &query, 1000, 100)?;

        // Header and metadata
        assert!(
            result
                .merged_document
                .content
                .contains("# Context Composition")
        );
        assert!(
            result
                .merged_document
                .content
                .contains("**Repository:** repo")
        );

        // Token/accounting and execution id
        assert!(result.merged_document.tokens > 0);
        assert_eq!(result.execution_id.len(), EXECUTION_ID_LENGTH);

        // Sources and confidence
        assert_eq!(result.merged_document.source_documents.len(), 2);
        assert!(result.confidence_score > 0.0 && result.confidence_score <= 1.0);

        // Rationale present
        assert!(result.selection_rationale.contains("Selected Documents"));
        Ok(())
    }

    #[test]
    fn test_compose_respects_confidence_threshold() -> crate::domain::errors::Result<()> {
        let composer = BuildComposer::new()?;
        let query = BuildQuery::new("repo".into(), "main".into(), "abc123".into())
            .with_confidence_threshold(0.75);

        let c1 = create_test_candidate("doc1", "High Confidence", 0.9);
        let c2 = create_test_candidate("doc2", "Low Confidence", 0.5); // filtered out
        let res = composer.compose(vec![c1, c2], &query, 1000, 0)?;

        assert_eq!(res.merged_document.source_documents.len(), 1);
        Ok(())
    }

    #[test]
    fn test_compose_budget_enforcement_rationale() -> crate::domain::errors::Result<()> {
        let composer = BuildComposer::new()?;
        let query = BuildQuery::new("repo".into(), "main".into(), "abc123".into())
            .with_confidence_threshold(0.0);

        // Make three candidates that cannot all fit (tokens=100 each, budget=180)
        let mut c1 = create_test_candidate("a", "A", 0.9);
        c1.tokens = 100;
        let mut c2 = create_test_candidate("b", "B", 0.8);
        c2.tokens = 100;
        let mut c3 = create_test_candidate("c", "C", 0.7);
        c3.tokens = 100;

        let res = composer.compose(vec![c1, c2, c3], &query, 180, 0)?;

        // Some document(s) must be rejected and rationale reports it
        assert!(res.merged_document.source_documents.len() <= 2);
        assert!(res.selection_rationale.contains("Rejected Documents"));
        Ok(())
    }
    #[test]
    fn test_build_composer_creation() -> crate::domain::errors::Result<()> {
        let composer = BuildComposer::new()?;
        // 기본 구조 테스트
        assert!(std::mem::size_of_val(&composer) > 0);
        Ok(())
    }

    #[test]
    fn test_build_query_creation() {
        let query = BuildQuery::new(
            "test_repo".to_string(),
            "main".to_string(),
            "abc123".to_string(),
        )
        .with_confidence_threshold(0.5);

        assert_eq!(query.repo, "test_repo");
        assert_eq!(query.branch, "main");
        assert_eq!(query.commit_sha, "abc123");
        assert_eq!(query.confidence_threshold, 0.5);
    }

    #[test]
    fn test_document_candidate_creation() {
        let candidate = create_test_candidate("doc1", "Test Doc", 0.8);

        assert_eq!(candidate.doc_id, "doc1");
        assert_eq!(candidate.title, "Test Doc");
        assert_eq!(candidate.confidence, 0.8);
        assert_eq!(candidate.tokens, 100);
    }
}
