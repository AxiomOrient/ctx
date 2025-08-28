# Dependency Graph Management

의존성 그래프 분석 및 최적화 모듈입니다.

## 주요 기능

- 문서 간 의존 관계 분석
- 순환 의존성 탐지 및 해결
- 의존성 기반 빌드 순서 최적화

## 구조

- `mod.rs` - 공통 트레이트 및 타입 정의
- `standard.rs` - 표준 의존성 그래프 매니저 구현

## 사용법

```rust
use ctxset::dependency::{DependencyGraphManager, StandardDependencyGraphManager};

let manager = StandardDependencyGraphManager::new();
// 의존성 분석 로직
```