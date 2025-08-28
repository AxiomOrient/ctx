# Core 모듈 리팩토링 가이드

본 문서는 [AI 개발 원칙](../../../../../RULES.md)과 [Rust 개발 가이드](../../../../../RUST.md)를 엄격히 준수하여 작성된 전문가급 리팩토링 계획입니다.

## 🚨 Critical Issues (즉시 수정 필수)

### 1. 메모리 안전성 위반

**파일**: `cache/mod.rs:66-69`
```rust
// ❌ 현재: unsafe 블록 남용
cache: Arc<RwLock<LruCache::new(
    std::num::NonZero::new(max_size).unwrap_or_else(|| {
        // SAFETY: 1000 is always non-zero
        unsafe { std::num::NonZero::new_unchecked(1000) }
    }),
)>>,
```

**수정안**:
```rust
// ✅ 개선: 안전한 NonZero 생성
use std::num::NonZeroUsize;

impl<K, V> MemoryCache<K, V> 
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(max_size: usize, ttl_secs: u64) -> Result<Self, ContextError> {
        let capacity = NonZeroUsize::new(max_size.max(1))
            .ok_or_else(|| ContextError::ConfigError("Cache size must be positive".into()))?;
            
        Ok(Self {
            cache: Arc::new(RwLock::new(LruCache::new(capacity))),
            ttl: Duration::from_secs(ttl_secs),
            max_size,
        })
    }
}
```

**근거**: RUST.md 절대 금지사항 - "불필요한 `unsafe` 블록"

### 2. Panic 사용 금지

**파일**: `determinism.rs:252-256`
```rust
// ❌ 현재: Production에서 panic 사용
impl Default for DeterminismEngine {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| panic!("Failed to create default DeterminismEngine"))
    }
}
```

**수정안**:
```rust
// ✅ 개선: OnceLock을 사용한 안전한 Default
use std::sync::OnceLock;

static DEFAULT_ENGINE: OnceLock<DeterminismEngine> = OnceLock::new();

impl Default for DeterminismEngine {
    fn default() -> Self {
        DEFAULT_ENGINE
            .get_or_init(|| {
                Self::new().unwrap_or_else(|e| {
                    tracing::error!("Failed to create DeterminismEngine: {}", e);
                    // 최소 기능으로라도 동작하는 엔진 반환
                    Self::minimal()
                })
            })
            .clone()
    }
}

impl DeterminismEngine {
    fn minimal() -> Self {
        Self {
            whitespace_regex: Regex::new(r"\s+").expect("Basic regex should compile"),
            header_regex: Regex::new(r"^#+\s").expect("Basic regex should compile"),
            code_fence_regex: Regex::new(r"```").expect("Basic regex should compile"),
            url_regex: Regex::new(r"/$").expect("Basic regex should compile"),
            normalization_version: "1.0".to_string(),
        }
    }
}
```

**근거**: RUST.md 절대 금지사항 - "`unwrap()`/`expect()` (테스트 코드 제외)"

### 3. 알고리즘 버그 수정

**파일**: `dependency/standard.rs:117-121`
```rust
// ❌ 현재: 잘못된 토폴로지 정렬
for (from, deps) in &self.dependencies {
    for _dep in deps {
        *in_degree.entry(from.clone()).or_insert(0) += 1; // 논리 오류!
    }
}
```

**수정안**:
```rust
// ✅ 수정: 올바른 진입차수 계산
for (from, deps) in &self.dependencies {
    for dep in deps {
        *in_degree.entry(dep.clone()).or_insert(0) += 1;
    }
}
```

**근거**: RULES.md TIER 0 - "에러·경고 절대 규범: 원인 분석(RCA) 후 근본적 수정"

## 🔄 성능 최적화

### 1. Zero-Cost 추상화 구현

**파일**: `determinism.rs:122-165`
```rust
// ❌ 현재: 불필요한 String 할당
fn normalize_text(&self, text: &str) -> Result<String, ContextError> {
    let mut normalized = text.to_string();
    if NORMALIZE_WHITESPACE {
        normalized = self.whitespace_regex.replace_all(&normalized, " ").to_string();
    }
    // ... 반복적인 할당
}
```

**수정안**:
```rust
// ✅ 개선: Cow를 사용한 제로-코스트 추상화
use std::borrow::Cow;

fn normalize_text<'a>(&self, text: &'a str) -> Result<Cow<'a, str>, ContextError> {
    let mut result = Cow::Borrowed(text);
    
    if NORMALIZE_WHITESPACE && self.whitespace_regex.is_match(&result) {
        result = Cow::Owned(self.whitespace_regex.replace_all(&result, " ").into_owned());
    }
    
    if NORMALIZE_MARKDOWN_HEADERS && self.header_regex.is_match(&result) {
        result = Cow::Owned(self.header_regex.replace_all(&result, "# ").into_owned());
    }
    
    if NORMALIZE_CODE_FENCES && self.code_fence_regex.is_match(&result) {
        result = Cow::Owned(self.code_fence_regex.replace_all(&result, "```\n").into_owned());
    }
    
    if NORMALIZE_URLS && self.url_regex.is_match(&result) {
        result = Cow::Owned(
            self.url_regex
                .replace_all(&result, |caps: &regex::Captures| {
                    let url = caps.get(0).map_or("", |m| m.as_str());
                    url.trim_end_matches('/')
                })
                .into_owned()
        );
    }
    
    // 길이 제한 적용
    if result.len() > MAX_NORMALIZED_TEXT_LENGTH {
        let mut owned = result.into_owned();
        owned.truncate(MAX_NORMALIZED_TEXT_LENGTH);
        result = Cow::Owned(owned);
    }
    
    Ok(result)
}
```

**근거**: RUST.md 기본 철학 - "제로 코스트 추상화"

### 2. 캐시 동기화 최적화

**파일**: `cache/mod.rs:76-89`
```rust
// ❌ 현재: 읽기 작업에 write lock 사용
pub fn get(&self, key: &K) -> Option<V> {
    let mut cache = self.cache.write().ok()?;
    // ...
}
```

**수정안**:
```rust
// ✅ 개선: 읽기-쓰기 락 분리
pub fn get(&self, key: &K) -> Option<V> {
    // 먼저 읽기 락으로 확인
    {
        let cache = self.cache.read().ok()?;
        if let Some(entry) = cache.peek(key) {
            if !entry.is_expired(self.ttl) {
                return Some(entry.value.clone());
            }
        }
    }
    
    // 만료된 경우에만 쓰기 락으로 제거
    let mut cache = self.cache.write().ok()?;
    if let Some(entry) = cache.get_mut(key) {
        if entry.is_expired(self.ttl) {
            cache.pop(key);
            None
        } else {
            Some(entry.access().clone())
        }
    } else {
        None
    }
}
```

**근거**: RUST.md 기본 철학 - "명시적 동시성 제어: lock 범위 최소화"

## 🏗️ 아키텍처 개선

### 1. 의존성 주입 패턴

**파일**: `composer/mod.rs:37-50`
```rust
// ❌ 현재: 하드코딩된 의존성
pub struct BuildComposer {
    scorer: DocumentScorer,
    selector: DocumentSelector,
    merger: DocumentMerger,
}

impl BuildComposer {
    pub fn new() -> Self {
        Self {
            scorer: DocumentScorer::new(),
            selector: DocumentSelector::new(),
            merger: DocumentMerger::new(),
        }
    }
}
```

**수정안**:
```rust
// ✅ 개선: 트레이트 기반 의존성 주입
pub struct BuildComposer<S, L, M> {
    scorer: S,
    selector: L,
    merger: M,
}

impl<S, L, M> BuildComposer<S, L, M>
where
    S: DocumentScorer,
    L: DocumentSelector,
    M: DocumentMerger,
{
    pub fn new(scorer: S, selector: L, merger: M) -> Self {
        Self { scorer, selector, merger }
    }
}

// 기본 구현 제공
impl BuildComposer<DefaultDocumentScorer, DefaultDocumentSelector, DefaultDocumentMerger> {
    pub fn with_defaults() -> Self {
        Self::new(
            DefaultDocumentScorer::new(),
            DefaultDocumentSelector::new(),
            DefaultDocumentMerger::new(),
        )
    }
}
```

**근거**: RULES.md TIER 2 - "의존성 분리·DI 활용으로 테스트 가능성 확보"

### 2. 설정 구조체 도입

```rust
// ✅ 새로운 설정 시스템
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    pub determinism: DeterminismConfig,
    pub classification: ClassificationConfig,
    pub composition: CompositionConfig,
    pub caching: CachingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterminismConfig {
    pub normalize_whitespace: bool,
    pub normalize_markdown_headers: bool,
    pub normalize_code_fences: bool,
    pub normalize_urls: bool,
    pub max_input_size_bytes: usize,
    pub execution_id_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationConfig {
    pub keyword_weight: f32,
    pub link_weight: f32,
    pub regex_weight: f32,
    pub min_confidence: f32,
    pub max_confidence: f32,
    pub saturation_constant: f32,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            determinism: DeterminismConfig::default(),
            classification: ClassificationConfig::default(),
            composition: CompositionConfig::default(),
            caching: CachingConfig::default(),
        }
    }
}

impl CoreConfig {
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, ContextError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| ContextError::ConfigError(format!("Failed to read config: {}", e)))?;
        
        toml::from_str(&contents)
            .map_err(|e| ContextError::ConfigError(format!("Failed to parse config: {}", e)))
    }
}
```

**근거**: RULES.md TIER 2 - "설정 외부화"

## 🧪 테스트 전략 개선

### 1. 속성 기반 테스트

```rust
// ✅ 새로운 테스트 전략
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn determinism_consistency(
            input in ".*{0,1000}",
            facets in prop::collection::vec("\\w+", 0..10),
            metadata in prop::collection::hash_map("\\w+", ".*", 0..10)
        ) {
            let engine = DeterminismEngine::new().unwrap();
            let metadata_pairs: Vec<(String, String)> = metadata.into_iter().collect();
            
            let result1 = engine.normalize_input(&input, &facets, &metadata_pairs).unwrap();
            let result2 = engine.normalize_input(&input, &facets, &metadata_pairs).unwrap();
            
            prop_assert_eq!(result1, result2, "Determinism violated for same input");
            
            let id1 = engine.generate_execution_id(&result1);
            let id2 = engine.generate_execution_id(&result2);
            prop_assert_eq!(id1, id2, "Execution IDs must be identical for same input");
        }
        
        #[test]
        fn cache_correctness(
            keys in prop::collection::vec(0u64..1000u64, 1..100),
            values in prop::collection::vec(0f32..1.0f32, 1..100)
        ) {
            let cache = MemoryCache::new(50, 60).unwrap();
            
            // 값 저장
            for (key, value) in keys.iter().zip(values.iter()) {
                cache.put(*key, *value);
            }
            
            // 저장된 값 확인
            for (key, expected) in keys.iter().zip(values.iter()) {
                if let Some(actual) = cache.get(key) {
                    prop_assert_eq!(actual, *expected, "Cache corruption detected");
                }
            }
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_full_composition_pipeline() {
        let temp_dir = TempDir::new().unwrap();
        let config = CoreConfig::default();
        
        let composer = BuildComposer::with_config(config);
        let query = BuildQuery::new(
            "test-repo".to_string(),
            "main".to_string(),
            "abc123".to_string(),
        );
        
        let candidates = create_test_candidates(&temp_dir).await;
        let result = composer.compose(candidates, &query, 2000, 100).unwrap();
        
        // 결과 검증
        assert!(!result.merged_document.content.is_empty());
        assert!(result.merged_document.tokens > 0);
        assert!(result.average_confidence > 0.0);
    }
    
    async fn create_test_candidates(temp_dir: &TempDir) -> Vec<DocumentCandidate> {
        // 실제 파일 생성하여 통합 테스트
        vec![]
    }
}
```

**근거**: RULES.md TIER 3 - "테스트 피라미드(Unit > Integration > E2E) 유지"

## 🔧 에러 처리 개선

### 1. 구조화된 에러 타입

```rust
// ✅ 새로운 에러 시스템
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Determinism engine error: {source}")]
    Determinism {
        #[from]
        source: DeterminismError,
    },
    
    #[error("Classification error: {source}")]
    Classification {
        #[from]
        source: ClassificationError,
    },
    
    #[error("Composition error: {source}")]
    Composition {
        #[from]
        source: CompositionError,
    },
    
    #[error("Cache operation failed: {operation}")]
    Cache { operation: String },
    
    #[error("Configuration error: {message}")]
    Config { message: String },
}

#[derive(Error, Debug)]
pub enum DeterminismError {
    #[error("Input size {size} exceeds maximum {max_size}")]
    InputTooLarge { size: usize, max_size: usize },
    
    #[error("Regex compilation failed: {pattern}")]
    RegexCompilation { pattern: String },
    
    #[error("Normalization failed: {reason}")]
    Normalization { reason: String },
}

// Result 타입 별칭
pub type CoreResult<T> = Result<T, CoreError>;
pub type DeterminismResult<T> = Result<T, DeterminismError>;
```

**근거**: RUST.md 에러 처리 - "thiserror 기반, 메시지 명확"

## 📋 구현 계획

### Phase 1: 안전성 확보 (1주)
```markdown
## Phase 1 Tasks
- [ ] unsafe 블록 모두 제거 - `cache/mod.rs`
- [ ] panic 호출 모두 제거 - `determinism.rs`
- [ ] 토폴로지 정렬 알고리즘 수정 - `dependency/standard.rs`
- [ ] 해시 충돌 방지 로직 구현
- [ ] 전체 컴파일 및 테스트 통과 확인

### 성공 기준
- `cargo clippy -- -D warnings` 통과
- `cargo test --all` 100% 통과
- 메모리 누수 검사 통과
```

### Phase 2: 성능 최적화 (2주)
```markdown
## Phase 2 Tasks
- [ ] Cow<str> 도입으로 문자열 할당 최적화
- [ ] 캐시 동기화 개선 (읽기-쓰기 락 분리)
- [ ] 중복 계산 제거 (MMR, 해시 계산)
- [ ] 벤치마크 테스트 추가

### 성공 기준
- 성능 회귀 없음 (기존 대비 20% 향상)
- 메모리 사용량 10% 감소
- 처리량 15% 향상
```

### Phase 3: 아키텍처 개선 (3주)
```markdown
## Phase 3 Tasks
- [ ] 의존성 주입 패턴 구현
- [ ] 설정 시스템 구축 (TOML 기반)
- [ ] 에러 처리 표준화
- [ ] 트레이트 기반 인터페이스 설계

### 성공 기준
- 모듈 간 결합도 감소
- 테스트 커버리지 90% 이상
- 설정 파일로 동작 제어 가능
```

### Phase 4: 확장성 및 모니터링 (2주)
```markdown
## Phase 4 Tasks
- [ ] 플러그인 아키텍처 구현
- [ ] 메트릭 수집 시스템
- [ ] 동적 설정 로딩
- [ ] 성능 프로파일링 도구

### 성공 기준
- 새 알고리즘 플러그인 방식으로 추가 가능
- 런타임 메트릭 수집 및 모니터링
- 무중단 설정 변경 지원
```

## 📊 품질 게이트

### 모든 Phase 공통 요구사항
1. `cargo check` 성공
2. `cargo clippy --all-targets --all-features -- -D warnings` 경고 0개
3. `cargo test --all --all-features` 100% 통과
4. `cargo fmt --all --check` 통과
5. `cargo audit` 취약점 없음
6. 코드 커버리지 90% 이상
7. 메모리 누수 검사 통과
8. 벤치마크 테스트 성능 회귀 없음

### 각 PR 요구사항
- [ ] 변경 이유 명확히 기술
- [ ] 영향 받는 모듈 목록
- [ ] 테스트 시나리오 설명
- [ ] 성능 영향 측정 결과
- [ ] 리뷰어 2명 이상 승인

**최종 목표**: Production 환경에서 안전하고 고성능으로 동작하는 core 모듈 완성

---

*본 리팩토링 가이드는 RULES.md의 "에러·경고 절대 규범"과 RUST.md의 "소유권/빌림 시스템 준수" 원칙을 엄격히 따릅니다.*