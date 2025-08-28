//! 의존성 그래프 관리 모듈
//!
//! 문서 간 의존 관계를 분석하고 최적화하는 기능을 제공합니다.

pub mod standard;

pub use standard::StandardDependencyGraphManager;

use crate::domain::errors::Result;
#[allow(unused_imports)]
use std::collections::HashMap;

/// 의존성 그래프 매니저 트레이트
pub trait DependencyGraphManager {
    /// 의존성 관계 추가
    fn add_dependency(&mut self, from: String, to: String) -> Result<()>;

    /// 순환 의존성 탐지
    fn detect_cycles(&self) -> Vec<Vec<String>>;

    /// 토폴로지 정렬 (빌드 순서)
    fn topological_sort(&self) -> Result<Vec<String>>;

    /// 의존성 통계
    fn get_stats(&self) -> DependencyStats;
}

/// 의존성 통계
#[derive(Debug, Clone)]
pub struct DependencyStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub cycles_count: usize,
    pub max_depth: usize,
}

/// 의존성 노드
#[derive(Debug, Clone)]
pub struct DependencyNode {
    pub id: String,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
}
