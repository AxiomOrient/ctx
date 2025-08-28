use crate::common::{ContextDocument, IndexEntry};
use crate::common::errors::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 인덱스 관리자 트레이트
#[allow(dead_code)]
pub trait IndexManager {
    /// 인덱스 로드
    fn load_index(&mut self, path: &Path) -> Result<()>;

    /// 인덱스 저장
    fn save_index(&self, path: &Path) -> Result<()>;

    /// 인덱스 엔트리 추가/업데이트
    fn upsert_entry(&mut self, entry: IndexEntry) -> Result<()>;

    /// 인덱스 엔트리 제거
    fn remove_entry(&mut self, id: &str) -> Result<bool>;

    /// 인덱스 통계
    fn get_stats(&self) -> IndexStats;

    /// 인덱스 무결성 검사
    fn validate_integrity(&self) -> Result<IntegrityReport>;

    /// 인덱스 최적화 (중복 제거, 압축 등)
    fn optimize(&mut self) -> Result<OptimizationReport>;
}

/// 인덱스 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_documents: usize,
    pub total_sections: usize,
    pub types_distribution: HashMap<String, usize>,
    pub domains_distribution: HashMap<String, usize>,
    pub tags_distribution: HashMap<String, usize>,
    pub schema_versions: HashMap<String, usize>,
    pub last_updated: DateTime<Utc>,
    pub index_size_bytes: u64,
}

/// 무결성 검사 보고서
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    pub is_valid: bool,
    pub issues: Vec<IntegrityIssue>,
    pub orphaned_files: Vec<PathBuf>,
    pub missing_files: Vec<String>, // document IDs
    pub duplicate_ids: Vec<String>,
}

/// 무결성 문제
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityIssue {
    pub document_id: String,
    pub issue_type: IntegrityIssueType,
    pub description: String,
    pub severity: IssueSeverity,
}

/// 무결성 문제 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrityIssueType {
    MissingFile,
    InvalidPath,
    SchemaViolation,
    BrokenDependency,
    DuplicateId,
}

/// 문제 심각도
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// 최적화 보고서
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationReport {
    pub entries_removed: usize,
    pub space_saved_bytes: u64,
    pub duration_ms: u64,
    pub actions_taken: Vec<String>,
}

/// 지속화 가능한 인덱스 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedIndex {
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub entries: HashMap<String, PersistedEntry>,
    pub metadata: IndexMetadata,
}

/// 지속화된 인덱스 엔트리
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedEntry {
    pub doc: ContextDocument,
    pub path: PathBuf,
    pub summary: String,
    pub last_checked: DateTime<Utc>,
    pub file_hash: String, // 파일 변경 감지용
    pub file_size: u64,
}

/// 인덱스 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub total_scans: u64,
    pub last_scan_duration_ms: u64,
    pub schema_migrations: Vec<SchemaMigration>,
}

/// 스키마 마이그레이션 기록
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaMigration {
    pub from_version: String,
    pub to_version: String,
    pub migrated_at: DateTime<Utc>,
    pub documents_affected: usize,
}

pub mod pool;
pub mod pooled_sqlite;
pub mod sqlite;
