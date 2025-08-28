# Design Document

## Overview

CTXSET의 구현 설계는 기존의 잘 구조화된 6-레이어 아키텍처를 기반으로 하여 누락된 핵심 구현을 완성하는 것입니다. 이 설계는 결정성(determinism)과 유사-결정성(similarity-stable) 정책을 중심으로 하며, 확장 가능하고 유지보수 가능한 시스템을 목표로 합니다.

## Architecture

### Layer Architecture (Existing)
```
app/           # HTTP/CLI/MCP 진입점
├── server/    # HTTP API 서버
├── cli/       # CLI 명령어
└── mcp/       # MCP JSON-RPC 서버

core/          # 비즈니스 로직
├── classifier/ # 패싯 분류 엔진
├── composer/  # 프롬프트 조합 엔진
├── dependency/ # 의존성 분석
└── transformer/ # 데이터 변환

data/          # 데이터 접근 레이어
├── index/     # SQLite 인덱스
├── storage/   # 파일 스토리지
├── git/       # Git 연동
└── id/        # ID 생성

doc/           # 문서 처리
├── parse/     # 파싱 (frontmatter, sections)
├── schema/    # 스키마 검증
└── validate/  # 검증 파이프라인

knowledge/     # 온톨로지/룰 관리
├── ontology/  # 온톨로지 스키마
└── rules/     # 분류 룰

common/        # 공통 타입/상수/에러
├── constants/ # 중앙화된 상수
├── errors.rs  # 에러 타입
└── utils.rs   # 유틸리티
```

### Key Design Principles

1. **Deterministic Execution**: 동일 입력 → 동일 출력
2. **Similarity-stable**: 유사 입력 → 유사 출력  
3. **Centralized Constants**: 모든 임계값/가중치는 constants/에서 관리
4. **Layered Dependencies**: 단방향 의존성 (app → core → data/doc/knowledge → common)
5. **Feature Flags**: 선택적 컴파일 (server, cli, mcp)

## Components and Interfaces

### 1. Core Composer (핵심 구현 필요)

```rust
// src/core/composer/mod.rs
pub struct BuildComposer {
    scorer: DocumentScorer,
    selector: DocumentSelector,
    merger: DocumentMerger,
}

impl BuildComposer {
    pub fn compose(
        &self,
        candidates: Vec<DocumentCandidate>,
        query: &BuildQuery,
        budget: usize,
        reserve: usize,
    ) -> Result<CompositionResult> {
        // 1. Score documents
        let scored = self.scorer.score_documents(&candidates, query)?;
        
        // 2. Select optimal subset (MMR algorithm)
        let selected = self.selector.select_documents(scored, budget, reserve)?;
        
        // 3. Merge into single document
        let merged = self.merger.merge_documents(selected, query)?;
        
        Ok(CompositionResult {
            merged_document: merged,
            selection_rationale: self.generate_rationale(),
            total_tokens_used: merged.tokens,
            confidence_score: self.calculate_confidence(),
            rejected_documents: vec![], // TODO: track rejected
        })
    }
}
```

### 2. Document Scorer (완성 필요)

```rust
// src/core/composer/scorer/mod.rs
pub struct DocumentScorer {
    weights: ScoringWeights,
}

impl DocumentScorer {
    pub fn score_documents(
        &self,
        candidates: &[DocumentCandidate],
        query: &BuildQuery,
    ) -> Result<Vec<ScoredCandidate>> {
        // Multi-criteria scoring with weighted combination
    }
    
    pub fn calculate_similarity(
        &self, 
        a: &DocumentCandidate, 
        b: &DocumentCandidate
    ) -> f32 {
        // Jaccard + Trigram similarity
    }
}
```

### 3. Document Selector (새로 구현)

```rust
// src/core/composer/selector.rs
pub struct DocumentSelector {
    mmr_lambda: f32,
    scorer: Arc<DocumentScorer>, // For similarity calculations
}

impl DocumentSelector {
    pub fn select_documents(
        &self,
        scored: Vec<ScoredCandidate>,
        budget: usize,
        reserve: usize,
    ) -> Result<Vec<ScoredCandidate>> {
        let effective_budget = budget.saturating_sub(reserve);
        
        // Phase 1: Greedy selection by score (fast path for small sets)
        if scored.len() <= 10 {
            return self.greedy_selection(scored, effective_budget);
        }
        
        // Phase 2: MMR selection for larger sets
        self.mmr_selection(scored, effective_budget)
    }
    
    fn mmr_selection(
        &self,
        mut candidates: Vec<ScoredCandidate>,
        budget: usize,
    ) -> Result<Vec<ScoredCandidate>> {
        let mut selected = Vec::new();
        let mut remaining_budget = budget;
        
        // Sort by relevance score initially
        candidates.sort_by(|a, b| b.base_score.partial_cmp(&a.base_score).unwrap_or(Ordering::Equal));
        
        while !candidates.is_empty() && remaining_budget > 0 {
            let mut best_idx = 0;
            let mut best_mmr_score = f32::NEG_INFINITY;
            
            for (idx, candidate) in candidates.iter().enumerate() {
                if candidate.candidate.tokens > remaining_budget {
                    continue; // Skip if doesn't fit
                }
                
                // Calculate diversity score (average similarity to selected documents)
                let diversity_score = if selected.is_empty() {
                    1.0 // First document has maximum diversity
                } else {
                    let avg_similarity = selected.iter()
                        .map(|sel| self.scorer.calculate_similarity(&candidate.candidate, &sel.candidate))
                        .sum::<f32>() / selected.len() as f32;
                    1.0 - avg_similarity // Higher diversity = lower similarity
                };
                
                // MMR score: (1-λ) * relevance + λ * diversity
                let mmr_score = (1.0 - self.mmr_lambda) * candidate.base_score 
                              + self.mmr_lambda * diversity_score;
                
                if mmr_score > best_mmr_score {
                    best_mmr_score = mmr_score;
                    best_idx = idx;
                }
            }
            
            if best_mmr_score == f32::NEG_INFINITY {
                break; // No more documents fit in budget
            }
            
            let selected_doc = candidates.remove(best_idx);
            remaining_budget = remaining_budget.saturating_sub(selected_doc.candidate.tokens);
            selected.push(selected_doc);
        }
        
        Ok(selected)
    }
    
    fn greedy_selection(
        &self,
        mut candidates: Vec<ScoredCandidate>,
        budget: usize,
    ) -> Result<Vec<ScoredCandidate>> {
        // Simple greedy selection for small sets
        candidates.sort_by(|a, b| b.base_score.partial_cmp(&a.base_score).unwrap_or(Ordering::Equal));
        
        let mut selected = Vec::new();
        let mut remaining_budget = budget;
        
        for candidate in candidates {
            if candidate.candidate.tokens <= remaining_budget {
                remaining_budget -= candidate.candidate.tokens;
                selected.push(candidate);
            }
        }
        
        Ok(selected)
    }
}
```

### 4. Document Merger (새로 구현)

```rust
// src/core/composer/merger.rs
pub struct DocumentMerger {
    tokenizer: Box<dyn Tokenizer>,
}

impl DocumentMerger {
    pub fn merge_documents(
        &self,
        mut documents: Vec<ScoredCandidate>,
        query: &BuildQuery,
    ) -> Result<MergedDocument> {
        if documents.is_empty() {
            return Ok(MergedDocument::empty());
        }
        
        // 1. Sort by score (highest first)
        documents.sort_by(|a, b| b.base_score.partial_cmp(&a.base_score).unwrap_or(Ordering::Equal));
        
        let mut merged_content = String::new();
        let mut source_documents = Vec::new();
        let mut sections = Vec::new();
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
            sections.push(MergedSection {
                title: doc.title.clone(),
                content: content.clone(),
                source_doc_id: doc.doc_id.clone(),
                confidence: doc.confidence,
            });
            
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
        let final_tokens = self.tokenizer.estimate_tokens(&merged_content) as usize;
        
        Ok(MergedDocument {
            content: merged_content,
            source_documents,
            tokens: final_tokens,
            sections,
        })
    }
}

impl MergedDocument {
    fn empty() -> Self {
        Self {
            content: "# No Documents Found\n\nNo documents matched the specified criteria.".to_string(),
            source_documents: vec![],
            tokens: 10,
            sections: vec![],
        }
    }
}
```

### 5. API Validation Layer (강화 필요)

```rust
// src/app/server/validation.rs
pub struct ApiValidator;

impl ApiValidator {
    pub fn validate_classify_request(req: &ClassifyRequest) -> Result<()> {
        // Validate text length, metadata format
    }
    
    pub fn validate_compose_request(req: &ComposeRequest) -> Result<()> {
        // Validate repo/branch/sha format
        // Check facets against ontology
        // Validate token budget limits
    }
    
    pub fn apply_intent_gate(
        facets: &HashMap<String, Vec<String>>,
        task_contract: &TaskContract,
    ) -> Result<()> {
        // Check category ↔ action compatibility
        // Return allowed actions on mismatch
    }
}
```

### 6. Determinism Engine (새로 구현)

```rust
// src/core/determinism.rs
pub struct DeterminismEngine {
    similarity_threshold: f32,
    cache: Arc<RwLock<LruCache<String, CachedResult>>>, // Thread-safe cache
    db_path: PathBuf, // For persistent similarity cache
}

#[derive(Debug, Clone)]
struct CachedResult {
    execution_id: String,
    result: CompositionResult,
    created_at: DateTime<Utc>,
}

impl DeterminismEngine {
    pub fn new(cache_size: usize, db_path: PathBuf) -> Self {
        Self {
            similarity_threshold: SIMILARITY_THRESHOLD,
            cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
            db_path,
        }
    }
    
    pub fn get_or_compute_execution_id(
        &self,
        canonical_input: &CanonicalInput,
        context: &ExecutionContext,
    ) -> String {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(&canonical_input.normalized_text);
        hasher.update(&serde_json::to_string(&canonical_input.facets).unwrap_or_default());
        hasher.update(&context.rules_version);
        hasher.update(&context.recipe_version);
        hasher.update(&context.commit_sha);
        hasher.update(&context.constants_version);
        
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
    
    pub async fn check_similarity_stable(
        &self,
        current_input: &CanonicalInput,
        query: &BuildQuery,
    ) -> Result<Option<CompositionResult>> {
        // 1. Check in-memory cache first
        let current_hash = self.hash_canonical_input(current_input);
        
        if let Some(cached) = self.cache.read().unwrap().get(&current_hash) {
            return Ok(Some(cached.result.clone()));
        }
        
        // 2. Check database for similar inputs
        let similar_results = self.find_similar_in_db(&current_hash, current_input).await?;
        
        for (similarity_score, cached_result) in similar_results {
            if similarity_score >= self.similarity_threshold {
                // Cache the result for future use
                self.cache.write().unwrap().put(current_hash.clone(), CachedResult {
                    execution_id: cached_result.execution_id.clone(),
                    result: cached_result.clone(),
                    created_at: Utc::now(),
                });
                
                return Ok(Some(cached_result));
            }
        }
        
        Ok(None)
    }
    
    pub fn canonicalize_input(&self, raw_input: &str, facets: &HashMap<String, Vec<String>>) -> CanonicalInput {
        let normalized_text = self.normalize_text(raw_input);
        let sorted_facets: BTreeMap<String, Vec<String>> = facets.iter()
            .map(|(k, v)| {
                let mut sorted_values = v.clone();
                sorted_values.sort();
                (k.clone(), sorted_values)
            })
            .collect();
        
        CanonicalInput {
            normalized_text,
            facets: sorted_facets,
            metadata: HashMap::new(), // TODO: Add relevant metadata
        }
    }
    
    fn normalize_text(&self, text: &str) -> String {
        use regex::Regex;
        
        let mut normalized = text.to_string();
        
        // 1. Normalize whitespace
        normalized = Regex::new(r"\s+").unwrap().replace_all(&normalized, " ").to_string();
        
        // 2. Normalize markdown headers
        normalized = Regex::new(r"#{1,6}\s+").unwrap().replace_all(&normalized, "# ").to_string();
        
        // 3. Normalize code fences
        normalized = Regex::new(r"```\w*\n").unwrap().replace_all(&normalized, "```\n").to_string();
        
        // 4. Normalize file paths (convert to forward slashes)
        normalized = normalized.replace('\\', "/");
        
        // 5. Normalize URLs (remove trailing slashes)
        normalized = Regex::new(r"https?://[^\s]+/+").unwrap()
            .replace_all(&normalized, |caps: &regex::Captures| {
                caps[0].trim_end_matches('/').to_string()
            }).to_string();
        
        // 6. Normalize dates to ISO format (basic attempt)
        // This is a simplified version - production would need more robust date parsing
        
        normalized.trim().to_string()
    }
    
    fn hash_canonical_input(&self, input: &CanonicalInput) -> String {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(&input.normalized_text);
        hasher.update(&serde_json::to_string(&input.facets).unwrap_or_default());
        
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
    
    async fn find_similar_in_db(
        &self,
        current_hash: &str,
        current_input: &CanonicalInput,
    ) -> Result<Vec<(f32, CompositionResult)>> {
        // This would query the similarity_cache table
        // For now, return empty - full implementation would:
        // 1. Query recent similar inputs from DB
        // 2. Calculate similarity scores
        // 3. Return sorted by similarity
        Ok(vec![])
    }
    
    pub async fn store_result(
        &self,
        input: &CanonicalInput,
        result: &CompositionResult,
    ) -> Result<()> {
        let input_hash = self.hash_canonical_input(input);
        
        // Store in memory cache
        self.cache.write().unwrap().put(input_hash.clone(), CachedResult {
            execution_id: result.execution_id.clone(),
            result: result.clone(),
            created_at: Utc::now(),
        });
        
        // Store in database for persistence
        // Implementation would insert into execution_cache table
        
        Ok(())
    }
}
```

## Data Models

### Core Data Structures

```rust
// Enhanced from existing common/mod.rs
#[derive(Debug, Clone)]
pub struct CompositionResult {
    pub merged_document: MergedDocument,
    pub selection_rationale: String,
    pub total_tokens_used: usize,
    pub confidence_score: f32,
    pub rejected_documents: Vec<DocumentCandidate>,
    pub execution_id: String,
    pub similarity_used: Option<f32>, // If reused from similar query
}

#[derive(Debug, Clone)]
pub struct MergedDocument {
    pub content: String,
    pub source_documents: Vec<String>,
    pub tokens: usize,
    pub sections: Vec<MergedSection>,
}

#[derive(Debug, Clone)]
pub struct MergedSection {
    pub title: String,
    pub content: String,
    pub source_doc_id: String,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub rules_version: String,
    pub recipe_version: String,
    pub commit_sha: String,
    pub constants_version: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CanonicalInput {
    pub normalized_text: String,
    pub facets: BTreeMap<String, Vec<String>>,
    pub metadata: HashMap<String, String>,
}
```

### Database Schema Enhancements

```sql
-- Additional tables for determinism tracking
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

CREATE INDEX IF NOT EXISTS idx_execution_canonical_hash ON execution_cache(canonical_input_hash);
CREATE INDEX IF NOT EXISTS idx_execution_context_hash ON execution_cache(context_hash);
CREATE INDEX IF NOT EXISTS idx_execution_created_at ON execution_cache(created_at);

CREATE TABLE IF NOT EXISTS similarity_cache (
    input_hash TEXT NOT NULL,
    similar_hash TEXT NOT NULL,
    similarity_score REAL NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (input_hash, similar_hash)
);

CREATE INDEX IF NOT EXISTS idx_similarity_score ON similarity_cache(similarity_score DESC);
CREATE INDEX IF NOT EXISTS idx_similarity_input_hash ON similarity_cache(input_hash);

-- Enhanced indexes for existing tables
CREATE INDEX IF NOT EXISTS idx_docs_repo_branch_sha ON docs(repo, branch, sha) WHERE valid_to IS NULL;
CREATE INDEX IF NOT EXISTS idx_docs_valid_from ON docs(valid_from DESC) WHERE valid_to IS NULL;
CREATE INDEX IF NOT EXISTS idx_doc_facets_namespace_value ON doc_facets(namespace, value);
CREATE INDEX IF NOT EXISTS idx_classify_log_confidence ON classify_log(confidence DESC);

-- Performance optimization: covering index for common queries
CREATE INDEX IF NOT EXISTS idx_docs_covering ON docs(doc_id, repo, branch, sha, title, path, valid_from) 
    WHERE valid_to IS NULL AND deleted = 0;
```

## Error Handling

### Structured Error Types

```rust
// Enhanced from existing common/errors.rs
#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    // ... existing variants ...
    
    #[error("Intent gate violation: {category} does not allow {action}. Allowed: {allowed:?}")]
    IntentGateViolation {
        category: String,
        action: String,
        allowed: Vec<String>,
    },
    
    #[error("Token budget exceeded: requested {requested}, available {available}")]
    TokenBudgetExceeded {
        requested: usize,
        available: usize,
    },
    
    #[error("Similarity computation failed: {reason}")]
    SimilarityError { reason: String },
    
    #[error("Determinism violation: execution ID mismatch")]
    DeterminismViolation,
}
```

### API Error Responses

```rust
// src/app/server/dto.rs
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: String,
    pub code: String,
    pub details: Option<serde_json::Value>,
    pub suggestions: Option<Vec<String>>,
}

impl From<ContextError> for ApiErrorResponse {
    fn from(err: ContextError) -> Self {
        match err {
            ContextError::IntentGateViolation { allowed, .. } => {
                Self {
                    error: err.to_string(),
                    code: "INTENT_GATE_VIOLATION".to_string(),
                    details: None,
                    suggestions: Some(allowed),
                }
            }
            // ... other error mappings
        }
    }
}
```

## Testing Strategy

### Unit Testing Approach

1. **Scorer Tests**: Property-based tests for score bounds, monotonicity
2. **Selector Tests**: MMR algorithm correctness, budget constraints
3. **Merger Tests**: Content formatting, token counting accuracy
4. **Determinism Tests**: Identical input/output verification
5. **Similarity Tests**: Threshold behavior, cache hit/miss

### Integration Testing

1. **API Endpoint Tests**: Full request/response cycle
2. **Database Tests**: Transaction isolation, concurrent access
3. **MCP Protocol Tests**: JSON-RPC 2.0 compliance
4. **End-to-End Tests**: Complete classify → validate → compose flow

### Performance Testing

1. **Load Testing**: Concurrent request handling
2. **Memory Testing**: Large document processing
3. **Database Testing**: Query performance with large datasets
4. **Cache Testing**: Hit ratio optimization

## Configuration Management

### Constants Organization

```rust
// src/common/constants/scoring.rs
pub const DEFAULT_KEYWORD_WEIGHT: f32 = 0.3;
pub const DEFAULT_FRESHNESS_WEIGHT: f32 = 0.2;
pub const DEFAULT_TRUST_WEIGHT: f32 = 0.2;
pub const DEFAULT_CONFIDENCE_WEIGHT: f32 = 0.15;
pub const DEFAULT_FACET_COVERAGE_WEIGHT: f32 = 0.1;
pub const DEFAULT_AXES_MATCH_WEIGHT: f32 = 0.05;

// src/common/constants/determinism.rs
pub const SIMILARITY_THRESHOLD: f32 = 0.85;
pub const FACET_SIMILARITY_WEIGHT: f32 = 0.4;
pub const TEXT_SIMILARITY_WEIGHT: f32 = 0.4;
pub const STRUCT_SIMILARITY_WEIGHT: f32 = 0.2;

// src/common/constants/composition.rs
pub const DEFAULT_MMR_LAMBDA: f32 = 0.3;
pub const DEFAULT_TOKEN_BUDGET: usize = 2000;
pub const DEFAULT_TOKEN_RESERVE: usize = 200;
pub const MAX_CANDIDATES_TO_SCORE: usize = 1000;
```

### Version Management

```rust
// src/common/constants/versions.rs
pub const RULES_VERSION: &str = "1.0.0";
pub const RECIPE_VERSION: &str = "1.0.0";
pub const CONSTANTS_VERSION: &str = "1.0.0";
pub const ONTOLOGY_VERSION: &str = "1.0.0";

// Version hash for determinism
pub fn get_version_hash() -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(RULES_VERSION);
    hasher.update(RECIPE_VERSION);
    hasher.update(CONSTANTS_VERSION);
    hasher.update(ONTOLOGY_VERSION);
    format!("{:x}", hasher.finalize())[..16].to_string()
}
```

## Security Considerations

1. **Input Validation**: Sanitize all user inputs, prevent injection attacks
2. **Rate Limiting**: Implement per-client request limits
3. **Authentication**: Token-based auth for API endpoints (future)
4. **Data Privacy**: Avoid logging sensitive content
5. **Resource Limits**: Prevent DoS through large requests

## Performance Optimizations

1. **Database Indexing**: Optimize queries with proper indexes
2. **Connection Pooling**: Reuse database connections
3. **Caching Strategy**: Cache expensive computations
4. **Lazy Loading**: Load document content only when needed
5. **Parallel Processing**: Use rayon for CPU-intensive tasks

## Deployment Considerations

1. **Configuration**: Environment-based config management
2. **Monitoring**: Health checks, metrics collection
3. **Logging**: Structured logging with appropriate levels
4. **Graceful Shutdown**: Handle SIGTERM properly
5. **Resource Management**: Memory and file handle limits## 
Implementation Phases

### Phase 1: Core Foundation (High Priority)
1. **Complete Scoring System** - Implement all scorer modules with proper algorithms
2. **Document Selector** - MMR algorithm with greedy fallback
3. **Document Merger** - Content formatting and token management
4. **Enhanced Database Schema** - Add indexes and determinism tables

### Phase 2: API and Validation (Medium Priority)
1. **API Validation Layer** - Input validation and error handling
2. **Intent Gate Implementation** - Category/action compatibility checking
3. **Enhanced Error Responses** - Structured error messages with suggestions
4. **MCP Server Improvements** - Better error handling and async support

### Phase 3: Determinism and Optimization (Lower Priority)
1. **Determinism Engine** - Similarity-stable caching (can be simplified initially)
2. **Performance Optimizations** - Connection pooling, query optimization
3. **Advanced Testing** - Property-based tests, load testing
4. **Monitoring and Metrics** - Health checks, performance metrics

## Simplified Initial Implementation

For the initial implementation, we can simplify some complex features:

### Determinism Engine Simplification
- Start with basic execution ID generation (without similarity-stable caching)
- Add similarity-stable features in Phase 3
- Use simple in-memory cache initially

### MMR Algorithm Simplification  
- Start with greedy selection for all cases
- Add full MMR implementation once basic functionality works
- Use simple diversity metric (document type/category differences)

### Error Handling Simplification
- Start with basic error types and messages
- Add structured error responses with suggestions later
- Focus on preventing crashes first, user experience second

This phased approach ensures we can deliver working functionality quickly while building toward the full feature set.