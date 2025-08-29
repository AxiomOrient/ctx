-- ctx Database Schema v1.0
-- PLAN.md compliant SQLite schema for document indexing and search

-- Documents table: Core document metadata and classification
CREATE TABLE docs (
    id TEXT PRIMARY KEY,                -- Deterministic document ID
    title TEXT NOT NULL,                -- Document title
    path TEXT NOT NULL,                 -- File path relative to repo root
    source TEXT NOT NULL DEFAULT 'local', -- Source type: local, git, remote
    created_at TEXT NOT NULL,           -- ISO 8601 timestamp
    updated_at TEXT NOT NULL,           -- ISO 8601 timestamp
    content_hash TEXT NOT NULL,         -- SHA256 of content for change detection
    facets TEXT NOT NULL DEFAULT '{}', -- JSON: classified facets {ns: [values]}
    trust_score REAL NOT NULL DEFAULT 0.5,    -- Trust level: 0.0-1.0
    confidence REAL NOT NULL DEFAULT 0.0,     -- Classification confidence: 0.0-1.0
    token_count INTEGER NOT NULL DEFAULT 0,   -- Approximate token count
    content TEXT NOT NULL DEFAULT '',         -- Full document content for FTS
    metadata TEXT DEFAULT NULL                -- JSON: additional metadata
);

-- Full-Text Search table: FTS5 virtual table for content search
CREATE VIRTUAL TABLE docs_fts USING fts5(
    content,            -- Searchable content
    title,              -- Searchable title
    content='docs',     -- Reference table
    content_rowid='id'  -- Use docs.rowid for linking
);

-- Index metadata: System configuration and versioning
CREATE TABLE idx_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX idx_docs_facets ON docs(facets);
CREATE INDEX idx_docs_trust_conf ON docs(trust_score, confidence);
CREATE INDEX idx_docs_updated ON docs(updated_at);
CREATE INDEX idx_docs_path ON docs(path);
CREATE INDEX idx_docs_content_hash ON docs(content_hash);

-- Insert schema version and metadata
INSERT INTO idx_meta (key, value) VALUES
    ('schema_version', '1'),
    ('ontology_version', ''),
    ('rules_version', ''),
    ('last_index_update', CURRENT_TIMESTAMP);
