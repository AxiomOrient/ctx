//! SQLite-based document indexing and storage
//!
//! Provides core indexing functionality using SQLx and FTS5 for full-text search.

use crate::domain::errors::{ContextError, Result};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use serde_json;
use std::collections::HashMap;

/// SQLite-based document index manager
#[derive(Debug, Clone)]
pub struct SqliteIndexManager {
    pool: SqlitePool,
}

impl SqliteIndexManager {
    /// Create new SqliteIndexManager with database connection
    pub async fn new(database_url: &str) -> Result<Self> {
        // Initialize database connection pool
        let pool = SqlitePool::connect(database_url)
            .await
            .map_err(|e| ContextError::Other(format!("Failed to connect to database: {}", e)))?;

        // Run migrations if needed
        let manager = Self { pool };
        manager.ensure_schema().await?;
        
        Ok(manager)
    }

    /// Initialize database schema from migrations
    async fn ensure_schema(&self) -> Result<()> {
        // Execute schema migration
        let migration_sql = include_str!("../../../migrations/0001_init.sql");
        
        sqlx::query(migration_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| ContextError::Other(format!("Failed to initialize schema: {}", e)))?;
        
        Ok(())
    }

    /// Index a single document
    pub async fn index_document(
        &self,
        id: &str,
        title: &str,
        path: &Path,
        content: &str,
        facets: &HashMap<String, Vec<String>>,
        trust_score: f64,
        confidence: f64,
    ) -> Result<()> {
        let path_str = path.to_string_lossy();
        let facets_json = serde_json::to_string(facets)
            .map_err(|e| ContextError::Other(format!("Failed to serialize facets: {}", e)))?;
        
        // Calculate content hash
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());
        
        // Estimate token count (rough approximation)
        let token_count = content.split_whitespace().count() as i64;
        
        let now = chrono::Utc::now().to_rfc3339();
        
        // Begin transaction for atomic operation
        let mut tx = self.pool.begin()
            .await
            .map_err(|e| ContextError::Other(format!("Failed to start transaction: {}", e)))?;
        
        // Insert/update document
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO docs (
                id, title, path, source, created_at, updated_at, content_hash,
                facets, trust_score, confidence, token_count, content
            ) VALUES (?, ?, ?, 'local', ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(id)
        .bind(title)
        .bind(path_str.as_ref())
        .bind(&now)
        .bind(&now)
        .bind(&content_hash)
        .bind(&facets_json)
        .bind(trust_score)
        .bind(confidence)
        .bind(token_count)
        .bind(content)
        .execute(&mut *tx)
        .await
        .map_err(|e| ContextError::Other(format!("Failed to insert document: {}", e)))?;
        
        // Update FTS index
        sqlx::query("INSERT OR REPLACE INTO docs_fts(rowid, content, title) VALUES ((SELECT rowid FROM docs WHERE id = ?), ?, ?)")
            .bind(id)
            .bind(content)
            .bind(title)
            .execute(&mut *tx)
            .await
            .map_err(|e| ContextError::Other(format!("Failed to update FTS index: {}", e)))?;
        
        // Commit transaction
        tx.commit()
            .await
            .map_err(|e| ContextError::Other(format!("Failed to commit transaction: {}", e)))?;
        
        Ok(())
    }

    /// Search documents using full-text search
    pub async fn search_documents(&self, query: &str, limit: usize) -> Result<Vec<DocumentResult>> {
        let rows = sqlx::query(
            r#"
            SELECT d.id, d.title, d.path, d.facets, d.trust_score, d.confidence, 
                   rank, snippet(docs_fts, 0, '[MATCH]', '[/MATCH]', '...', 32) as snippet
            FROM docs_fts
            JOIN docs d ON docs_fts.rowid = (SELECT rowid FROM docs WHERE id = docs_fts.rowid)
            WHERE docs_fts MATCH ?
            ORDER BY rank
            LIMIT ?
            "#
        )
        .bind(query)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ContextError::Other(format!("Search failed: {}", e)))?;
        
        let mut results = Vec::new();
        for row in rows {
            let facets_json: String = row.get("facets");
            let facets: HashMap<String, Vec<String>> = serde_json::from_str(&facets_json)
                .unwrap_or_default();
            
            results.push(DocumentResult {
                id: row.get("id"),
                title: row.get("title"),
                path: row.get("path"),
                facets,
                trust_score: row.get("trust_score"),
                confidence: row.get("confidence"),
                snippet: row.get("snippet"),
            });
        }
        
        Ok(results)
    }

    /// Get document by ID
    pub async fn get_document(&self, id: &str) -> Result<Option<DocumentRecord>> {
        let row = sqlx::query(
            "SELECT id, title, path, content, facets, trust_score, confidence, token_count, created_at, updated_at FROM docs WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ContextError::Other(format!("Failed to get document: {}", e)))?;
        
        if let Some(row) = row {
            let facets_json: String = row.get("facets");
            let facets: HashMap<String, Vec<String>> = serde_json::from_str(&facets_json)
                .unwrap_or_default();
            
            Ok(Some(DocumentRecord {
                id: row.get("id"),
                title: row.get("title"),
                path: row.get("path"),
                content: row.get("content"),
                facets,
                trust_score: row.get("trust_score"),
                confidence: row.get("confidence"),
                token_count: row.get("token_count"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    /// List all documents with optional filtering
    pub async fn list_documents(&self, facet_filter: Option<&str>, limit: Option<usize>) -> Result<Vec<DocumentResult>> {
        let mut query = "SELECT id, title, path, facets, trust_score, confidence FROM docs".to_string();
        let mut bindings = Vec::new();
        
        if let Some(facet) = facet_filter {
            query.push_str(" WHERE facets LIKE ?");
            bindings.push(format!("%{}%", facet));
        }
        
        query.push_str(" ORDER BY updated_at DESC");
        
        if let Some(limit) = limit {
            query.push_str(" LIMIT ?");
            bindings.push(limit.to_string());
        }
        
        let mut query_builder = sqlx::query(&query);
        for binding in &bindings {
            query_builder = query_builder.bind(binding);
        }
        
        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ContextError::Other(format!("Failed to list documents: {}", e)))?;
        
        let mut results = Vec::new();
        for row in rows {
            let facets_json: String = row.get("facets");
            let facets: HashMap<String, Vec<String>> = serde_json::from_str(&facets_json)
                .unwrap_or_default();
            
            results.push(DocumentResult {
                id: row.get("id"),
                title: row.get("title"),
                path: row.get("path"),
                facets,
                trust_score: row.get("trust_score"),
                confidence: row.get("confidence"),
                snippet: "".to_string(), // No snippet for list operations
            });
        }
        
        Ok(results)
    }

    /// Check if document exists and get its hash
    pub async fn get_document_hash(&self, id: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT content_hash FROM docs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ContextError::Other(format!("Failed to get document hash: {}", e)))?;
        
        Ok(row.map(|r| r.get("content_hash")))
    }

    /// Get indexing statistics
    pub async fn get_index_stats(&self) -> Result<IndexStats> {
        let total_docs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM docs")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ContextError::Other(format!("Failed to get document count: {}", e)))?;
        
        let total_tokens: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(token_count), 0) FROM docs")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        
        let last_updated: Option<String> = sqlx::query_scalar(
            "SELECT value FROM idx_meta WHERE key = 'last_index_update'"
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ContextError::Other(format!("Failed to get last update: {}", e)))?;
        
        Ok(IndexStats {
            total_documents: total_docs as usize,
            total_tokens: total_tokens as usize,
            last_updated: last_updated.unwrap_or_else(|| "Unknown".to_string()),
        })
    }

    /// Update index metadata
    pub async fn update_metadata(&self, key: &str, value: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        
        sqlx::query("INSERT OR REPLACE INTO idx_meta (key, value, updated_at) VALUES (?, ?, ?)")
            .bind(key)
            .bind(value)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| ContextError::Other(format!("Failed to update metadata: {}", e)))?;
        
        Ok(())
    }

    /// Clear all documents (full reindex)
    pub async fn clear_index(&self) -> Result<()> {
        let mut tx = self.pool.begin()
            .await
            .map_err(|e| ContextError::Other(format!("Failed to start transaction: {}", e)))?;
        
        sqlx::query("DELETE FROM docs_fts")
            .execute(&mut *tx)
            .await
            .map_err(|e| ContextError::Other(format!("Failed to clear FTS index: {}", e)))?;
        
        sqlx::query("DELETE FROM docs")
            .execute(&mut *tx)
            .await
            .map_err(|e| ContextError::Other(format!("Failed to clear docs: {}", e)))?;
        
        tx.commit()
            .await
            .map_err(|e| ContextError::Other(format!("Failed to commit clear: {}", e)))?;
        
        Ok(())
    }
}

/// Document search result
#[derive(Debug, Clone)]
pub struct DocumentResult {
    pub id: String,
    pub title: String,
    pub path: String,
    pub facets: HashMap<String, Vec<String>>,
    pub trust_score: f64,
    pub confidence: f64,
    pub snippet: String,
}

/// Full document record
#[derive(Debug, Clone)]
pub struct DocumentRecord {
    pub id: String,
    pub title: String,
    pub path: String,
    pub content: String,
    pub facets: HashMap<String, Vec<String>>,
    pub trust_score: f64,
    pub confidence: f64,
    pub token_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub total_documents: usize,
    pub total_tokens: usize,
    pub last_updated: String,
}