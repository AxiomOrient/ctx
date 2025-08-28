use crate::Result;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;

/// SQLite 기반 문서 인덱스 관리자
pub struct SqliteIndexManager {
    conn: Connection,
}

impl SqliteIndexManager {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // WAL 모드 설정
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        let manager = Self { conn };
        manager.ensure_schema()?;
        Ok(manager)
    }

    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(include_str!("sqlite_schema.sql"))?;
        Ok(())
    }

    /// Expose read-only reference to underlying connection (for tests/utilities)
    pub fn get_connection(&self) -> &Connection {
        &self.conn
    }

    /// 마지막 인덱싱된 커밋 SHA 조회
    pub fn get_last_indexed_sha(&self, repo: &str, branch: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT last_indexed_sha FROM index_state WHERE repo=?1 AND branch=?2")?;

        let sha = stmt
            .query_row(params![repo, branch], |r| r.get::<_, Option<String>>(0))
            .optional()?;

        Ok(sha.flatten())
    }

    /// 마지막 인덱싱된 커밋 SHA 설정
    pub fn set_last_indexed_sha(&self, repo: &str, branch: &str, sha: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO index_state(repo,branch,last_indexed_sha) VALUES(?,?,?)
             ON CONFLICT(repo,branch) DO UPDATE SET last_indexed_sha=excluded.last_indexed_sha",
            params![repo, branch, sha],
        )?;

        Ok(())
    }

    /// 문서 삭제 (soft delete)
    pub fn delete_document(&self, doc_id: &str) -> Result<bool> {
        let rows_affected = self.conn.execute(
            "UPDATE docs SET deleted=1, valid_to=?1 WHERE doc_id=?2 AND valid_to IS NULL",
            params![Utc::now().to_rfc3339(), doc_id],
        )?;

        Ok(rows_affected > 0)
    }

    /// 모든 데이터 삭제 (완전 삭제)
    pub fn clear_all_data(&self) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        // 모든 테이블의 데이터 삭제 (스키마는 유지)
        tx.execute("DELETE FROM doc_facets", [])?;
        tx.execute("DELETE FROM facet_values", [])?;
        tx.execute("DELETE FROM classify_log", [])?;
        tx.execute("DELETE FROM docs", [])?;
        tx.execute("DELETE FROM index_state", [])?;

        // SQLite 최적화
        tx.execute("VACUUM", [])?;

        tx.commit()?;
        Ok(())
    }

    /// 문서 인덱스 업서트
    pub fn upsert_document(&self, doc: &DocumentIndex) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        // 이전 스냅샷 닫기
        tx.execute(
            "UPDATE docs SET valid_to=?1 WHERE doc_id=?2 AND valid_to IS NULL",
            params![Utc::now().to_rfc3339(), &doc.doc_id],
        )?;

        // 새 스냅샷 삽입
        tx.execute(
            r#"INSERT INTO docs(doc_id,repo,branch,path,sha,content_hash,title,locale,trust,freshness,source_type,maturity,valid_from,valid_to,deleted)
               VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,NULL,0)"#,
            params![
                &doc.doc_id, &doc.repo, &doc.branch, &doc.path, &doc.sha, &doc.content_hash,
                &doc.title, &doc.locale, doc.trust, &doc.freshness, &doc.source_type, &doc.maturity,
                Utc::now().to_rfc3339()
            ]
        )?;

        // 패싯 매핑
        for (namespace, values) in &doc.facets {
            for value in values {
                tx.execute(
                    "INSERT OR IGNORE INTO facet_values(namespace,value) VALUES(?,?)",
                    params![namespace, value],
                )?;

                tx.execute(
                    "INSERT OR REPLACE INTO doc_facets(doc_id,sha,namespace,value) VALUES(?,?,?,?)",
                    params![&doc.doc_id, &doc.sha, namespace, value],
                )?;
            }
        }

        // 분류 로그 - JSON 직렬화 에러를 적절히 처리
        let warnings_json =
            serde_json::to_string(&doc.warnings).map_err(crate::ContextError::JsonError)?;
        let errors_json =
            serde_json::to_string(&doc.errors).map_err(crate::ContextError::JsonError)?;

        tx.execute(
            "INSERT OR REPLACE INTO classify_log(doc_id,sha,confidence,warnings,errors,ontology_version,rules_version) VALUES(?,?,?,?,?,?,?)",
            params![
                &doc.doc_id, &doc.sha, doc.confidence,
                warnings_json, errors_json,
                &doc.ontology_version, &doc.rules_version
            ]
        )?;

        tx.commit()?;

        Ok(())
    }

    /// 특정 커밋의 후보 문서들 조회
    pub fn load_candidates(&self, query: &BuildQuery) -> Result<Vec<DocumentCandidate>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.doc_id,d.sha,d.title,d.locale,COALESCE(d.trust,0.8),COALESCE(d.freshness,'1970-01-01'),
                    COALESCE(cl.confidence,0.5),d.path
             FROM docs d
             LEFT JOIN classify_log cl ON cl.doc_id=d.doc_id AND cl.sha=d.sha
             WHERE d.repo=?1 AND d.branch=?2 AND d.sha=?3 AND d.deleted=0
               AND (?4 IS NULL OR d.locale=?4)
               AND (?5 IS NULL OR d.maturity=?5)"
        )?;

        let mut rows = stmt.query(params![
            query.repo,
            query.branch,
            query.commit_sha,
            query.lang,
            query.maturity
        ])?;

        let mut candidates = Vec::new();
        while let Some(row) = rows.next()? {
            let doc_id: String = row.get(0)?;
            let sha: String = row.get(1)?;
            let title: String = row.get(2)?;
            let locale: String = row.get(3)?;
            let trust: f32 = row.get(4)?;
            let freshness: String = row.get(5)?;
            let confidence: f32 = row.get(6)?;
            let path: String = row.get(7)?;

            candidates.push((
                doc_id, sha, title, locale, trust, freshness, confidence, path,
            ));
        }

        // 모든 문서의 패싯을 한 번에 로드 (N+1 쿼리 해결)
        let all_facets = self.load_all_document_facets(&candidates)?;

        // 최종 결과 구성
        let mut final_candidates = Vec::new();
        for (doc_id, sha, title, locale, trust, freshness, confidence, path) in candidates {
            let facets = all_facets
                .get(&(doc_id.clone(), sha.clone()))
                .cloned()
                .unwrap_or_default();

            // 필수 패싯 검사
            if self.matches_required_facets(&facets, &query.required_facets) {
                // 파일에서 토큰 계산 (간단 추정)
                let tokens = std::fs::read_to_string(&path)
                    .ok()
                    .map(|s| {
                        let words = s.split_whitespace().count();
                        ((words as f32)
                            * crate::common::constants::defaults::TOKEN_ESTIMATION_MULTIPLIER)
                            .ceil() as usize
                    })
                    .unwrap_or(0);

                final_candidates.push(DocumentCandidate {
                    doc_id,
                    sha,
                    title,
                    locale,
                    trust,
                    freshness,
                    confidence,
                    path,
                    facets,
                    tokens,
                    content: None, // 지연 로딩
                });
            }
        }

        Ok(final_candidates)
    }

    /// 모든 문서의 패싯을 한 번에 로드 (N+1 쿼리 해결)
    fn load_all_document_facets(&self, candidates: &[CandidateTuple]) -> Result<DocumentFacetMap> {
        if candidates.is_empty() {
            return Ok(HashMap::new());
        }

        // IN 절을 위한 플레이스홀더 생성
        let placeholders: Vec<String> = candidates.iter().map(|_| "(?,?)".to_string()).collect();
        let query = format!(
            "SELECT doc_id, sha, namespace, value FROM doc_facets WHERE (doc_id, sha) IN ({})",
            placeholders.join(",")
        );

        let mut stmt = self.conn.prepare(&query)?;

        // 파라미터 바인딩
        let mut params = Vec::new();
        for (doc_id, sha, _, _, _, _, _, _) in candidates {
            params.push(doc_id as &dyn rusqlite::ToSql);
            params.push(sha as &dyn rusqlite::ToSql);
        }

        let mut rows = stmt.query(&*params)?;
        let mut all_facets: DocumentFacetMap = HashMap::new();

        while let Some(row) = rows.next()? {
            let doc_id: String = row.get(0)?;
            let sha: String = row.get(1)?;
            let namespace: String = row.get(2)?;
            let value: String = row.get(3)?;

            all_facets
                .entry((doc_id, sha))
                .or_default()
                .entry(namespace)
                .or_default()
                .push(value);
        }

        Ok(all_facets)
    }

    #[allow(dead_code)]
    fn load_document_facets(
        &self,
        doc_id: &str,
        sha: &str,
    ) -> Result<HashMap<String, Vec<String>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT namespace,value FROM doc_facets WHERE doc_id=?1 AND sha=?2")?;

        let mut rows = stmt.query(params![doc_id, sha])?;

        let mut facets: HashMap<String, Vec<String>> = HashMap::new();
        while let Some(row) = rows.next()? {
            let namespace: String = row.get(0)?;
            let value: String = row.get(1)?;

            facets.entry(namespace).or_default().push(value);
        }

        Ok(facets)
    }

    fn matches_required_facets(
        &self,
        facets: &HashMap<String, Vec<String>>,
        required: &[(String, Vec<String>)],
    ) -> bool {
        for (namespace, allowed_values) in required {
            if let Some(doc_values) = facets.get(namespace) {
                if allowed_values.is_empty() {
                    // 네임스페이스만 있으면 됨
                    continue;
                } else {
                    // 허용된 값 중 하나라도 있어야 함
                    if !allowed_values.iter().any(|v| doc_values.contains(v)) {
                        return false;
                    }
                }
            } else {
                return false;
            }
        }
        true
    }

    /// 실행 결과를 캐시에 저장
    pub fn store_execution_result(
        &self,
        execution_id: &str,
        canonical_input_hash: &str,
        context_hash: &str,
        result_json: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO execution_cache 
             (execution_id, canonical_input_hash, context_hash, result_json, created_at, last_accessed) 
             VALUES (?, ?, ?, ?, ?, ?)",
            params![execution_id, canonical_input_hash, context_hash, result_json, now, now],
        )?;
        Ok(())
    }

    /// 실행 결과를 캐시에서 조회
    pub fn get_execution_result(&self, execution_id: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT result_json FROM execution_cache WHERE execution_id = ?")?;

        let result = stmt
            .query_row(params![execution_id], |row| row.get::<_, String>(0))
            .optional()?;

        // 액세스 카운트 업데이트
        if result.is_some() {
            let now = Utc::now().to_rfc3339();
            self.conn.execute(
                "UPDATE execution_cache SET access_count = access_count + 1, last_accessed = ? WHERE execution_id = ?",
                params![now, execution_id],
            )?;
        }

        Ok(result)
    }

    /// 유사성 점수를 캐시에 저장
    pub fn store_similarity_score(
        &self,
        input_hash: &str,
        similar_hash: &str,
        similarity_score: f32,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO similarity_cache 
             (input_hash, similar_hash, similarity_score, created_at) 
             VALUES (?, ?, ?, ?)",
            params![input_hash, similar_hash, similarity_score, now],
        )?;
        Ok(())
    }

    /// 유사한 입력들을 캐시에서 조회
    pub fn get_similar_inputs(
        &self,
        input_hash: &str,
        threshold: f32,
    ) -> Result<Vec<(String, f32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT similar_hash, similarity_score FROM similarity_cache 
             WHERE input_hash = ? AND similarity_score >= ? 
             ORDER BY similarity_score DESC",
        )?;

        let rows = stmt.query_map(params![input_hash, threshold], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    /// 만료된 캐시 항목들 정리
    pub fn cleanup_expired_cache(&self, hours: i64) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::hours(hours);
        let cutoff_str = cutoff.to_rfc3339();

        let deleted = self.conn.execute(
            "DELETE FROM execution_cache WHERE created_at < ?",
            params![cutoff_str],
        )?;

        // 유사성 캐시도 정리 (더 오래된 것들)
        let similarity_cutoff = Utc::now() - chrono::Duration::hours(hours * 2);
        let similarity_cutoff_str = similarity_cutoff.to_rfc3339();

        let similarity_deleted = self.conn.execute(
            "DELETE FROM similarity_cache WHERE created_at < ?",
            params![similarity_cutoff_str],
        )?;

        Ok(deleted + similarity_deleted)
    }
}

/// 문서 인덱스 엔트리
#[derive(Debug, Clone)]
pub struct DocumentIndex {
    pub doc_id: String,
    pub repo: String,
    pub branch: String,
    pub path: String,
    pub sha: String,
    pub content_hash: String,
    pub title: String,
    pub locale: String,
    pub trust: f32,
    pub freshness: String,
    pub source_type: String,
    pub maturity: String,
    pub facets: HashMap<String, Vec<String>>,
    pub confidence: f32,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub ontology_version: String,
    pub rules_version: String,
}

use crate::common::{BuildQuery, DocumentCandidate};

type CandidateTuple = (String, String, String, String, f32, String, f32, String);
type DocumentFacetMap = HashMap<(String, String), HashMap<String, Vec<String>>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContextError;
    use std::collections::HashMap;
    use tempfile::NamedTempFile;

    fn create_test_document() -> DocumentIndex {
        let mut facets = HashMap::new();
        facets.insert("language".to_string(), vec!["rust".to_string()]);
        facets.insert("framework".to_string(), vec!["tokio".to_string()]);

        DocumentIndex {
            doc_id: "test-doc".to_string(),
            repo: "test-repo".to_string(),
            branch: "main".to_string(),
            path: "/test/doc.md".to_string(),
            sha: "abc123".to_string(),
            content_hash: "hash123".to_string(),
            title: "Test Document".to_string(),
            locale: "en".to_string(),
            trust: 0.8,
            freshness: "2024-01-01".to_string(),
            source_type: "markdown".to_string(),
            maturity: "stable".to_string(),
            facets,
            confidence: 0.9,
            warnings: vec!["test warning".to_string()],
            errors: vec![],
            ontology_version: "1.0".to_string(),
            rules_version: "1.0".to_string(),
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_sqlite_manager_creation() -> Result<()> {
        let temp_file = NamedTempFile::new().map_err(ContextError::Io)?;
        let manager = SqliteIndexManager::new(temp_file.path().to_str().unwrap())?;

        // 스키마가 생성되었는지 확인
        let result = manager.get_last_indexed_sha("test", "main")?;
        assert_eq!(result, None);

        Ok(())
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_upsert_and_retrieve_document() -> Result<()> {
        let temp_file = NamedTempFile::new().map_err(ContextError::Io)?;
        let manager = SqliteIndexManager::new(temp_file.path().to_str().unwrap())?;

        let doc = create_test_document();
        manager.upsert_document(&doc)?;

        // 인덱스 상태 설정
        manager.set_last_indexed_sha("test-repo", "main", "abc123")?;

        // 조회 테스트
        let query = BuildQuery {
            repo: "test-repo".to_string(),
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
            lang: Some("en".to_string()),
            maturity: Some("stable".to_string()),
            required_facets: vec![],
            axes: None,
            query_text: None,
            include_sources: None,
            confidence_threshold: 0.5,
        };

        let candidates = manager.load_candidates(&query)?;
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].doc_id, "test-doc");
        assert_eq!(candidates[0].title, "Test Document");

        Ok(())
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_last_indexed_sha() -> Result<()> {
        let temp_file = NamedTempFile::new().map_err(ContextError::Io)?;
        let manager = SqliteIndexManager::new(temp_file.path().to_str().unwrap())?;

        // 초기 상태
        let result = manager.get_last_indexed_sha("test-repo", "main")?;
        assert_eq!(result, None);

        // SHA 설정
        manager.set_last_indexed_sha("test-repo", "main", "abc123")?;
        let result = manager.get_last_indexed_sha("test-repo", "main")?;
        assert_eq!(result, Some("abc123".to_string()));

        // 업데이트
        manager.set_last_indexed_sha("test-repo", "main", "def456")?;
        let result = manager.get_last_indexed_sha("test-repo", "main")?;
        assert_eq!(result, Some("def456".to_string()));

        Ok(())
    }
}
