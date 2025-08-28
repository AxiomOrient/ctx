-- 인덱스 버전/설정
CREATE TABLE IF NOT EXISTS idx_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- 커밋 추적(스냅샷 단위)
CREATE TABLE IF NOT EXISTS commits (
  repo TEXT NOT NULL,
  branch TEXT NOT NULL,
  sha TEXT PRIMARY KEY,
  parent_sha TEXT,
  indexed_at TEXT NOT NULL,
  UNIQUE(repo, branch, sha)
);

-- 문서 마스터 (SCD2)
CREATE TABLE IF NOT EXISTS docs (
  doc_id TEXT NOT NULL,            -- UUID v5 (repo:first_commit:file_signature)
  repo TEXT NOT NULL,
  branch TEXT NOT NULL,
  path TEXT NOT NULL,              -- 파일 경로(그 시점)
  sha TEXT NOT NULL,               -- 인덱싱 기준 커밋
  content_hash TEXT NOT NULL,      -- SHA-256 of normalized content
  title TEXT,
  locale TEXT,
  trust REAL,
  freshness TEXT,                  -- ISO date
  source_type TEXT,
  maturity TEXT,
  valid_from TEXT NOT NULL,        -- commits.indexed_at
  valid_to TEXT,                   -- NULL이면 최신 레코드
  deleted INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (doc_id, sha)
);

-- 패싯 값 정규화 테이블
CREATE TABLE IF NOT EXISTS facet_values (
  namespace TEXT NOT NULL,
  value TEXT NOT NULL,             -- canonical
  PRIMARY KEY (namespace, value)
);

-- 문서-패싯 매핑
CREATE TABLE IF NOT EXISTS doc_facets (
  doc_id TEXT NOT NULL,
  sha TEXT NOT NULL,
  namespace TEXT NOT NULL,
  value TEXT NOT NULL,
  PRIMARY KEY (doc_id, sha, namespace, value),
  FOREIGN KEY (doc_id, sha) REFERENCES docs (doc_id, sha)
);

-- 참조 관계
CREATE TABLE IF NOT EXISTS doc_requires (
  doc_id TEXT NOT NULL, 
  sha TEXT NOT NULL, 
  req_id TEXT NOT NULL,
  PRIMARY KEY (doc_id, sha, req_id)
);

CREATE TABLE IF NOT EXISTS doc_conflicts (
  doc_id TEXT NOT NULL, 
  sha TEXT NOT NULL, 
  bad_id TEXT NOT NULL,
  PRIMARY KEY (doc_id, sha, bad_id)
);

-- 분류 로그/품질
CREATE TABLE IF NOT EXISTS classify_log (
  doc_id TEXT NOT NULL, 
  sha TEXT NOT NULL,
  confidence REAL NOT NULL,
  warnings TEXT,                   -- JSON array
  errors TEXT,                     -- JSON array
  ontology_version TEXT NOT NULL,
  rules_version TEXT NOT NULL,
  PRIMARY KEY (doc_id, sha)
);

-- 인덱싱 상태
CREATE TABLE IF NOT EXISTS index_state (
  repo TEXT NOT NULL, 
  branch TEXT NOT NULL,
  last_indexed_sha TEXT,
  PRIMARY KEY (repo, branch)
);

-- 실행 결과 캐시 (결정성 보장)
CREATE TABLE IF NOT EXISTS execution_cache (
    execution_id TEXT PRIMARY KEY,
    canonical_input_hash TEXT NOT NULL,
    context_hash TEXT NOT NULL,
    result_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT,
    access_count INTEGER DEFAULT 1,
    last_accessed TEXT NOT NULL
);

-- 유사성 캐시 (similarity-stable 정책)
CREATE TABLE IF NOT EXISTS similarity_cache (
    input_hash TEXT NOT NULL,
    similar_hash TEXT NOT NULL,
    similarity_score REAL NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (input_hash, similar_hash)
);

-- 성능을 위한 인덱스들
CREATE INDEX IF NOT EXISTS idx_docs_repo_branch_sha ON docs(repo, branch, sha);
CREATE INDEX IF NOT EXISTS idx_docs_valid_to ON docs(valid_to);
CREATE INDEX IF NOT EXISTS idx_doc_facets_sha ON doc_facets(sha);
CREATE INDEX IF NOT EXISTS idx_doc_facets_namespace_value ON doc_facets(namespace, value);

-- 결정성 엔진용 인덱스들
CREATE INDEX IF NOT EXISTS idx_execution_canonical_hash ON execution_cache(canonical_input_hash);
CREATE INDEX IF NOT EXISTS idx_execution_context_hash ON execution_cache(context_hash);
CREATE INDEX IF NOT EXISTS idx_execution_created_at ON execution_cache(created_at);
CREATE INDEX IF NOT EXISTS idx_similarity_score ON similarity_cache(similarity_score DESC);
CREATE INDEX IF NOT EXISTS idx_similarity_input_hash ON similarity_cache(input_hash);

-- 성능 최적화용 커버링 인덱스
CREATE INDEX IF NOT EXISTS idx_docs_covering ON docs(doc_id, repo, branch, sha, title, path, valid_from) 
    WHERE valid_to IS NULL AND deleted = 0;
CREATE INDEX IF NOT EXISTS idx_docs_valid_from ON docs(valid_from DESC) WHERE valid_to IS NULL;
CREATE INDEX IF NOT EXISTS idx_classify_log_confidence ON classify_log(confidence DESC);

-- 조합(Compose) 로그: 대시보드 통계 및 최근 작업용
CREATE TABLE IF NOT EXISTS compose_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_at TEXT NOT NULL,
  repo TEXT NOT NULL,
  branch TEXT NOT NULL,
  commit_sha TEXT NOT NULL,
  facets_json TEXT NOT NULL,
  tokens_used INTEGER,
  documents_selected INTEGER,
  confidence REAL
);
CREATE INDEX IF NOT EXISTS idx_compose_log_created_at ON compose_log(created_at DESC);

PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
