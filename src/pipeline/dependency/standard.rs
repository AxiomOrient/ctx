//! 표준 의존성 그래프 매니저 구현

use super::{DependencyGraphManager, DependencyStats};
use crate::domain::errors::{ContextError, Result};
use std::collections::{HashMap, HashSet, VecDeque};

/// 표준 의존성 그래프 매니저
#[derive(Debug, Clone)]
pub struct StandardDependencyGraphManager {
    /// 노드 ID -> 의존하는 노드들
    dependencies: HashMap<String, HashSet<String>>,
    /// 노드 ID -> 이 노드에 의존하는 노드들
    dependents: HashMap<String, HashSet<String>>,
}

impl StandardDependencyGraphManager {
    /// 새 매니저 생성
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
        }
    }

    /// 모든 노드 ID 반환
    pub fn get_all_nodes(&self) -> HashSet<String> {
        let mut nodes = HashSet::new();
        nodes.extend(self.dependencies.keys().cloned());
        nodes.extend(self.dependents.keys().cloned());
        nodes
    }

    /// DFS를 사용한 순환 탐지
    fn dfs_cycle_detection(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(deps) = self.dependencies.get(node) {
            for dep in deps {
                if !visited.contains(dep) {
                    if let Some(cycle) = self.dfs_cycle_detection(dep, visited, rec_stack, path) {
                        return Some(cycle);
                    }
                } else if rec_stack.contains(dep) {
                    // 순환 발견
                    if let Some(cycle_start) = path.iter().position(|x| x == dep) {
                        return Some(path[cycle_start..].to_vec());
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(node);
        None
    }
}

impl Default for StandardDependencyGraphManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraphManager for StandardDependencyGraphManager {
    fn add_dependency(&mut self, from: String, to: String) -> Result<()> {
        // from이 to에 의존한다는 의미
        self.dependencies
            .entry(from.clone())
            .or_default()
            .insert(to.clone());
        self.dependents.entry(to).or_default().insert(from);
        Ok(())
    }

    fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();

        for node in self.get_all_nodes() {
            if !visited.contains(&node) {
                let mut rec_stack = HashSet::new();
                let mut path = Vec::new();

                if let Some(cycle) =
                    self.dfs_cycle_detection(&node, &mut visited, &mut rec_stack, &mut path)
                {
                    cycles.push(cycle);
                }
            }
        }

        cycles
    }

    fn topological_sort(&self) -> Result<Vec<String>> {
        let nodes = self.get_all_nodes();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // 진입 차수 계산 - dependencies 그래프에서 진입 차수는
        // 해당 노드가 다른 노드에 의해 의존되는 횟수
        for node in &nodes {
            in_degree.insert(node.clone(), 0);
        }

        // from -> to 의존성에서 from이 to에 의존하므로
        // to가 먼저 실행되어야 함. 즉, from의 진입차수가 deps의 개수만큼 증가
        for (from, deps) in &self.dependencies {
            *in_degree.entry(from.clone()).or_insert(0) += deps.len();
        }

        // 진입 차수가 0인 노드들을 큐에 추가 (의존성이 없는 노드들)
        for (node, degree) in &in_degree {
            if *degree == 0 {
                queue.push_back(node.clone());
            }
        }

        // 토폴로지 정렬
        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            // 현재 노드에 의존하는 모든 노드들의 진입차수 감소
            if let Some(dependents) = self.dependents.get(&node) {
                for dependent in dependents {
                    if let Some(degree) = in_degree.get_mut(dependent) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        // 순환 의존성 검사
        if result.len() != nodes.len() {
            return Err(ContextError::Other("Circular dependency detected".into()));
        }

        Ok(result)
    }

    fn get_stats(&self) -> DependencyStats {
        let total_nodes = self.get_all_nodes().len();
        let total_edges: usize = self.dependencies.values().map(|deps| deps.len()).sum();
        let cycles = self.detect_cycles();

        // 최대 깊이 계산 (간단한 구현)
        let max_depth = self
            .dependencies
            .values()
            .map(|deps| deps.len())
            .max()
            .unwrap_or(0);

        DependencyStats {
            total_nodes,
            total_edges,
            cycles_count: cycles.len(),
            max_depth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_dependency() -> Result<()> {
        let mut manager = StandardDependencyGraphManager::new();

        // A depends on B, B depends on C
        manager.add_dependency("A".to_string(), "B".to_string())?;
        manager.add_dependency("B".to_string(), "C".to_string())?;

        let sorted = manager.topological_sort()?;

        // 올바른 순서: C (의존성 없음) -> B (C에 의존) -> A (B에 의존)
        // C가 먼저, A가 마지막에 와야 함
        assert_eq!(sorted, vec!["C", "B", "A"]);

        Ok(())
    }

    #[test]
    fn test_cycle_detection() -> Result<()> {
        let mut manager = StandardDependencyGraphManager::new();

        manager.add_dependency("A".to_string(), "B".to_string())?;
        manager.add_dependency("B".to_string(), "C".to_string())?;
        manager.add_dependency("C".to_string(), "A".to_string())?;

        let cycles = manager.detect_cycles();
        assert!(!cycles.is_empty());

        Ok(())
    }

    #[test]
    fn test_stats() -> Result<()> {
        let mut manager = StandardDependencyGraphManager::new();

        manager.add_dependency("A".to_string(), "B".to_string())?;
        manager.add_dependency("B".to_string(), "C".to_string())?;

        let stats = manager.get_stats();
        assert_eq!(stats.total_nodes, 3);
        assert_eq!(stats.total_edges, 2);
        assert_eq!(stats.cycles_count, 0);

        Ok(())
    }
}
