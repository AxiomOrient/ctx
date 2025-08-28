# Rust 개발 가이드
> 본 문서는 **RULES.md(반영 원칙)**을 전제로 하며, **Rust 및 시스템 프로그래밍에 특화된 규칙만** 정의합니다.
> 범용 품질·PR·리뷰 가이드는 RULES.md를 참고하고, 본 문서는 **정책·경계·도구·검증**에 집중합니다.
---
## 0. 적용 범위 및 정책
### 0.1 플랫폼/버전 표준화
* **Edition**: 워크스페이스 단위로 고정(현재 `2021`, 필요 시 `2024`로 승격—승격 시 마이그레이션 가이드 필수).
* **MSRV**: 최소 지원 러스트 버전 명시(예: `rust-version = "1.70"`), CI에서 MSRV 매트릭스로 검증.
* **Target Triples**: 지원 대상(OS/Arch) 명시(예: `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`).
* **Feature Gate**: `default-features = false` 원칙. 실험 기능은 `experimental_*`로 **opt-in**.
### 0.2 API 안정성
* **SemVer 준수**: 공개 API의 파괴적 변경은 **메이저 버전** 증가 + 마이그레이션 가이드.
* **확장 방지**: 필요 시 `#[non_exhaustive]`로 확장 여지 통제.
* **문서화**: 모든 공개 API는 **rustdoc + 실행 가능한 doc-test** 제공.
---
## Quickstart (온보딩 1페이지)
```bash
# 포맷 & 린트 (경고=실패)
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
# 빌드 & 테스트
cargo check --all-targets --all-features
cargo test --all --all-features
# 의존성 보안 점검
cargo audit
```
**좋은 예 (소유권 남용 방지):**
```rust
fn process(data: Vec<String>) -> Vec<String> {
data.into_iter().filter(|s| !s.is_empty()).collect()
}
```
**나쁜 예 (불필요한 clone):**
```rust
fn process_bad(data: &Vec<String>) -> Vec<String> {
data.clone().into_iter().collect()
}
```
---
## 1. 안전성(Unsafe) 정책
* **기본 정책**: 라이브러리 크레이트는 **`#![forbid(unsafe_code)]`**.
불가피한 경우, **모듈 단위**로만 `#![allow(unsafe_code)]` 허용.
* **Unsafe 허용 절차(필수)**
1. **안전 래퍼** 제공(외부엔 안전 API만 노출)
2. **불변식/스레드·메모리 안전 근거**를 코드 주석으로 문서화
3. **검증 강화**: Miri, fuzz, property 테스트 추가
4. 리뷰 시 **감사 표준폼**(아래 §12.2) 첨부
```rust
/// SAFETY:
/// - `bytes`는 유효한 UTF-8이어야 한다.
/// - 호출자가 불변식을 보장한다.
/// 위 조건이 만족되므로 안전하다.
pub unsafe fn from_utf8_unchecked_wrapper(bytes: &[u8]) -> &str {
std::str::from_utf8_unchecked(bytes)
}
```

---

## 2. FFI/ABI 경계 규칙

* **ABI 표준**: C ABI만 허용. 공개 구조체는 `#[repr(C)]`; enum은 `#[repr(C)]` 또는 변환 계층 제공.
* **포인터 안전**: 길이/소유권/수명/정렬/엔디안/NUL-종결 요구 사항 명시.
* **도구**: `cbindgen`/`bindgen`으로 시그니처 자동화, **ABI 호환 테스트(FFI round-trip)** 필수.
* **안전 래퍼**: FFI 경계 바깥으로는 안전 API만 노출.

---

## 3. 에러 처리 정책

* **계층 분리**

  * **라이브러리**: `thiserror` 기반 도메인 오류 타입. 외부 오류는 `#[from]`으로 체이닝.
  * **애플리케이션 경계**: `anyhow` + `.context()`로 의미 있는 실패 메시지 제공.
* **금지 사항**: `unwrap()/expect()` (테스트 제외), 결과 무시(`let _ = ...`) 지양, `anyhow`의 라이브러리 내부 역류 금지.

```rust
use anyhow::{Context, Result};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("invalid field: {field}")]
    InvalidField { field: String },
}

pub fn parse_user(s: &str) -> Result<User, ValidationError> { /* ... */ }

pub fn process_user(data: &str) -> Result<User> {
    parse_user(data).context("failed to process user")
}
```

---

## 4. 동시성·Async 규칙

* **런타임 표준**: 워크스페이스는 **하나의 async 런타임만** 채택(예: Tokio). 혼용 금지.
* **구조적 동시성**: 생성한 태스크 핸들은 반드시 **own + await/abort**. “떠다니는 태스크” 금지.
* **차단 호출 격리**: 파일 I/O, CPU 바운드 작업은 `spawn_blocking`으로 분리.
* **취소/타임아웃**: 모든 외부 I/O·대기점에 **타임아웃** 지정. 상위 취소 신호를 **전파**.
* **Backpressure**: 채널은 **버퍼 크기·드롭 전략** 명시. 무한 버퍼 금지.
* **락 최소화**: `Arc<Mutex<T>>` 임계구역 최소화. 가능한 `RwLock`/채널/lock-free 구조 채택.
* **`Send/Sync`**: 스레드 간 이동 가능성 문서화. 필요 시 명시적으로 제한(`!Send/!Sync` 보장).

```rust
use anyhow::Result;
use tokio::{task, time};
use std::time::Duration;

pub async fn read_large_file(path: &str) -> Result<Vec<u8>> {
    let timeout = Duration::from_secs(10);
    let read = task::spawn_blocking({
        let p = path.to_owned();
        move || std::fs::read(p)
    });
    let bytes = time::timeout(timeout, read)
        .await
        .map_err(|_| anyhow::anyhow!("I/O timeout"))??; // JoinError, io::Error 처리
    Ok(bytes)
}

pub async fn structured_two_tasks() -> Result<(u32, u32)> {
    let h1 = tokio::spawn(async { Ok::<_, anyhow::Error>(compute_a().await) });
    let h2 = tokio::spawn(async { Ok::<_, anyhow::Error>(compute_b().await) });

    let (r1, r2) = tokio::join!(h1, h2);
    Ok((r1??, r2??))
}
```

---

## 5. API·모듈 설계

* **Newtype/ID 강화**: 원시 타입 남용 금지—의미 있는 타입으로 의도 표현.
* **노출면 최소화**: `pub(crate)` 우선, 불필요한 `pub` 축소.
* **문서화**: 공개 API는 rustdoc 필수, 예제는 **doc-test**로 검증.
* **자동 파생**: `Debug/Clone` 등 필요한 최소한만. 의미 왜곡 파생 금지.
* **확장 안정성**: 공개 enum/struct에 `#[non_exhaustive]` 고려.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(std::num::NonZeroU64);
```

---

## 6. 관측성(Logging/Tracing)

* **표준 스택**: `tracing` + 애플리케이션 레벨 `tracing-subscriber` 구성.
* **필수 필드 스키마**(최소): `request_id`, `user_id`, `latency_ms`, `target`(모듈명).
* **레벨 정책**: 라이브러리=ERROR/WARN/INFO 중심, 앱=환경별 레벨 전환.
* **보안/PII**: 민감 데이터는 마스킹·수집 금지. 비밀 키는 메모리 **제로화**(`secrecy/zeroize`).

```rust
use tracing::{instrument, Span, info};

#[instrument(fields(request_id = %req.id, user_id = req.user_id, latency_ms = tracing::field::Empty))]
pub async fn handle(req: Request) -> anyhow::Result<Response> {
    let start = std::time::Instant::now();
    let resp = process(req).await?;
    Span::current().record("latency_ms", start.elapsed().as_millis());
    info!("request ok");
    Ok(resp)
}
```

---

## 7. 메모리·성능

* **할당 전략**: 작은 컬렉션은 스택 우선(`SmallVec`, `arrayvec` 활용).
* **카피 비용**: `clone()` 남용 금지—소유권 이전/슬라이스/이터레이터 활용.
* **컴파일타임 최적화**: 측정 기반 `#[inline]`, 가능한 로직은 `const fn`.
* **벤치마크**: `criterion` 도입, 기준선 그래프 관리.

---

## 8. 테스트·검증(고급)

* **계층**: 단위(`#[test]`), 비동기(`#[tokio::test]`), 통합(`/tests`), **doc-tests**.
* **프로퍼티 테스트**: `proptest`로 입력 공간 탐색.
* **퍼즈**: `cargo-fuzz`로 파서/FFI/핵심 경로 퍼징.
* **Miri/Sanitizer**: UB 감지(miri), 가능 시 ASan/TSan 매트릭스 포함.

```rust
#[cfg(test)]
mod property_tests {
    use proptest::prelude::*;
    proptest! {
        #[test]
        fn serialize_roundtrip(data: Vec<u8>) {
            let ser = serialize(&data).unwrap();
            let de  = deserialize(&ser).unwrap();
            prop_assert_eq!(data, de);
        }
    }
}
```

---

## 9. 보안·의존성

* **라이선스/취약점**: `cargo-deny`(licenses/advisories/bans) 필수, `cargo audit` 병행.
* **불용 의존**: `cargo udeps`로 제거. **구버전 정리**: `cargo outdated`.
* **Secrets**: 코드/리포지토리에 비밀 저장 금지. 런타임 주입/전용 보관소 사용.

---

## 10. 빌드·배포·재현성

* **build.rs 제한**: 꼭 필요한 경우(FFI/코드 생성)만 사용. 환경 의존 로직 금지.
* **Reproducible Build**: 고정 버전(lockfile), 일관 RUSTFLAGS/프로파일, 결정적 빌드.
* **panic 전략**:

  * **바이너리**: 기본(unwind) 또는 성능·크기 요구 시 `abort`—선택 사유 문서화.
  * **라이브러리**: panic 유출 금지(결과로 신호).

`.cargo/config.toml` 예시:

```toml
[build]
# 환경에 맞게 조정. 재현성 목적이면 과도한 최적화 지양.
rustflags = []

[profile.release]
debug = true
lto = true
codegen-units = 1
# panic = "abort"  # 채택 시 반드시 문서화
```

---

## 11. 워크스페이스 운영

* **단일 런타임/로깅 표준 공유**: 전 크레이트 동일 정책.
* **공통 프렐루드**: 빈번한 트레이트/타입은 `crate::prelude`로 재노출.
* **examples/**: 실행 가능한 예시 제공, 문서와 동기화(doc-test).

```rust
// crates/common/src/prelude.rs
pub use anyhow::{Result, Context as _};
pub use tracing::{debug, error, info, warn};
```

---

## 12. 체크리스트 & 표준 폼

### 12.1 동시성 리뷰 체크(6문항)

* [ ] 모든 외부 I/O·대기점에 **타임아웃**이 있는가?
* [ ] 생성된 태스크는 **소유·await/abort**로 회수되는가?
* [ ] 차단 호출은 `spawn_blocking`으로 **격리**했는가?
* [ ] 락 범위가 **최소화**되었는가(경합·데드락 가능성 점검)?
* [ ] 채널 **버퍼/드롭 전략**이 명시되었는가(Backpressure)?
* [ ] 타입의 **`Send/Sync`** 보장이 명시·검증되었는가?

### 12.2 Unsafe/FFI 감사 표준폼 (PR에 첨부)

```md
# Unsafe/FFI Audit

- 위치(모듈/라인):
- 동기(왜 unsafe/FFI가 필요한가):
- 불변식(메모리/동시성/라이프타임):
- 대안 검토(unsafe 회피 가능성):
- 검증(테스트/Miri/Fuzz 링크):
- 외부 노출(안전 래퍼 유무):
- ABI/레이아웃(#[repr(C)] 여부, 정렬/엔디안/호출규약):
- 리스크/완화:
```

### 12.3 변경 시 확인사항

* [ ] 공개 API 변경 → **semver**/마이그레이션 가이드
* [ ] unsafe 추가/수정 → **감사 표준폼 + 테스트 보강**
* [ ] 의존성 변경 → **cargo-deny/audit** 통과
* [ ] 성능 변경 → **criterion 기준선** 갱신
* [ ] FFI 변경 → **round-trip/ABI 호환** 확인

### 12.4 릴리스 전 검증

* [ ] 모든 **feature 조합** 빌드 성공
* [ ] **Miri** UB 검사 통과(핵심 크레이트)
* [ ] **보안** 점검 통과(cargo audit/deny)
* [ ] 문서 빌드 & **doc-test** 통과
* [ ] **examples/** 실행 확인

---

## 13. 도구체인 & CI 품질 게이트

### 13.1 권장 도구

* 코드 품질: `clippy`, `rustfmt`, `nextest`, `llvm-cov`
* 검증: `miri`, `cargo-fuzz`, `proptest`
* 보안/의존성: `cargo-deny`, `cargo-audit`, `cargo-udeps`, `cargo-outdated`

### 13.2 CI 스크립트 예시

```bash
#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo check --all-targets --all-features
cargo test --all-features

# 고급 게이트
cargo deny check
cargo udeps --all-targets
cargo audit || true         # 알려진 취약점은 PR에서 명시적으로 승인
cargo nextest run --all-features

# 선택: 커버리지/UB/퍼징(핵심 경로만)
# cargo llvm-cov --lcov --output-path lcov.info
# cargo miri test
# timeout 60s cargo fuzz run fuzz_target_1 || true
```

### 13.3 단계적 롤아웃(4주 로드맵)

* **W1**: `fmt/clippy -D warnings`, unit/integration/nextest, `tracing` 필드 스키마 적용
* **W2**: `cargo-deny/audit/udeps/outdated` 도입, 위반 항목 대시보드화
* **W3**: 핵심 크레이트에 **Miri**, 퍼즈 타깃 1개 도입
* **W4**: 리뷰 템플릿에 **동시성 체크리스트(§12.1)** 강제

---

### 부록 A. 예시: 타임아웃·취소 전파 패턴

```rust
use tokio::{time, select};
use std::time::Duration;

pub async fn fetch_with_cancel(cancel: tokio::sync::watch::Receiver<bool>) -> anyhow::Result<()> {
    let op = async {
        // ... 외부 I/O
        Ok::<_, anyhow::Error>(())
    };
    select! {
        _ = time::sleep(Duration::from_secs(5)) => Err(anyhow::anyhow!("timeout")),
        _ = async {
            while !*cancel.borrow() { cancel.changed().await.ok(); }
        } => Err(anyhow::anyhow!("cancelled")),
        r = op => r,
    }
}
```

---
