//! 데이터베이스 커넥션 풀 관리
//!
//! SQLite 커넥션 풀을 통해 다중 클라이언트의 동시 접근을 효율적으로 처리합니다.

use crate::common::constants::defaults::*;
use crate::common::errors::ContextError;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// SQLite 커넥션 풀 래퍼
#[derive(Clone)]
pub struct SqlitePool {
    pool: Pool<SqliteConnectionManager>,
    db_path: String,
}

/// 커넥션 풀 설정
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// 최대 커넥션 수
    pub max_connections: u32,
    /// 최소 유휴 커넥션 수  
    pub min_idle: Option<u32>,
    /// 커넥션 타임아웃 (초)
    pub connection_timeout: Duration,
    /// 유휴 타임아웃 (초)
    pub idle_timeout: Option<Duration>,
    /// 커넥션 최대 생명주기
    pub max_lifetime: Option<Duration>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: DEFAULT_MAX_DB_CONNECTIONS,
            min_idle: Some(DEFAULT_MIN_IDLE_CONNECTIONS),
            connection_timeout: Duration::from_secs(DEFAULT_CONNECTION_TIMEOUT_SECS),
            idle_timeout: Some(Duration::from_secs(DEFAULT_IDLE_TIMEOUT_SECS)),
            max_lifetime: Some(Duration::from_secs(DEFAULT_MAX_LIFETIME_SECS)),
        }
    }
}

impl SqlitePool {
    /// 새로운 커넥션 풀 생성
    pub fn new(db_path: &str, config: Option<PoolConfig>) -> Result<Self, ContextError> {
        let config = config.unwrap_or_default();

        // SQLite 플래그 설정
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX; // 멀티스레드에서 안전

        let manager = SqliteConnectionManager::file(db_path)
            .with_flags(flags)
            .with_init(|c| {
                // 커넥션 초기화 설정
                c.pragma_update(None, "journal_mode", "WAL")?;
                c.pragma_update(None, "synchronous", "NORMAL")?;
                c.pragma_update(None, "temp_store", "memory")?;
                c.pragma_update(None, "mmap_size", "67108864")?; // 64MB
                c.pragma_update(None, "cache_size", "10000")?; // 10,000 pages
                c.execute_batch("PRAGMA optimize")?;
                Ok(())
            });

        let pool = Pool::builder()
            .max_size(config.max_connections)
            .min_idle(config.min_idle)
            .connection_timeout(config.connection_timeout)
            .idle_timeout(config.idle_timeout)
            .max_lifetime(config.max_lifetime)
            .build(manager)
            .map_err(|e| ContextError::Other(format!("Failed to create connection pool: {}", e)))?;

        // 스키마 초기화 (첫 번째 커넥션으로)
        let conn = pool
            .get()
            .map_err(|e| ContextError::Other(format!("Failed to get initial connection: {}", e)))?;

        conn.execute_batch(include_str!("sqlite_schema.sql"))
            .map_err(ContextError::DatabaseError)?;

        Ok(Self {
            pool,
            db_path: db_path.to_string(),
        })
    }

    /// 기본 설정으로 커넥션 풀 생성
    pub fn new_default(db_path: &str) -> Result<Self, ContextError> {
        Self::new(db_path, None)
    }

    /// 커넥션 풀에서 커넥션 가져오기
    pub fn get(&self) -> Result<PooledConnection, ContextError> {
        let conn = self.pool.get().map_err(|e| {
            ContextError::Other(format!("Failed to get connection from pool: {}", e))
        })?;

        Ok(PooledConnection { inner: conn })
    }

    /// 풀 상태 정보 가져오기
    pub fn state(&self) -> PoolState {
        let state = self.pool.state();
        PoolState {
            connections: state.connections,
            idle_connections: state.idle_connections,
            max_connections: self.pool.max_size(),
        }
    }

    /// 데이터베이스 경로 반환
    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    /// 커넥션 풀 테스트 (연결 확인)
    pub fn test_connection(&self) -> Result<(), ContextError> {
        let conn = self.get()?;
        let _result: i32 = conn.inner.query_row("SELECT 1", [], |row| row.get(0))?;
        Ok(())
    }

    /// 데이터베이스 최적화 실행
    pub fn optimize(&self) -> Result<(), ContextError> {
        let conn = self.get()?;
        conn.execute_batch("PRAGMA optimize; ANALYZE;")?;
        Ok(())
    }

    /// 데이터베이스 정리 (VACUUM)
    pub fn vacuum(&self) -> Result<(), ContextError> {
        let conn = self.get()?;
        conn.execute("VACUUM", [])?;
        Ok(())
    }
}

/// 풀링된 커넥션 래퍼
pub struct PooledConnection {
    inner: r2d2::PooledConnection<SqliteConnectionManager>,
}

impl PooledConnection {
    /// 내부 커넥션에 직접 접근
    pub fn connection(&self) -> &Connection {
        &self.inner
    }

    /// 쿼리 실행
    pub fn execute(&self, sql: &str, params: impl rusqlite::Params) -> Result<usize, ContextError> {
        self.inner
            .execute(sql, params)
            .map_err(ContextError::DatabaseError)
    }

    /// 배치 실행
    pub fn execute_batch(&self, sql: &str) -> Result<(), ContextError> {
        self.inner
            .execute_batch(sql)
            .map_err(ContextError::DatabaseError)
    }

    /// 준비된 구문 생성
    pub fn prepare(&self, sql: &str) -> Result<rusqlite::Statement, ContextError> {
        self.inner.prepare(sql).map_err(ContextError::DatabaseError)
    }

    /// 트랜잭션 시작
    pub fn transaction(&mut self) -> Result<rusqlite::Transaction, ContextError> {
        self.inner
            .transaction()
            .map_err(ContextError::DatabaseError)
    }

    /// 체크하지 않은 트랜잭션 시작 (성능 최적화용)
    pub fn unchecked_transaction(&mut self) -> Result<rusqlite::Transaction, ContextError> {
        self.inner
            .unchecked_transaction()
            .map_err(ContextError::DatabaseError)
    }
}

/// 커넥션 풀 상태 정보
#[derive(Debug, Clone)]
pub struct PoolState {
    /// 현재 활성 커넥션 수
    pub connections: u32,
    /// 현재 유휴 커넥션 수
    pub idle_connections: u32,
    /// 최대 커넥션 수
    pub max_connections: u32,
}

impl PoolState {
    /// 사용률 계산 (0.0 ~ 1.0)
    pub fn utilization(&self) -> f32 {
        if self.max_connections == 0 {
            0.0
        } else {
            self.connections as f32 / self.max_connections as f32
        }
    }

    /// 사용 가능한 커넥션 수
    pub fn available_connections(&self) -> u32 {
        self.max_connections.saturating_sub(self.connections)
    }
}

/// 커넥션 풀 매니저 - 싱글톤 패턴
pub struct PoolManager {
    pools: std::sync::RwLock<std::collections::HashMap<String, Arc<SqlitePool>>>,
}

impl PoolManager {
    /// 새로운 풀 매니저 생성
    pub fn new() -> Self {
        Self {
            pools: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// 풀 가져오기 또는 생성
    pub fn get_or_create_pool(
        &self,
        db_path: &str,
        config: Option<PoolConfig>,
    ) -> Result<Arc<SqlitePool>, ContextError> {
        let canonical_path = Path::new(db_path)
            .canonicalize()
            .map_err(|e| ContextError::Other(format!("Invalid database path {}: {}", db_path, e)))?
            .to_string_lossy()
            .to_string();

        // 읽기 락으로 먼저 확인
        {
            let pools = self
                .pools
                .read()
                .map_err(|_| ContextError::Other("Pool manager lock poisoned".to_string()))?;

            if let Some(pool) = pools.get(&canonical_path) {
                return Ok(Arc::clone(pool));
            }
        }

        // 쓰기 락으로 새 풀 생성
        let mut pools = self
            .pools
            .write()
            .map_err(|_| ContextError::Other("Pool manager lock poisoned".to_string()))?;

        // 다시 확인 (다른 스레드가 이미 생성했을 수 있음)
        if let Some(pool) = pools.get(&canonical_path) {
            return Ok(Arc::clone(pool));
        }

        let pool = Arc::new(SqlitePool::new(&canonical_path, config)?);
        pools.insert(canonical_path, Arc::clone(&pool));

        Ok(pool)
    }

    /// 풀 제거
    pub fn remove_pool(&self, db_path: &str) -> Result<bool, ContextError> {
        let canonical_path = Path::new(db_path)
            .canonicalize()
            .map_err(|e| ContextError::Other(format!("Invalid database path {}: {}", db_path, e)))?
            .to_string_lossy()
            .to_string();

        let mut pools = self
            .pools
            .write()
            .map_err(|_| ContextError::Other("Pool manager lock poisoned".to_string()))?;

        Ok(pools.remove(&canonical_path).is_some())
    }

    /// 모든 풀의 상태 정보
    pub fn get_all_states(
        &self,
    ) -> Result<std::collections::HashMap<String, PoolState>, ContextError> {
        let pools = self
            .pools
            .read()
            .map_err(|_| ContextError::Other("Pool manager lock poisoned".to_string()))?;

        let mut states = std::collections::HashMap::new();
        for (path, pool) in pools.iter() {
            states.insert(path.clone(), pool.state());
        }

        Ok(states)
    }

    /// 모든 풀 최적화
    pub fn optimize_all(&self) -> Result<(), ContextError> {
        let pools = self
            .pools
            .read()
            .map_err(|_| ContextError::Other("Pool manager lock poisoned".to_string()))?;

        for pool in pools.values() {
            pool.optimize()?;
        }

        Ok(())
    }
}

impl Default for PoolManager {
    fn default() -> Self {
        Self::new()
    }
}

// 전역 풀 매니저 인스턴스
lazy_static::lazy_static! {
    static ref GLOBAL_POOL_MANAGER: PoolManager = PoolManager::new();
}

/// 전역 풀 매니저에 접근
pub fn global_pool_manager() -> &'static PoolManager {
    &GLOBAL_POOL_MANAGER
}

/// 편의 함수: 기본 풀 가져오기
pub fn get_default_pool(db_path: &str) -> Result<Arc<SqlitePool>, ContextError> {
    global_pool_manager().get_or_create_pool(db_path, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_pool_creation() -> Result<(), ContextError> {
        let temp_file = NamedTempFile::new().map_err(ContextError::Io)?;
        let db_path = temp_file
            .path()
            .to_str()
            .ok_or_else(|| ContextError::Other("Invalid temp file path".to_string()))?;

        let pool = SqlitePool::new_default(db_path)?;

        assert_eq!(pool.state().max_connections, DEFAULT_MAX_DB_CONNECTIONS);

        // 연결 테스트를 더 견고하게 수행
        match pool.test_connection() {
            Ok(()) => {
                // 성공적으로 연결됨
            }
            Err(e) => {
                eprintln!("Connection test failed: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_pool_connection() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let pool = SqlitePool::new_default(temp_file.path().to_str().unwrap())
            .expect("Failed to create pool");

        let conn = pool.get().expect("Failed to get connection");
        let result: i32 = conn
            .connection()
            .query_row("SELECT 1", [], |row| row.get(0))
            .expect("Failed to execute query");

        assert_eq!(result, 1);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_pool_manager() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let db_path = temp_file.path().to_str().unwrap();

        let manager = PoolManager::new();
        let pool1 = manager
            .get_or_create_pool(db_path, None)
            .expect("Failed to create pool");
        let pool2 = manager
            .get_or_create_pool(db_path, None)
            .expect("Failed to get pool");

        assert!(Arc::ptr_eq(&pool1, &pool2));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_pool_state() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let pool = SqlitePool::new_default(temp_file.path().to_str().unwrap())
            .expect("Failed to create pool");

        let state = pool.state();
        assert_eq!(state.max_connections, DEFAULT_MAX_DB_CONNECTIONS);
        assert!(state.utilization() >= 0.0 && state.utilization() <= 1.0);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_concurrent_connections() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let pool = Arc::new(
            SqlitePool::new_default(temp_file.path().to_str().unwrap())
                .expect("Failed to create pool"),
        );

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let pool_clone = Arc::clone(&pool);
                std::thread::spawn(move || {
                    let conn = pool_clone.get().expect("Failed to get connection");
                    let result: i32 = conn
                        .connection()
                        .query_row("SELECT ?", [i], |row| row.get(0))
                        .expect("Failed to execute query");
                    assert_eq!(result, i);
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }
}
