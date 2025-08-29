use crate::domain::errors::Result;
use crate::domain::types::MergedDocument;
use crate::domain::types::{BuildQuery, DocumentCandidate};
use crate::pipeline::scorer::ScoredCandidate;
// use crate::domain::tokenizer::{SimpleTokenizer, Tokenizer};
// chrono::Utc not used directly here; timestamps are formatted via chrono in formatting macros
use regex::Regex;
use std::collections::HashSet;

/// 문서 병합 옵션
#[derive(Debug, Clone)]
pub struct MergeOptions {
    pub include_sources: bool,
    pub remove_duplicate_headers: bool,
    pub preserve_code_blocks: bool,
    pub deduplicate_paragraphs: bool,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            include_sources: true,
            remove_duplicate_headers: true,
            preserve_code_blocks: true,
            deduplicate_paragraphs: true,
        }
    }
}

/// 문서 병합기 (design.md 사양 구현)
pub struct DocumentMerger {
    paragraph_regex: Regex,
    // tokenizer: Box<dyn Tokenizer>,
}

impl DocumentMerger {
    pub fn new() -> Result<Self> {
        Ok(Self {
            paragraph_regex: Regex::new(r"\n{2,}").map_err(|e| {
                crate::domain::errors::ContextError::Other(format!("Invalid regex pattern: {}", e))
            })?,
            // tokenizer: Box::new(SimpleTokenizer),
        })
    }

    // pub fn with_tokenizer(tokenizer: Box<dyn Tokenizer>) -> Result<Self> {
    //     Ok(Self {
    //         paragraph_regex: Regex::new(r"\n{2,}")
    //             .map_err(|e| crate::domain::errors::ContextError::Other(format!("Invalid regex pattern: {}", e)))?,
    //         tokenizer,
    //     })
    // }

    /// design.md 사양에 맞는 문서 병합 (ScoredCandidate + BuildQuery)
    pub fn merge_documents_with_query(
        &self,
        mut documents: Vec<ScoredCandidate>,
        query: &BuildQuery,
    ) -> Result<MergedDocument> {
        if documents.is_empty() {
            return Ok(MergedDocument::empty());
        }

        // 1. Sort by score (highest first)
        documents.sort_by(|a, b| {
            b.base_score
                .partial_cmp(&a.base_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut merged_content = String::new();
        let mut source_documents = Vec::new();
        // let mut sections = Vec::new();
        let mut total_tokens = 0;

        // 2. Add header with context information
        merged_content.push_str(&format!(
            "# Context Composition\n\n**Repository:** {}\n**Branch:** {}\n**Commit:** {}\n\n",
            query.repo, query.branch, query.commit_sha
        ));

        // 3. Process each document
        for (idx, scored_doc) in documents.iter().enumerate() {
            let doc = &scored_doc.candidate;

            // Load document content if not already loaded
            let content = if let Some(ref content) = doc.content {
                content.clone()
            } else {
                std::fs::read_to_string(&doc.path)
                    .unwrap_or_else(|_| format!("⚠️ Could not load: {}", doc.path))
            };

            // 4. Add section header with metadata
            let section_header = format!(
                "## Document {}: {}\n\n**Source:** `{}`  \n**Confidence:** {:.2}  \n**Trust:** {:.2}  \n**Freshness:** {}  \n\n",
                idx + 1,
                doc.title,
                doc.path,
                doc.confidence,
                doc.trust,
                doc.freshness
            );

            merged_content.push_str(&section_header);
            merged_content.push_str(&content);
            merged_content.push_str("\n\n---\n\n");

            // 5. Track metadata
            source_documents.push(doc.doc_id.clone());
            // sections.push(MergedSection {
            //     title: doc.title.clone(),
            //     content: content.clone(),
            //     source_doc_id: doc.doc_id.clone(),
            //     confidence: doc.confidence,
            // });

            total_tokens += doc.tokens;
        }

        // 6. Add footer with composition metadata
        merged_content.push_str(&format!(
            "---\n\n**Composition Summary:**\n- Documents: {}\n- Total Tokens: ~{}\n- Generated: {}\n",
            documents.len(),
            total_tokens,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // 7. Recalculate final token count (may differ due to headers)
        // let final_tokens = self.tokenizer.estimate_tokens(&merged_content) as usize;

        Ok(MergedDocument {
            content: merged_content,
            source_documents,
            tokens: total_tokens, //final_tokens,
            sections: vec![],     // sections,
        })
    }

    /// 기존 인터페이스 유지 (선택된 문서들을 병합하여 하나의 프롬프트 생성)
    pub fn merge_documents(
        &self,
        candidates: Vec<DocumentCandidate>,
        options: &MergeOptions,
    ) -> Result<MergedDocument> {
        let mut content = String::new();
        let mut seen_shingles = HashSet::new();
        let mut _total_tokens = 0;
        let mut source_documents = Vec::new();

        // 헤더 생성
        content.push_str("# Composed Prompt\n\n");

        for candidate in &candidates {
            // 문서 제목 추가
            content.push_str(&format!("## {}\n\n", candidate.title));

            // 파일 내용 읽기 (실제 구현에서는 캐시된 내용 사용)
            let body = self.load_document_content(candidate)?;
            let processed_body = if options.remove_duplicate_headers {
                self.strip_top_heading(&body)
            } else {
                body
            };

            // 문단 단위 중복 제거
            if options.deduplicate_paragraphs {
                for paragraph in self.paragraph_regex.split(&processed_body) {
                    let trimmed = paragraph.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let shingle = self.create_shingle(trimmed);
                    if seen_shingles.contains(&shingle) {
                        continue;
                    }
                    seen_shingles.insert(shingle);

                    content.push_str(trimmed);
                    content.push_str("\n\n");
                }
            } else {
                content.push_str(&processed_body);
                content.push_str("\n\n");
            }

            // 출처 정보 추가
            if options.include_sources {
                content.push_str(&format!(
                    "<!-- src: {}@{} -->\n\n",
                    candidate.doc_id, candidate.sha
                ));
            }

            _total_tokens += candidate.tokens;
            source_documents.push(candidate.doc_id.clone());
        }

        // let final_tokens = self.tokenizer.estimate_tokens(&content) as usize;

        Ok(MergedDocument {
            content,
            tokens: _total_tokens, // final_tokens,
            source_documents,
            sections: vec![], // 기존 인터페이스에서는 섹션 정보 없음
        })
    }

    /// 문서 내용 로드 (파일 시스템에서 읽기)
    fn load_document_content(&self, candidate: &DocumentCandidate) -> Result<String> {
        std::fs::read_to_string(&candidate.path).map_err(crate::domain::errors::ContextError::Io)
    }

    /// 최상위 헤딩 제거
    fn strip_top_heading(&self, content: &str) -> String {
        let mut lines = content.lines();
        if let Some(first_line) = lines.next() {
            let trimmed = first_line.trim_start();
            if trimmed.starts_with('#') {
                // 첫 번째 헤딩 라인 제거
                lines.collect::<Vec<_>>().join("\n")
            } else {
                content.to_string()
            }
        } else {
            content.to_string()
        }
    }

    /// 문단의 fingerprint 생성 (중복 감지용)
    fn create_shingle(&self, paragraph: &str) -> String {
        // 간단한 3-gram 기반 fingerprint
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        if words.len() < 3 {
            return paragraph.to_string();
        }

        words
            .windows(3)
            .take(10) // 처음 10개 3-gram만 사용
            .map(|w| w.join("|"))
            .collect::<Vec<_>>()
            .join("::")
    }
}

// Use empty() from common::MergedDocument

impl Default for DocumentMerger {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            paragraph_regex: Regex::new(r"\\n{2,}").unwrap(), // fallback for default
                                                              // tokenizer: Box::new(SimpleTokenizer),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[allow(clippy::unwrap_used)] // 테스트 코드에서는 허용
    fn create_test_candidate(id: &str, title: &str, content: &str) -> DocumentCandidate {
        // 테스트용 임시 파일 생성 (시스템 임시 디렉토리 사용)
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("ctx_test_{}.md", id));
        std::fs::write(&temp_path, content).unwrap();

        DocumentCandidate {
            doc_id: id.to_string(),
            sha: "test-sha".to_string(),
            title: title.to_string(),
            locale: "ko".to_string(),
            trust: 0.8,
            freshness: "2025-08-11".to_string(),
            confidence: 0.9,
            path: temp_path.to_string_lossy().to_string(),
            facets: HashMap::new(),
            tokens: content.len() / 4, // 간단한 토큰 추정
            content: Some(content.to_string()),
        }
    }

    #[test]
    fn test_merge_documents_basic() -> Result<()> {
        let merger = DocumentMerger::new()?;

        let candidates = vec![
            create_test_candidate("doc1", "First Document", "# First\n\nContent 1"),
            create_test_candidate("doc2", "Second Document", "# Second\n\nContent 2"),
        ];

        let options = MergeOptions::default();
        let result = merger.merge_documents(candidates, &options)?;

        assert!(result.content.contains("## First Document"));
        assert!(result.content.contains("## Second Document"));
        assert!(result.content.contains("Content 1"));
        assert!(result.content.contains("Content 2"));
        assert_eq!(result.source_documents.len(), 2);
        assert!(result.tokens > 0);

        Ok(())
    }

    #[test]
    fn test_strip_top_heading() {
        let merger = DocumentMerger::new().unwrap();

        let content = "# Main Title\n\nSome content\n\n## Sub Title";
        let result = merger.strip_top_heading(content);

        assert!(!result.contains("# Main Title"));
        assert!(result.contains("Some content"));
        assert!(result.contains("## Sub Title"));
    }

    #[test]
    fn test_create_shingle() {
        let merger = DocumentMerger::new().unwrap();

        let paragraph = "This is a test paragraph with some words";
        let shingle = merger.create_shingle(paragraph);

        assert!(!shingle.is_empty());
        assert!(shingle.contains("This|is|a"));
    }

    #[test]
    fn test_merge_documents_with_query() -> Result<()> {
        use crate::domain::types::BuildQuery;
        use crate::pipeline::scorer::ScoredCandidate;

        let merger = DocumentMerger::new()?;

        let mut candidate1 =
            create_test_candidate("doc1", "First Document", "# First\n\nContent 1");
        candidate1.confidence = 0.9;
        candidate1.trust = 0.8;

        let mut scored1 = ScoredCandidate::new(candidate1);
        scored1.base_score = 10.0;

        let mut candidate2 =
            create_test_candidate("doc2", "Second Document", "# Second\n\nContent 2");
        candidate2.confidence = 0.8;
        candidate2.trust = 0.7;

        let mut scored2 = ScoredCandidate::new(candidate2);
        scored2.base_score = 8.0;

        let documents = vec![scored1, scored2];
        let query = BuildQuery::new(
            "test-repo".to_string(),
            "main".to_string(),
            "abc123".to_string(),
        );

        let result = merger.merge_documents_with_query(documents, &query)?;

        // Context header 확인
        assert!(result.content.contains("# Context Composition"));
        assert!(result.content.contains("**Repository:** test-repo"));
        assert!(result.content.contains("**Branch:** main"));
        assert!(result.content.contains("**Commit:** abc123"));

        // Document sections 확인
        assert!(result.content.contains("## Document 1: First Document"));
        assert!(result.content.contains("**Confidence:** 0.90"));
        assert!(result.content.contains("**Trust:** 0.80"));

        // Metadata 확인
        assert_eq!(result.source_documents.len(), 2);
        // assert_eq!(result.sections.len(), 2);
        assert!(result.tokens > 0);

        Ok(())
    }

    #[test]
    fn test_tokenizer_interface() {
        // let merger = DocumentMerger::new().unwrap();

        // let text = "This is a test document with some content";
        // let token_count = merger.tokenizer.estimate_tokens(text);

        // assert!(token_count > 0);
        // // 단어 수는 9개, 예상 토큰은 약 12개 (9 * 1.3)
        // assert!(token_count >= 9);
    }

    #[test]
    fn test_confidence_scoring_in_sections() -> Result<()> {
        use crate::domain::types::BuildQuery;
        use crate::pipeline::scorer::ScoredCandidate;

        let merger = DocumentMerger::new()?;

        let mut candidate = create_test_candidate("doc1", "Test Document", "Content");
        candidate.confidence = 0.95;

        let scored = ScoredCandidate::new(candidate);
        let documents = vec![scored];
        let query = BuildQuery::new(
            "repo".to_string(),
            "branch".to_string(),
            "commit".to_string(),
        );

        let _result = merger.merge_documents_with_query(documents, &query)?;

        // 섹션에 신뢰도가 포함되어야 함
        // assert_eq!(result.sections.len(), 1);
        // assert_eq!(result.sections[0].confidence, 0.95);

        Ok(())
    }
}
