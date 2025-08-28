# Requirements Document

## Introduction

CTXSET (Deterministic Prompt Router & Composer)는 프롬프트 결정성과 유사-결정성을 제공하는 컨텍스트/프롬프트 서비스입니다. 현재 코드베이스는 아키텍처가 잘 설계되어 있지만 핵심 구현이 미완성 상태입니다. 이 요구사항 문서는 완전한 구현을 위한 기능적/비기능적 요구사항을 정의합니다.

## Requirements

### Requirement 1: Core Composer Implementation

**User Story:** As a developer, I want the composer to select and merge documents based on scoring algorithms, so that I can get deterministic prompt composition results.

#### Acceptance Criteria

1. WHEN a BuildQuery is provided THEN the system SHALL load candidates from SQLite index matching repo, branch, commit_sha
2. WHEN candidates are loaded THEN the system SHALL score them using multiple scoring algorithms (keyword, freshness, trust, confidence, facet coverage)
3. WHEN documents are scored THEN the system SHALL select optimal documents within token budget using MMR algorithm with diversity consideration
4. WHEN token budget is exceeded THEN the system SHALL prioritize highest-scoring documents and truncate or exclude lower-scoring ones
5. WHEN documents are selected THEN the system SHALL merge them into a single prompt with section headers and source attribution
6. WHEN composition is complete THEN the system SHALL return merged content with metadata, rationale, and token usage statistics

### Requirement 2: Complete Scoring System

**User Story:** As a system, I want to score documents using multiple criteria, so that the most relevant documents are selected for composition.

#### Acceptance Criteria

1. WHEN calculating keyword scores THEN the system SHALL use TF-IDF or token-based matching with query text
2. WHEN calculating freshness scores THEN the system SHALL apply exponential decay: exp(-days_old / tau_days)
3. WHEN calculating facet coverage THEN the system SHALL measure intersection ratio between document facets and required facets
4. WHEN calculating similarity THEN the system SHALL use Jaccard similarity for facets and trigram similarity for text
5. WHEN combining scores THEN the system SHALL use weighted sum: w_k*keyword + w_f*freshness + w_t*trust + w_c*confidence + w_fc*facet_coverage + w_ax*axes_match
6. WHEN applying MMR algorithm THEN the system SHALL balance relevance and diversity using lambda parameter: (1-λ)*relevance + λ*diversity

### Requirement 3: API Validation Layer

**User Story:** As an API consumer, I want input validation and proper error handling, so that I get clear feedback on invalid requests.

#### Acceptance Criteria

1. WHEN receiving /v1/classify requests THEN the system SHALL validate text and metadata parameters
2. WHEN receiving /v1/validate requests THEN the system SHALL check facets against task contracts and intent gates
3. WHEN receiving /v1/compose requests THEN the system SHALL validate repo, branch, commit_sha, and facets
4. WHEN validation fails THEN the system SHALL return structured error responses with specific error codes
5. WHEN intent gate violations occur THEN the system SHALL provide allowed actions in error response

### Requirement 4: Determinism and Similarity-stable Policy

**User Story:** As a user, I want consistent results for identical inputs and similar results for similar inputs, so that the system behavior is predictable.

#### Acceptance Criteria

1. WHEN processing identical inputs THEN the system SHALL generate identical execution IDs and results
2. WHEN calculating similarity THEN the system SHALL use formula: similarity = w_facet*FacetSim + w_text*TextSim + w_struct*StructSim
3. WHEN similarity >= SIMILARITY_THRESHOLD THEN the system SHALL reuse previous recipe and similar snippet sets
4. WHEN calculating execution ID THEN the system SHALL hash canonical_input, rules_version, recipe_version, commit_sha, constants_version
5. WHEN canonicalizing input THEN the system SHALL normalize whitespace, markdown, code fences, paths, URLs, and dates to standard format
6. WHEN applying Intent Gate THEN the system SHALL reject requests where task category conflicts with allowed actions (e.g., legacy_edit only allows refactor, migrate)

### Requirement 5: Complete MCP Server Implementation

**User Story:** As an LLM agent, I want to access classification and composition functions via MCP protocol, so that I can integrate with the ctxset service.

#### Acceptance Criteria

1. WHEN receiving MCP ping requests THEN the system SHALL respond with service status
2. WHEN receiving classifyText requests THEN the system SHALL classify text and return facets with confidence
3. WHEN receiving composePrompt requests THEN the system SHALL compose prompts and return merged content
4. WHEN receiving listContexts requests THEN the system SHALL return paginated document list
5. WHEN receiving getContext requests THEN the system SHALL return detailed document information

### Requirement 6: Database Schema and Indexing

**User Story:** As a system, I want efficient document storage and retrieval, so that queries perform well at scale.

#### Acceptance Criteria

1. WHEN storing documents THEN the system SHALL use temporal versioning with valid_from/valid_to
2. WHEN indexing facets THEN the system SHALL normalize and store in separate facet_values table
3. WHEN querying candidates THEN the system SHALL use efficient joins and avoid N+1 queries
4. WHEN updating documents THEN the system SHALL soft delete previous versions
5. WHEN clearing data THEN the system SHALL provide complete cleanup with VACUUM

### Requirement 7: Configuration and Constants Management

**User Story:** As a system administrator, I want centralized configuration management, so that scoring weights and thresholds can be controlled consistently.

#### Acceptance Criteria

1. WHEN loading constants THEN the system SHALL read from centralized constants modules
2. WHEN changing scoring weights THEN the system SHALL update constants and trigger version increment
3. WHEN validating thresholds THEN the system SHALL use configured similarity and confidence thresholds
4. WHEN calculating MMR THEN the system SHALL use configured lambda parameter
5. WHEN applying freshness decay THEN the system SHALL use configured tau_days parameter

### Requirement 8: Error Handling and Logging

**User Story:** As a developer, I want comprehensive error handling and logging, so that I can debug issues effectively.

#### Acceptance Criteria

1. WHEN errors occur THEN the system SHALL provide structured error responses with error codes
2. WHEN logging operations THEN the system SHALL use appropriate log levels (debug, info, warn, error)
3. WHEN handling database errors THEN the system SHALL provide meaningful error messages
4. WHEN processing files THEN the system SHALL handle missing files gracefully
5. WHEN validating input THEN the system SHALL provide specific validation error details

### Requirement 9: Testing and Quality Assurance

**User Story:** As a developer, I want comprehensive test coverage, so that the system reliability is ensured.

#### Acceptance Criteria

1. WHEN implementing core functions THEN the system SHALL include unit tests with >70% coverage (realistic target)
2. WHEN testing scoring algorithms THEN the system SHALL include property-based tests for score bounds and monotonicity
3. WHEN testing API endpoints THEN the system SHALL include integration tests with sample requests/responses
4. WHEN testing database operations THEN the system SHALL use temporary databases and test data isolation
5. WHEN testing MCP protocol THEN the system SHALL include JSON-RPC 2.0 compliance tests with error handling

### Requirement 10: Performance and Scalability

**User Story:** As a user, I want fast response times and efficient resource usage, so that the system scales well.

#### Acceptance Criteria

1. WHEN processing classification requests THEN the system SHALL respond within 200ms for documents <10KB (realistic target)
2. WHEN composing prompts THEN the system SHALL complete within 1000ms for queries with <100 candidates (realistic target)
3. WHEN loading candidates THEN the system SHALL use database connection pooling with prepared statements
4. WHEN calculating similarities THEN the system SHALL use efficient algorithms and avoid redundant computations
5. WHEN serving multiple requests THEN the system SHALL handle concurrent access safely with proper locking
## 
Requirements Dependencies

### Core Dependencies
- **Requirement 6** (Database Schema) is prerequisite for **Requirement 1** (Composer)
- **Requirement 2** (Scoring System) is prerequisite for **Requirement 1** (Composer)
- **Requirement 7** (Configuration) is prerequisite for **Requirement 2** (Scoring System)

### Implementation Priority
1. **High Priority**: Requirements 6, 7, 2, 1 (Core functionality)
2. **Medium Priority**: Requirements 3, 4, 8 (API and reliability)
3. **Low Priority**: Requirements 5, 9, 10 (Integration and optimization)

## Success Criteria

The implementation will be considered successful when:
1. All API endpoints return valid responses according to OpenAPI specification
2. Deterministic behavior is verified through identical input/output tests
3. Similarity-stable behavior is verified through similar input tests
4. MCP server passes JSON-RPC 2.0 compliance tests
5. Performance targets are met under typical load conditions