use sha2::{Digest, Sha256};
use uuid::{Uuid, uuid};

/// ctxset 전용 네임스페이스 UUID
#[allow(dead_code)]
static NAMESPACE_CTXSET: Uuid = uuid!("2d2e4f1c-34d6-4f2b-9f6e-0a7e7ce6f3a2");

/// ID 생성 유틸리티
#[allow(dead_code)]
pub struct IdGenerator;

impl IdGenerator {
    /// 콘텐츠의 SHA-256 해시 생성
    #[allow(dead_code)]
    pub fn content_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 안정적인 문서 ID 생성
    ///
    /// 규칙: doc_id = UUIDv5(NAMESPACE_CTXSET, "${repo}@${first_add_commit_sha}:${initial_title_or_hash}")
    ///
    /// # Arguments
    /// * `repo` - 저장소 경로 또는 이름
    /// * `first_add_sha` - 파일이 최초로 추가된 커밋 SHA
    /// * `initial_title_or_hash` - 초기 제목 또는 콘텐츠 해시
    #[allow(dead_code)]
    pub fn stable_doc_id(repo: &str, first_add_sha: &str, initial_title_or_hash: &str) -> String {
        let name = format!("{}@{}:{}", repo, first_add_sha, initial_title_or_hash);
        let uuid = Uuid::new_v5(&NAMESPACE_CTXSET, name.as_bytes());
        format!("doc/{}", uuid)
    }

    /// frontmatter에서 제목 추출 또는 콘텐츠 해시 생성
    #[allow(dead_code)]
    pub fn extract_title_or_hash(title: Option<&str>, content: &str) -> String {
        match title {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => Self::content_hash(content),
        }
    }

    /// 마크다운에서 첫 번째 헤딩 추출
    #[allow(dead_code)]
    pub fn extract_first_heading(content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let heading = trimmed.trim_start_matches('#').trim();
                if !heading.is_empty() {
                    return Some(heading.to_string());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash() {
        let content = "Hello, World!";
        let hash = IdGenerator::content_hash(content);

        // SHA-256 of "Hello, World!" should be consistent
        assert_eq!(hash.len(), 64); // SHA-256 produces 64 hex characters

        // Same content should produce same hash
        let hash2 = IdGenerator::content_hash(content);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_stable_doc_id() {
        let repo = "test-repo";
        let first_sha = "abc123";
        let title = "Test Document";

        let id1 = IdGenerator::stable_doc_id(repo, first_sha, title);
        let id2 = IdGenerator::stable_doc_id(repo, first_sha, title);

        // Same inputs should produce same ID
        assert_eq!(id1, id2);
        assert!(id1.starts_with("doc/"));

        // Different inputs should produce different IDs
        let id3 = IdGenerator::stable_doc_id(repo, "def456", title);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_extract_title_or_hash() {
        // With title
        let result = IdGenerator::extract_title_or_hash(Some("My Title"), "content");
        assert_eq!(result, "My Title");

        // With empty title
        let result = IdGenerator::extract_title_or_hash(Some(""), "content");
        assert_eq!(result, IdGenerator::content_hash("content"));

        // Without title
        let result = IdGenerator::extract_title_or_hash(None, "content");
        assert_eq!(result, IdGenerator::content_hash("content"));
    }

    #[test]
    fn test_extract_first_heading() {
        let content = r#"
Some text before

# Main Heading

Some content

## Sub Heading
"#;

        let heading = IdGenerator::extract_first_heading(content);
        assert_eq!(heading, Some("Main Heading".to_string()));

        // No heading
        let content_no_heading = "Just some text without headings";
        let heading = IdGenerator::extract_first_heading(content_no_heading);
        assert_eq!(heading, None);
    }
}
