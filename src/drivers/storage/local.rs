use super::Storage;
use crate::domain::errors::{ContextError, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// 로컬 파일시스템 기반 Storage 구현체
#[derive(Debug, Clone)]
pub struct LocalFsStorage {
    base_path: PathBuf,
}

impl LocalFsStorage {
    /// 새로운 LocalFsStorage 인스턴스 생성
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// 절대 경로 생성 (경로 탐색 공격 방지)
    fn resolve_path(&self, path: &Path) -> Result<PathBuf> {
        // Build a full path under base_path
        let joined = if path.is_absolute() { path.to_path_buf() } else { self.base_path.join(path) };

        // Normalize components without requiring existence
        let normalized = self.normalize_path(&joined);

        // Canonicalize base and parent if possible for secure comparison
        let base_canonical = self
            .base_path
            .canonicalize()
            .unwrap_or_else(|_| self.normalize_path(&self.base_path));

        let candidate_canonical = if let Some(parent) = normalized.parent() {
            if parent.exists() {
                parent
                    .canonicalize()
                    .map(|p| p.join(normalized.file_name().unwrap_or_default()))
                    .unwrap_or_else(|_| normalized.clone())
            } else {
                // Parent doesn't exist; approximate by joining to canonical base
                base_canonical.join(path)
            }
        } else {
            base_canonical.join(path)
        };

        if !candidate_canonical.starts_with(&base_canonical) {
            return Err(ContextError::Other(format!(
                "Path traversal attempt detected: {:?} (resolved to {:?}, base: {:?})",
                path, candidate_canonical, base_canonical
            )));
        }

        Ok(normalized)
    }

    /// 경로 정규화 (존재하지 않는 파일도 처리 가능)
    /// 보안 강화: 경로 탐색 공격 방지를 위한 엄격한 정규화
    fn normalize_path(&self, path: &Path) -> PathBuf {
        let mut components = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    // ".." 컴포넌트는 스택에서 제거하되, 루트를 벗어나지 않도록 보호
                    components.pop();
                }
                std::path::Component::CurDir => {
                    // "." 컴포넌트는 무시
                }
                std::path::Component::Normal(name) => {
                    // 위험한 파일명 패턴 검사
                    let name_str = name.to_string_lossy();
                    if name_str.contains('\0') || name_str == "." || name_str == ".." {
                        // NULL 바이트나 특수 디렉토리명은 거부
                        continue;
                    }
                    components.push(component);
                }
                _ => {
                    components.push(component);
                }
            }
        }

        components.iter().collect()
    }
}

impl Storage for LocalFsStorage {
    fn read(&self, path: &Path) -> Result<String> {
        let full_path = self.resolve_path(path)?;
        fs::read_to_string(&full_path).map_err(ContextError::Io)
    }

    fn write(&self, path: &Path, content: &str) -> Result<()> {
        let full_path = self.resolve_path(path)?;

        // 부모 디렉토리가 없으면 생성
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&full_path, content).map_err(ContextError::Io)
    }

    fn list_files(&self, folder: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        let full_folder = self.resolve_path(folder)?;

        if !full_folder.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        let entries = fs::read_dir(&full_folder)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    // 간단한 glob 패턴 매칭 (*.md 등)
                    if pattern == "*" || file_name.ends_with(&pattern.replace("*", "")) {
                        // base_path에 대한 상대 경로로 변환
                        let relative_path = path.strip_prefix(&self.base_path).unwrap_or(&path);
                        files.push(relative_path.to_path_buf());
                    }
                }
            } else if path.is_dir() {
                // 재귀적으로 하위 디렉토리 탐색 - base path 중복 join 방지
                let relative_path = path.strip_prefix(&self.base_path).unwrap_or(&path);
                let sub_files = self.list_files(relative_path, pattern)?;
                files.extend(sub_files);
            }
        }

        // Deterministic ordering for determinism guarantees
        files.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
        Ok(files)
    }

    fn exists(&self, path: &Path) -> bool {
        self.resolve_path(path).map(|p| p.exists()).unwrap_or(false)
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path)?;
        fs::create_dir_all(&full_path).map_err(ContextError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_read_write_file() -> Result<()> {
        let temp_dir = TempDir::new().map_err(crate::domain::errors::ContextError::Io)?;
        let storage = LocalFsStorage::new(temp_dir.path());

        let test_path = Path::new("test.txt");
        let test_content = "Hello, World!";

        // 쓰기
        storage.write(test_path, test_content)?;

        // 읽기
        let read_content = storage.read(test_path)?;
        assert_eq!(read_content, test_content);
        Ok(())
    }

    #[test]
    fn test_list_files() -> Result<()> {
        let temp_dir = TempDir::new().map_err(crate::domain::errors::ContextError::Io)?;
        let storage = LocalFsStorage::new(temp_dir.path());

        // 테스트 파일들 생성
        storage.write(Path::new("test1.md"), "content1")?;
        storage.write(Path::new("test2.md"), "content2")?;
        storage.write(Path::new("test3.txt"), "content3")?;

        // .md 파일만 조회
        let md_files = storage.list_files(Path::new("."), "*.md")?;
        assert_eq!(md_files.len(), 2);

        // 모든 파일 조회
        let all_files = storage.list_files(Path::new("."), "*")?;
        assert_eq!(all_files.len(), 3);
        Ok(())
    }

    #[test]
    fn test_exists() -> Result<()> {
        let temp_dir = TempDir::new().map_err(crate::domain::errors::ContextError::Io)?;
        let storage = LocalFsStorage::new(temp_dir.path());

        let test_path = Path::new("test.txt");

        // 파일이 없을 때
        assert!(!storage.exists(test_path));

        // 파일 생성 후
        storage.write(test_path, "content")?;
        assert!(storage.exists(test_path));
        Ok(())
    }

    #[test]
    fn test_path_traversal_blocked() {
        let temp_dir = TempDir::new().map_err(crate::domain::errors::ContextError::Io).unwrap();
        let storage = LocalFsStorage::new(temp_dir.path());

        // 상대 경로로 상위 디렉토리 탈출 시도는 거부되어야 함
        let attempt = std::path::Path::new("../../etc/passwd");
        let err = storage.read(attempt).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("Path traversal attempt detected"));
    }
}
