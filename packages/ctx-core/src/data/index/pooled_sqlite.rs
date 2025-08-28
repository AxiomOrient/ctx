//! 커넥션 풀링을 사용하는 SQLite 인덱스 매니저
//!
//! 기존 SqliteIndexManager의 성능 최적화 버전

use crate::Result;
use crate::common::errors::ContextError;
use crate::common::{BuildQuery, DocumentCandidate};
use crate::data::index::pool::{PoolConfig, SqlitePool, get_default_pool};
use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;
use std::sync::Arc;

/// Type alias for document facets mapping: (doc_path, doc_type) -> facet_type -> values
type DocumentFacetsMap = HashMap<(String, String), HashMap<String, Vec<String>>>;

/// Type alias for document candidate tuple: (path, type, id, sha, score, title, facet_score, created_at)
type DocumentCandidateTuple = (String, String, String, String, f32, String, f32, String);

/// 풀링된 SQLite 인덱스 관리자
pub struct PooledSqliteIndexManager {
    pool: Arc<SqlitePool>,
}

impl PooledSqliteIndexManager {
    /// 새로운 풀링된 인덱스 관리자 생성
    pub fn new(db_path: &str, config: Option<PoolConfig>) -> Result<Self> {
        let pool = if let Some(config) = config {
            crate::data::index::pool::global_pool_manager()
                .get_or_create_pool(db_path, Some(config))?
        } else {
            get_default_pool(db_path)?
        };

        Ok(Self { pool })
    }

    /// 기본 설정으로 생성
    pub fn new_default(db_path: &str) -> Result<Self> {
        Self::new(db_path, None)
    }

    /// 커넥션 풀 상태 확인
    pub fn pool_state(&self) -> crate::data::index::pool::PoolState {
        self.pool.state()
    }

    /// 마지막 인덱싱된 커밋 SHA 조회
    pub fn get_last_indexed_sha(&self, repo: &str, branch: &str) -> Result<Option<String>> {
        let conn = self.pool.get()?;
        let mut stmt =
            conn.prepare("SELECT last_indexed_sha FROM index_state WHERE repo=?1 AND branch=?2")?;

        let sha = stmt
            .query_row(params![repo, branch], |r| r.get::<_, Option<String>>(0))
            .optional()
            .map_err(ContextError::DatabaseError)?;

        Ok(sha.flatten())
    }

    /// 마지막 인덱싱된 커밋 SHA 설정
    pub fn set_last_indexed_sha(&self, repo: &str, branch: &str, sha: &str) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO index_state(repo,branch,last_indexed_sha) VALUES(?,?,?)
             ON CONFLICT(repo,branch) DO UPDATE SET last_indexed_sha=excluded.last_indexed_sha",
            params![repo, branch, sha],
        )?;

        Ok(())
    }

    /// 문서 삭제 (soft delete)
    pub fn delete_document(&self, doc_id: &str) -> Result<bool> {
        let conn = self.pool.get()?;
        let rows_affected = conn.execute(
            "UPDATE docs SET deleted=1, valid_to=?1 WHERE doc_id=?2 AND valid_to IS NULL",
            params![Utc::now().to_rfc3339(), doc_id],
        )?;

        Ok(rows_affected > 0)
    }

    /// 모든 데이터 삭제 (완전 삭제)
    pub fn clear_all_data(&self) -> Result<()> {
        let mut conn = self.pool.get()?;
        let tx = conn.unchecked_transaction()?;

        // 모든 테이블의 데이터 삭제 (스키마는 유지)
        tx.execute("DELETE FROM doc_facets", [])?;
        tx.execute("DELETE FROM facet_values", [])?;
        tx.execute("DELETE FROM classify_log", [])?;
        tx.execute("DELETE FROM docs", [])?;
        tx.execute("DELETE FROM index_state", [])?;
        tx.execute("DELETE FROM execution_cache", [])?;
        tx.execute("DELETE FROM similarity_cache", [])?;

        // SQLite 최적화
        tx.execute("VACUUM", [])?;

        tx.commit().map_err(ContextError::DatabaseError)?;
        Ok(())
    }

    /// 문서 인덱스 업서트 (배치 처리 최적화)
    pub fn upsert_document(&self, doc: &DocumentIndex) -> Result<()> {
        let mut conn = self.pool.get()?;
        let tx = conn.unchecked_transaction()?;

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

        // 패싯 매핑 - 준비된 구문으로 최적화
        {
            let mut facet_values_stmt =
                tx.prepare("INSERT OR IGNORE INTO facet_values(namespace,value) VALUES(?,?)")?;
            let mut doc_facets_stmt = tx.prepare(
                "INSERT OR REPLACE INTO doc_facets(doc_id,sha,namespace,value) VALUES(?,?,?,?)",
            )?;

            for (namespace, values) in &doc.facets {
                for value in values {
                    facet_values_stmt.execute(params![namespace, value])?;
                    doc_facets_stmt.execute(params![&doc.doc_id, &doc.sha, namespace, value])?;
                }
            }
        } // 명시적으로 prepared statements drop

        // 분류 로그
        let warnings_json =
            serde_json::to_string(&doc.warnings).map_err(ContextError::JsonError)?;
        let errors_json = serde_json::to_string(&doc.errors).map_err(ContextError::JsonError)?;

        tx.execute(
            "INSERT OR REPLACE INTO classify_log(doc_id,sha,confidence,warnings,errors,ontology_version,rules_version) VALUES(?,?,?,?,?,?,?)",
            params![
                &doc.doc_id, &doc.sha, doc.confidence,
                warnings_json, errors_json,
                &doc.ontology_version, &doc.rules_version
            ]
        )?;

        tx.commit().map_err(ContextError::DatabaseError)?;
        Ok(())
    }

    /// 배치 문서 업서트 (대량 데이터 처리용)
    pub fn upsert_documents_batch(&self, docs: &[DocumentIndex]) -> Result<()> {
        if docs.is_empty() {
            return Ok(());
        }

        let mut conn = self.pool.get()?;
        let tx = conn.unchecked_transaction()?;

        // 모든 이전 스냅샷 닫기
        for doc in docs {
            tx.execute(
                "UPDATE docs SET valid_to=?1 WHERE doc_id=?2 AND valid_to IS NULL",
                params![Utc::now().to_rfc3339(), &doc.doc_id],
            )?;
        }

        // 준비된 구문들 - 스코프로 묶어서 트랜잭션 commit 전에 drop
        {
            let mut docs_stmt = tx.prepare(
                r#"INSERT INTO docs(doc_id,repo,branch,path,sha,content_hash,title,locale,trust,freshness,source_type,maturity,valid_from,valid_to,deleted)
                   VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,NULL,0)"#
            )?;
            let mut facet_values_stmt =
                tx.prepare("INSERT OR IGNORE INTO facet_values(namespace,value) VALUES(?,?)")?;
            let mut doc_facets_stmt = tx.prepare(
                "INSERT OR REPLACE INTO doc_facets(doc_id,sha,namespace,value) VALUES(?,?,?,?)",
            )?;
            let mut classify_log_stmt = tx.prepare(
                "INSERT OR REPLACE INTO classify_log(doc_id,sha,confidence,warnings,errors,ontology_version,rules_version) VALUES(?,?,?,?,?,?,?)"
            )?;

            for doc in docs {
                // 문서 삽입
                docs_stmt.execute(params![
                    &doc.doc_id,
                    &doc.repo,
                    &doc.branch,
                    &doc.path,
                    &doc.sha,
                    &doc.content_hash,
                    &doc.title,
                    &doc.locale,
                    doc.trust,
                    &doc.freshness,
                    &doc.source_type,
                    &doc.maturity,
                    Utc::now().to_rfc3339()
                ])?;

                // 패싯 매핑
                for (namespace, values) in &doc.facets {
                    for value in values {
                        facet_values_stmt.execute(params![namespace, value])?;
                        doc_facets_stmt.execute(params![
                            &doc.doc_id,
                            &doc.sha,
                            namespace,
                            value
                        ])?;
                    }
                }

                // 분류 로그
                let warnings_json =
                    serde_json::to_string(&doc.warnings).map_err(ContextError::JsonError)?;
                let errors_json =
                    serde_json::to_string(&doc.errors).map_err(ContextError::JsonError)?;

                classify_log_stmt.execute(params![
                    &doc.doc_id,
                    &doc.sha,
                    doc.confidence,
                    warnings_json,
                    errors_json,
                    &doc.ontology_version,
                    &doc.rules_version
                ])?;
            }
        } // 명시적으로 prepared statements drop

        tx.commit().map_err(ContextError::DatabaseError)?;
        Ok(())
    }

    /// 특정 커밋의 후보 문서들 조회 (최적화된 버전)
    pub fn load_candidates(&self, query: &BuildQuery) -> Result<Vec<DocumentCandidate>> {
        let conn = self.pool.get()?;

        // 인덱스가 있는 컬럼부터 필터링하여 쿼리 최적화
        let mut stmt = conn.prepare(
            "SELECT d.doc_id,d.sha,d.title,d.locale,COALESCE(d.trust,0.8),COALESCE(d.freshness,'1970-01-01'),
                    COALESCE(cl.confidence,0.5),d.path
             FROM docs d
             LEFT JOIN classify_log cl ON cl.doc_id=d.doc_id AND cl.sha=d.sha
             WHERE d.repo=?1 AND d.branch=?2 AND d.sha=?3 AND d.deleted=0
               AND (?4 IS NULL OR d.locale=?4)
               AND (?5 IS NULL OR d.maturity=?5)
             ORDER BY d.trust DESC, cl.confidence DESC"
        )?;

        let mut rows = stmt
            .query(params![
                query.repo,
                query.branch,
                query.commit_sha,
                query.lang,
                query.maturity
            ])
            .map_err(ContextError::DatabaseError)?;

        let mut candidates = Vec::new();
        while let Some(row) = rows.next().map_err(ContextError::DatabaseError)? {
            let doc_id: String = row.get(0).map_err(ContextError::DatabaseError)?;
            let sha: String = row.get(1).map_err(ContextError::DatabaseError)?;
            let title: String = row.get(2).map_err(ContextError::DatabaseError)?;
            let locale: String = row.get(3).map_err(ContextError::DatabaseError)?;
            let trust: f32 = row.get(4).map_err(ContextError::DatabaseError)?;
            let freshness: String = row.get(5).map_err(ContextError::DatabaseError)?;
            let confidence: f32 = row.get(6).map_err(ContextError::DatabaseError)?;
            let path: String = row.get(7).map_err(ContextError::DatabaseError)?;

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

    /// 모든 문서의 패싯을 한 번에 로드 (최적화된 IN 쿼리)
    fn load_all_document_facets(
        &self,
        candidates: &[DocumentCandidateTuple],
    ) -> Result<DocumentFacetsMap> {
        if candidates.is_empty() {
            return Ok(HashMap::new());
        }

        let conn = self.pool.get()?;

        // IN 절을 위한 플레이스홀더 생성
        let placeholders: Vec<String> = candidates.iter().map(|_| "(?,?)".to_string()).collect();
        let query = format!(
            "SELECT doc_id, sha, namespace, value FROM doc_facets WHERE (doc_id, sha) IN ({}) ORDER BY doc_id, sha, namespace",
            placeholders.join(",")
        );

        let mut stmt = conn.prepare(&query)?;

        // 파라미터 바인딩
        let mut params = Vec::new();
        for (doc_id, sha, _, _, _, _, _, _) in candidates {
            params.push(doc_id as &dyn rusqlite::ToSql);
            params.push(sha as &dyn rusqlite::ToSql);
        }

        let mut rows = stmt.query(&*params).map_err(ContextError::DatabaseError)?;
        let mut all_facets: DocumentFacetsMap = HashMap::new();

        while let Some(row) = rows.next().map_err(ContextError::DatabaseError)? {
            let doc_id: String = row.get(0).map_err(ContextError::DatabaseError)?;
            let sha: String = row.get(1).map_err(ContextError::DatabaseError)?;
            let namespace: String = row.get(2).map_err(ContextError::DatabaseError)?;
            let value: String = row.get(3).map_err(ContextError::DatabaseError)?;

            all_facets
                .entry((doc_id, sha))
                .or_default()
                .entry(namespace)
                .or_default()
                .push(value);
        }

        Ok(all_facets)
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
        let conn = self.pool.get()?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO execution_cache 
             (execution_id, canonical_input_hash, context_hash, result_json, created_at, last_accessed) 
             VALUES (?, ?, ?, ?, ?, ?)",
            params![execution_id, canonical_input_hash, context_hash, result_json, now, now],
        )?;
        Ok(())
    }

    /// 실행 결과를 캐시에서 조회
    pub fn get_execution_result(&self, execution_id: &str) -> Result<Option<String>> {
        let conn = self.pool.get()?;
        let mut stmt =
            conn.prepare("SELECT result_json FROM execution_cache WHERE execution_id = ?")?;

        let result = stmt
            .query_row(params![execution_id], |row| row.get::<_, String>(0))
            .optional()
            .map_err(ContextError::DatabaseError)?;

        // 액세스 카운트 업데이트
        if result.is_some() {
            let now = Utc::now().to_rfc3339();
            conn.execute(
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
        let conn = self.pool.get()?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
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
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            &format!("SELECT similar_hash, similarity_score FROM similarity_cache 
             WHERE input_hash = ? AND similarity_score >= ? 
             ORDER BY similarity_score DESC LIMIT {}", 
             crate::common::constants::defaults::MAX_SIMILARITY_RESULTS),
        )?;

        let rows = stmt
            .query_map(params![input_hash, threshold], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
            })
            .map_err(ContextError::DatabaseError)?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(ContextError::DatabaseError)?);
        }

        Ok(results)
    }

    /// 만료된 캐시 항목들 정리
    pub fn cleanup_expired_cache(&self, hours: i64) -> Result<usize> {
        let conn = self.pool.get()?;
        let cutoff = Utc::now() - chrono::Duration::hours(hours);
        let cutoff_str = cutoff.to_rfc3339();

        let deleted = conn.execute(
            "DELETE FROM execution_cache WHERE created_at < ?",
            params![cutoff_str],
        )?;

        // 유사성 캐시도 정리 (더 오래된 것들)
        let similarity_cutoff = Utc::now() - chrono::Duration::hours(hours * 2);
        let similarity_cutoff_str = similarity_cutoff.to_rfc3339();

        let similarity_deleted = conn.execute(
            "DELETE FROM similarity_cache WHERE created_at < ?",
            params![similarity_cutoff_str],
        )?;

        Ok(deleted + similarity_deleted)
    }

    /// 데이터베이스 통계 조회
    pub fn get_database_stats(&self) -> Result<DatabaseStats> {
        let conn = self.pool.get()?;

        let total_docs: i64 = conn
            .connection()
            .query_row("SELECT COUNT(*) FROM docs WHERE deleted=0", [], |row| {
                row.get(0)
            })
            .map_err(ContextError::DatabaseError)?;

        let total_facets: i64 = conn
            .connection()
            .query_row(
                "SELECT COUNT(DISTINCT namespace) FROM facet_values",
                [],
                |row| row.get(0),
            )
            .map_err(ContextError::DatabaseError)?;

        let cache_entries: i64 = conn
            .connection()
            .query_row("SELECT COUNT(*) FROM execution_cache", [], |row| row.get(0))
            .map_err(ContextError::DatabaseError)?;

        let similarity_entries: i64 = conn
            .connection()
            .query_row("SELECT COUNT(*) FROM similarity_cache", [], |row| {
                row.get(0)
            })
            .map_err(ContextError::DatabaseError)?;

        Ok(DatabaseStats {
            total_documents: total_docs as usize,
            total_facets: total_facets as usize,
            cache_entries: cache_entries as usize,
            similarity_entries: similarity_entries as usize,
            pool_state: self.pool_state(),
        })
    }

    /// 데이터베이스 최적화
    pub fn optimize_database(&self) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute_batch("PRAGMA optimize; ANALYZE; PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}

/// 문서 인덱스 엔트리 (기존과 동일)
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

/// 데이터베이스 통계
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_documents: usize,
    pub total_facets: usize,
    pub cache_entries: usize,
    pub similarity_entries: usize,
    pub pool_state: crate::data::index::pool::PoolState,
}

#[cfg(test)]
mod tests {
    use super::*;
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
    #[allow(clippy::expect_used)]
    fn test_pooled_manager_creation() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let manager = PooledSqliteIndexManager::new_default(temp_file.path().to_str().unwrap())
            .expect("Failed to create manager");

        let state = manager.pool_state();
        assert!(state.max_connections > 0);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_batch_upsert() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let manager = PooledSqliteIndexManager::new_default(temp_file.path().to_str().unwrap())
            .expect("Failed to create manager");

        let docs = vec![create_test_document(), {
            let mut doc = create_test_document();
            doc.doc_id = "test-doc-2".to_string();
            doc.title = "Test Document 2".to_string();
            doc
        }];

        manager
            .upsert_documents_batch(&docs)
            .expect("Failed to batch upsert");

        let stats = manager.get_database_stats().expect("Failed to get stats");
        assert_eq!(stats.total_documents, 2);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_caching() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let manager = PooledSqliteIndexManager::new_default(temp_file.path().to_str().unwrap())
            .expect("Failed to create manager");

        // 실행 결과 캐싱 테스트
        manager
            .store_execution_result("exec1", "input1", "context1", r#"{"result":"test"}"#)
            .expect("Failed to store execution result");

        let result = manager
            .get_execution_result("exec1")
            .expect("Failed to get execution result");
        assert_eq!(result, Some(r#"{"result":"test"}"#.to_string()));

        // 유사성 캐싱 테스트
        manager
            .store_similarity_score("input1", "similar1", 0.85)
            .expect("Failed to store similarity score");

        let similar = manager
            .get_similar_inputs("input1", 0.8)
            .expect("Failed to get similar inputs");
        assert_eq!(similar.len(), 1);
        assert_eq!(similar[0].0, "similar1");
        assert_eq!(similar[0].1, 0.85);
    }
}
