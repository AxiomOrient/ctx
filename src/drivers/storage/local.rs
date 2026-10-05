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

    /// Resolve existing targets and missing suffixes against the real workspace root.
    /// Symlinks are resolved before comparing containment, including the final entry.
    pub(crate) fn resolve_path(&self, path: &Path) -> Result<PathBuf> {
        let base = if self.base_path.is_absolute() {
            self.base_path.clone()
        } else {
            std::env::current_dir()?.join(&self.base_path)
        };
        let base_canonical = canonicalize_existing_prefix(&base)?;
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base_canonical.join(path)
        };
        let resolved = canonicalize_existing_prefix(&candidate)?;
        if !resolved.starts_with(&base_canonical) {
            return Err(ContextError::Other(format!(
                "Path traversal attempt detected: {:?} (resolved to {:?}, base: {:?})",
                path, resolved, base_canonical
            )));
        }
        Ok(resolved)
    }
}

/// Canonicalize the nearest existing entry, then append only ordinary missing names.
/// `symlink_metadata` distinguishes a dangling symlink from a genuinely missing path.
fn canonicalize_existing_prefix(path: &Path) -> Result<PathBuf> {
    let mut existing = path.to_path_buf();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(_) => {
                let mut resolved = fs::canonicalize(&existing)?;
                for name in missing.iter().rev() {
                    resolved.push(name);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = existing.file_name().ok_or_else(|| {
                    ContextError::Other(format!(
                        "Path traversal attempt detected: unresolved path {:?}",
                        path
                    ))
                })?;
                missing.push(name.to_os_string());
                if !existing.pop() {
                    return Err(ContextError::Io(error));
                }
            }
            Err(error) => return Err(ContextError::Io(error)),
        }
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
        let base_canonical = self.resolve_path(Path::new("."))?;

        if !full_folder.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        let entries = fs::read_dir(&full_folder)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            let file_type = entry.file_type()?;
            if file_type.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    // 간단한 glob 패턴 매칭 (*.md 등)
                    if pattern == "*" || file_name.ends_with(&pattern.replace("*", "")) {
                        // base_path에 대한 상대 경로로 변환
                        let relative_path = path.strip_prefix(&base_canonical).unwrap_or(&path);
                        files.push(relative_path.to_path_buf());
                    }
                }
            } else if file_type.is_dir() {
                // 재귀적으로 하위 디렉토리 탐색 - base path 중복 join 방지
                let relative_path = path.strip_prefix(&base_canonical).unwrap_or(&path);
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

    #[test]
    fn nested_writes_and_relative_listings_stay_under_root() -> Result<()> {
        let temp = TempDir::new()?;
        let root = temp.path().join("workspace");
        let storage = LocalFsStorage::new(&root);
        storage.write(Path::new("nested/document.md"), "inside")?;
        assert_eq!(storage.read(Path::new("nested/document.md"))?, "inside");
        assert_eq!(
            storage.list_files(Path::new("."), "*.md")?,
            vec![PathBuf::from("nested/document.md")]
        );
        assert_eq!(storage.read(&root.join("nested/document.md"))?, "inside");
        Ok(())
    }

    #[test]
    fn rejects_absolute_and_missing_parent_escapes() -> Result<()> {
        let temp = TempDir::new()?;
        let root = temp.path().join("workspace");
        fs::create_dir(&root)?;
        let outside = temp.path().join("outside.md");
        fs::write(&outside, "unchanged")?;
        let storage = LocalFsStorage::new(&root);
        assert!(storage.write(&outside, "changed").is_err());
        assert!(storage
            .write(Path::new("../missing/new.md"), "changed")
            .is_err());
        assert_eq!(fs::read_to_string(&outside)?, "unchanged");
        assert!(!temp.path().join("missing").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_leaf_parent_and_dangling_symlink_escapes() -> Result<()> {
        use std::os::unix::fs::symlink;
        let temp = TempDir::new()?;
        let root = temp.path().join("workspace");
        let outside_dir = temp.path().join("outside");
        fs::create_dir(&root)?;
        fs::create_dir(&outside_dir)?;
        let outside = outside_dir.join("private.md");
        fs::write(&outside, "unchanged")?;
        symlink(&outside, root.join("leaf.md"))?;
        symlink(&outside_dir, root.join("parent"))?;
        symlink(outside_dir.join("new.md"), root.join("dangling.md"))?;
        let storage = LocalFsStorage::new(&root);
        for path in [
            "leaf.md",
            "parent/private.md",
            "parent/nested/new.md",
            "dangling.md",
        ] {
            assert!(
                storage.read(Path::new(path)).is_err(),
                "read must reject {path}"
            );
            assert!(
                storage.write(Path::new(path), "changed").is_err(),
                "write must reject {path}"
            );
        }
        assert_eq!(fs::read_to_string(&outside)?, "unchanged");
        assert!(!outside_dir.join("new.md").exists());
        assert!(!outside_dir.join("nested").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn list_files_does_not_follow_symlink_cycles() -> Result<()> {
        use std::os::unix::fs::symlink;
        let temp = TempDir::new()?;
        fs::write(temp.path().join("document.md"), "inside")?;
        symlink(temp.path(), temp.path().join("cycle"))?;
        let storage = LocalFsStorage::new(temp.path());
        assert_eq!(
            storage.list_files(Path::new("."), "*.md")?,
            vec![PathBuf::from("document.md")]
        );
        Ok(())
    }
}
