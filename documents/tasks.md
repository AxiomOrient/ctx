# Implementation Plan

## Phase 1: Core Foundation (High Priority)

- [x] 0. Setup Core Infrastructure
  - Create missing constants and data structures needed for core implementation
  - Add enhanced error types and database schema
  - _Requirements: 7.1, 7.2, 8.1_

- [x] 0.1 Add missing constants definitions
  - Create `src/common/constants/composition.rs` with MMR_LAMBDA, TOKEN_BUDGET, etc.
  - Create `src/common/constants/determinism.rs` with SIMILARITY_THRESHOLD, weights
  - Update constants/mod.rs to export new constants
  - _Requirements: 7.1, 7.2_

- [x] 0.2 Enhance database schema with new tables and indexes
  - Update `src/data/index/sqlite_schema.sql` with execution_cache and similarity_cache tables
  - Add performance indexes for existing tables
  - Update SqliteIndexManager to handle schema migration
  - _Requirements: 6.1, 6.2, 6.3_

- [x] 0.3 Add core data structures for composition
  - Create CompositionResult, MergedDocument, MergedSection structs in `src/common/mod.rs`
  - Add ExecutionContext and CanonicalInput structs
  - Create unit tests for data structure serialization/deserialization
  - _Requirements: 1.6_

- [x] 1. Complete Document Scoring System
  - Implement missing scorer modules with proper algorithms
  - Add comprehensive scoring logic with weighted combinations
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_
  - _Depends on: 0.1_

- [x] 1.1 Implement keyword scorer with TF-IDF algorithm
  - Write keyword matching logic in `src/core/composer/scorer/keyword.rs`
  - Add TF-IDF calculation for better text relevance scoring
  - Create unit tests for keyword scoring accuracy
  - _Requirements: 2.1_

- [x] 1.2 Implement freshness scorer with exponential decay
  - Write freshness calculation in `src/core/composer/scorer/freshness.rs`
  - Add exponential decay formula: exp(-days_old / tau_days)
  - Create unit tests for freshness score bounds and decay behavior
  - _Requirements: 2.2_

- [x] 1.3 Implement facet coverage scorer
  - Write facet matching logic in `src/core/composer/scorer/facet_coverage.rs`
  - Add intersection ratio calculation between document and query facets
  - Create unit tests for facet coverage edge cases
  - _Requirements: 2.3_

- [x] 1.4 Complete similarity scorer implementation
  - Enhance `src/core/composer/scorer/similarity.rs` with Jaccard and trigram algorithms
  - Add proper similarity calculation between document pairs
  - Create unit tests for similarity score properties (symmetry, bounds)
  - _Requirements: 2.4_

- [x] 1.5 Integrate weighted scoring in DocumentScorer
  - Update `src/core/composer/scorer/mod.rs` with complete score combination logic
  - Add configurable weights from constants module
  - Create integration tests for end-to-end scoring
  - _Requirements: 2.5_

- [x] 2. Implement Document Selection with MMR Algorithm
  - Create document selector with MMR and greedy fallback strategies
  - Add token budget management and constraint handling
  - _Requirements: 1.3, 1.4_
  - _Depends on: 1.5_

- [x] 2.1 Create DocumentSelector structure and interface ✅ **COMPLETED**
  - ✅ Create `src/core/composer/selector.rs` file with DocumentSelector struct
  - ✅ Add basic constructor and interface methods (new, with_weights, with_mmr_lambda, etc.)
  - ✅ Add dependency on DocumentScorer for similarity calculations (Arc<DocumentScorer>)
  - ✅ Create unit tests for DocumentSelector creation (test_document_selector_creation, test_select_documents_interface)
  - _Requirements: 1.3_ ✅

- [x] 2.2 Implement MMR algorithm with diversity calculation ✅ **COMPLETED**
  - ✅ Add MMR selection logic with relevance vs diversity balance (mmr_selection method)
  - ✅ Implement diversity scoring based on document similarity (avg_similarity calculation)
  - ✅ Add lambda parameter for MMR tuning from constants (DEFAULT_MMR_LAMBDA)
  - ✅ Create unit tests for MMR algorithm correctness (test_mmr_algorithm_correctness, test_mmr_diversity_calculation, test_lambda_parameter_from_constants)
  - _Requirements: 1.3, 2.6_ ✅

- [x] 2.3 Add token budget constraint handling and greedy fallback ✅ **COMPLETED**
  - ✅ Add logic to respect token budget limits during selection (remaining_budget checks)
  - ✅ Implement greedy selection fallback for small document sets (GREEDY_SELECTION_THRESHOLD)
  - ✅ Add document truncation strategy when budget is exceeded (knapsack + greedy algorithms)
  - ✅ Create unit tests for budget constraint enforcement and fallback behavior (test_token_budget_constraint_enforcement, test_greedy_fallback_for_small_sets, test_budget_exceeded_truncation)
  - _Requirements: 1.4_ ✅

- [ ] 3. Implement Document Merger with Proper Formatting
  - Create document merger that combines selected documents into single prompt
  - Add section headers, source attribution, and metadata
  - _Requirements: 1.5, 1.6_
  - _Depends on: 0.3_

- [ ] 3.1 Create DocumentMerger structure and basic interface
  - Create `src/core/composer/merger.rs` file with DocumentMerger struct
  - Add constructor and basic merge method signature
  - Add dependency on Tokenizer trait for token counting
  - Create unit tests for DocumentMerger creation
  - _Requirements: 1.5_

- [ ] 3.2 Implement document merge logic with formatting
  - Add document combination logic with proper section headers
  - Implement content formatting with source attribution
  - Add metadata generation for merged documents
  - Create unit tests for merge logic and formatting consistency
  - _Requirements: 1.5_

- [ ] 3.3 Add token counting and composition metadata
  - Implement accurate token counting for merged documents
  - Generate composition metadata including source documents and rationale
  - Add confidence scoring for merged results
  - Create unit tests for token counting accuracy and metadata completeness
  - _Requirements: 1.6_

- [ ] 4. Complete BuildComposer Integration
  - Integrate scorer, selector, and merger into complete composition pipeline
  - Add error handling and result generation
  - _Requirements: 1.1, 1.2, 1.6_
  - _Depends on: 2.3, 3.3_

- [ ] 4.1 Create BuildComposer structure and basic integration
  - Create BuildComposer struct in `src/core/composer/mod.rs` with scorer, selector, merger fields
  - Add constructor that initializes all components
  - Add basic compose method signature and error handling
  - Create unit tests for BuildComposer creation and basic functionality
  - _Requirements: 1.1, 1.2_

- [ ] 4.2 Implement complete composition pipeline
  - Integrate scorer, selector, and merger into complete workflow
  - Add proper error handling and recovery strategies
  - Implement composition result generation with metadata
  - Create unit tests for complete composition process
  - _Requirements: 1.1, 1.2, 1.6_

- [ ] 4.3 Add composition rationale and confidence calculation
  - Implement rationale generation explaining document selection
  - Add confidence scoring for composition results
  - Add execution ID generation for determinism tracking
  - Create unit tests for rationale quality and confidence accuracy
  - _Requirements: 1.6, 4.1_

- [ ] 5. Integration Testing and Pipeline Validation
  - Test complete composition pipeline with real data
  - Validate end-to-end functionality
  - _Requirements: 9.3_
  - _Depends on: 4.3_

- [ ] 5.1 Create integration test suite for composition pipeline
  - Write integration tests that test complete classify → score → select → merge flow
  - Add test data fixtures with various document types and scenarios
  - Create performance baseline tests for composition pipeline
  - _Requirements: 9.3_

- [ ] 5.2 Add error handling integration tests
  - Test error propagation throughout the composition pipeline
  - Add tests for edge cases (empty candidates, budget exceeded, etc.)
  - Create tests for graceful degradation scenarios
  - _Requirements: 8.1, 8.2, 8.3_

- [ ] 5.3 Validate composition output quality
  - Add tests to verify merged document format and content quality
  - Test token counting accuracy across different document types
  - Create tests for metadata completeness and accuracy
  - _Requirements: 1.6_

## Phase 2: API and Validation (Medium Priority)

- [ ] 6. Implement API Validation Layer
  - Create comprehensive input validation for all API endpoints
  - Add structured error responses with helpful messages
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 6.1 Create ApiValidator structure and validation methods
  - Write `src/app/server/validation.rs` with input validation logic
  - Add validation for classify, validate, and compose requests
  - Create unit tests for validation rule enforcement
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 6.2 Implement Intent Gate validation
  - Add category/action compatibility checking logic
  - Create intent gate rules and policy enforcement
  - Create unit tests for intent gate violation detection
  - _Requirements: 3.5, 4.6_

- [ ] 6.3 Add structured error response generation
  - Enhance error types in `src/common/errors.rs` with detailed information
  - Add suggestion generation for validation failures
  - Create unit tests for error response format consistency
  - _Requirements: 3.4, 8.1_

- [ ] 7. Enhance HTTP API Endpoints
  - Complete missing API endpoint implementations
  - Add proper request/response handling and validation
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 7.1 Complete /v1/classify endpoint implementation
  - Update `src/app/server/http.rs` with complete classify logic
  - Add input validation and error handling
  - Create integration tests for classify endpoint
  - _Requirements: 3.1_

- [ ] 7.2 Complete /v1/validate endpoint implementation
  - Implement facet validation against task contracts
  - Add intent gate checking and policy enforcement
  - Create integration tests for validate endpoint
  - _Requirements: 3.2_

- [ ] 7.3 Complete /v1/compose endpoint implementation
  - Integrate BuildComposer into HTTP endpoint
  - Add proper request validation and response formatting
  - Create integration tests for compose endpoint
  - _Requirements: 3.3_

- [ ] 8. Improve MCP Server Implementation
  - Enhance JSON-RPC error handling and async support
  - Add missing MCP methods and improve existing ones
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

- [ ] 8.1 Enhance MCP error handling and JSON-RPC compliance
  - Update `src/app/cli/commands/mcp.rs` with proper JSON-RPC 2.0 error handling
  - Add structured error responses for all error conditions
  - Create unit tests for JSON-RPC compliance
  - _Requirements: 5.1, 8.1_

- [ ] 8.2 Improve classifyText and composePrompt methods
  - Add comprehensive input validation for MCP methods
  - Enhance error messages and response formatting
  - Create integration tests for MCP method functionality
  - _Requirements: 5.2, 5.3_

- [ ] 8.3 Add async support and performance improvements
  - Implement proper async handling for database operations
  - Add connection pooling for MCP server database access
  - Create performance tests for MCP server responsiveness
  - _Requirements: 5.4, 5.5, 10.3_

## Phase 3: Determinism and Optimization (Lower Priority)

- [ ] 9. Implement Determinism Engine
  - Create similarity-stable caching system
  - Add execution ID generation and canonical input processing
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

- [ ] 9.1 Create DeterminismEngine structure and basic functionality
  - Write `src/core/determinism.rs` with execution ID generation
  - Add canonical input normalization logic
  - Create unit tests for determinism engine core functionality
  - _Requirements: 4.1, 4.4, 4.5_

- [ ] 9.2 Implement similarity-stable caching
  - Add similarity calculation and threshold checking
  - Implement cache storage and retrieval logic
  - Create unit tests for similarity-stable behavior
  - _Requirements: 4.2, 4.3_

- [ ] 9.3 Integrate determinism engine with composition pipeline
  - Add determinism checking to BuildComposer workflow
  - Implement cache hit/miss tracking and metrics
  - Create integration tests for deterministic behavior
  - _Requirements: 4.1, 4.2, 4.3_

- [ ] 10. Add Performance Optimizations
  - Implement connection pooling and query optimization
  - Add caching strategies for expensive computations
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

- [ ] 10.1 Implement database connection pooling
  - Enhance database access with connection pooling
  - Add prepared statement caching for common queries
  - Create performance tests for database operations
  - _Requirements: 10.3_

- [ ] 10.2 Add computation caching strategies
  - Implement caching for similarity calculations and scoring
  - Add cache invalidation and cleanup logic
  - Create unit tests for cache behavior and performance
  - _Requirements: 10.4_

- [ ] 10.3 Optimize query performance
  - Analyze and optimize database queries for common operations
  - Add query execution time monitoring
  - Create performance benchmarks for critical operations
  - _Requirements: 10.1, 10.2_

- [ ] 11. Comprehensive Testing and Quality Assurance
  - Add property-based tests and integration test coverage
  - Implement performance testing and benchmarking
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_

- [ ] 11.1 Add property-based tests for scoring algorithms
  - Write property-based tests for score bounds and monotonicity
  - Add tests for scoring algorithm mathematical properties
  - Create test data generators for comprehensive coverage
  - _Requirements: 9.2_

- [ ] 11.2 Create comprehensive integration tests
  - Write end-to-end tests for complete API workflows
  - Add tests for error scenarios and edge cases
  - Create test fixtures and data management utilities
  - _Requirements: 9.3_

- [ ] 11.3 Implement performance testing and benchmarks
  - Create performance benchmarks for critical operations
  - Add load testing for concurrent request handling
  - Create performance regression detection tests
  - _Requirements: 9.1, 10.1, 10.2_

- [ ] 12. Configuration and Constants Management
  - Centralize all configuration parameters
  - Add version management for determinism tracking
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_

- [ ] 12.1 Organize constants modules
  - Review and organize all constants in `src/common/constants/`
  - Add version tracking for constants changes
  - Create unit tests for constants consistency
  - _Requirements: 7.1, 7.2_

- [ ] 12.2 Implement configuration version management
  - Add version hashing for determinism tracking
  - Implement configuration change detection
  - Create tests for version management functionality
  - _Requirements: 7.3, 7.4, 7.5_

## Testing Strategy by Phase

### Phase 1 Testing
- Unit tests for each scorer module
- Integration tests for composition pipeline
- Database schema migration tests
- Performance baseline establishment

### Phase 2 Testing  
- API endpoint integration tests
- MCP server JSON-RPC compliance tests
- Error handling and validation tests
- Intent gate policy tests

### Phase 3 Testing
- Property-based tests for determinism
- Load testing for performance optimization
- End-to-end workflow tests
- Regression testing for all features

## Success Criteria

### Phase 1 Complete When:
- All scorer modules pass unit tests with >70% coverage
- Complete composition pipeline produces valid merged documents
- Database operations perform within acceptable limits
- Basic API endpoints return valid responses

### Phase 2 Complete When:
- All API endpoints pass validation and error handling tests
- MCP server passes JSON-RPC 2.0 compliance tests
- Intent gate correctly enforces policy rules
- Error responses provide helpful suggestions

### Phase 3 Complete When:
- Deterministic behavior verified through identical input/output tests
- Performance targets met under typical load conditions
- Similarity-stable behavior verified through similar input tests
- Comprehensive test suite achieves target coverage
## Task
 Dependencies Summary

### Phase 1 Dependencies
```
0.1 (Constants) → 1.1, 1.2, 1.3, 1.4, 1.5
0.2 (Database) → 5.1, 5.2, 5.3 (moved to Phase 2)
0.3 (Data Structures) → 3.1, 4.1

1.1, 1.2, 1.3, 1.4 → 1.5 (Scoring Integration)
1.5 → 2.1 (Selector needs Scorer)
2.1 → 2.2 → 2.3 (Sequential selector implementation)
3.1 → 3.2 → 3.3 (Sequential merger implementation)
2.3, 3.3 → 4.1 (BuildComposer needs completed components)
4.1 → 4.2 → 4.3 (Sequential composer implementation)
4.3 → 5.1, 5.2, 5.3 (Integration testing)
```

### Critical Path
The critical path for Phase 1 is:
`0.1 → 1.1-1.4 → 1.5 → 2.1 → 2.2 → 2.3 → 4.1 → 4.2 → 4.3 → 5.1`

### Parallel Work Opportunities
- Tasks 1.1, 1.2, 1.3, 1.4 can be done in parallel after 0.1
- Tasks 0.2, 0.3 can be done in parallel with 1.x tasks
- Tasks 3.1, 3.2, 3.3 can be done in parallel with 2.x tasks (both depend on 0.3)

## Phase Completion Criteria

### Phase 1 Complete When:
- [ ] All scorer modules (keyword, freshness, facet_coverage, similarity) are implemented and tested
- [ ] DocumentSelector can successfully select documents using MMR algorithm within token budget
- [ ] DocumentMerger can combine selected documents into properly formatted merged document
- [ ] BuildComposer can execute complete composition pipeline end-to-end
- [ ] Integration tests pass for complete classify → score → select → merge workflow
- [ ] Performance baseline is established for composition operations

### Ready for Phase 2 When:
- [ ] Core composition functionality is stable and tested
- [ ] API endpoints can successfully call BuildComposer.compose()
- [ ] Error handling provides meaningful feedback for debugging
- [ ] Database schema supports all required operations