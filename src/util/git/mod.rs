use crate::domain::errors::Result;
use std::path::Path;
use std::process::Command;

/// Git 변경사항
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GitChange {
    pub status: String, // A, M, D, R100, etc.
    pub path: String,
    pub new_path: Option<String>, // rename target
}

/// Git 유틸리티
#[allow(dead_code)]
pub struct GitUtils;

impl GitUtils {
    /// 현재 브랜치 이름 조회
    #[allow(dead_code)]
    pub fn current_branch<P: AsRef<Path>>(repo_path: P) -> Result<String> {
        let repo_str = repo_path
            .as_ref()
            .to_str()
            .ok_or_else(|| crate::domain::errors::ContextError::Other("Invalid repository path".to_string()))?;

        let output = Command::new("git")
            .args(["-C", repo_str, "rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .map_err(crate::domain::errors::ContextError::Io)?;

        if !output.status.success() {
            return Err(crate::domain::errors::ContextError::OptimizationError(
                "Failed to get current branch".to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// HEAD 커밋 SHA 조회
    #[allow(dead_code)]
    pub fn head_sha<P: AsRef<Path>>(repo_path: P) -> Result<String> {
        let repo_str = repo_path
            .as_ref()
            .to_str()
            .ok_or_else(|| crate::domain::errors::ContextError::Other("Invalid repository path".to_string()))?;

        let output = Command::new("git")
            .args(["-C", repo_str, "rev-parse", "HEAD"])
            .output()
            .map_err(crate::domain::errors::ContextError::Io)?;

        if !output.status.success() {
            return Err(crate::domain::errors::ContextError::OptimizationError(
                "Failed to get HEAD SHA".to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// 파일의 최초 추가 커밋 조회
    #[allow(dead_code)]
    pub fn first_add_commit<P: AsRef<Path>>(repo_path: P, file_path: &str) -> Result<String> {
        let repo_str = repo_path
            .as_ref()
            .to_str()
            .ok_or_else(|| crate::domain::errors::ContextError::Other("Invalid repository path".to_string()))?;

        let output = Command::new("git")
            .args([
                "-C",
                repo_str,
                "log",
                "--diff-filter=A",
                "--format=%H",
                "--",
                file_path,
            ])
            .output()
            .map_err(crate::domain::errors::ContextError::Io)?;

        if !output.status.success() {
            return Err(crate::domain::errors::ContextError::OptimizationError(
                "Failed to get first add commit".to_string(),
            ));
        }

        let commits = String::from_utf8_lossy(&output.stdout);
        let last_commit = commits.lines().last().unwrap_or("");
        if last_commit.is_empty() {
            return Err(crate::domain::errors::ContextError::OptimizationError(
                "No commit found for file".to_string(),
            ));
        }
        Ok(last_commit.to_string())
    }

    /// 모든 추적된 파일 목록 조회
    #[allow(dead_code)]
    pub fn list_all_files<P: AsRef<Path>>(repo_path: P) -> Result<Vec<String>> {
        let repo_str = repo_path
            .as_ref()
            .to_str()
            .ok_or_else(|| crate::domain::errors::ContextError::Other("Invalid repository path".to_string()))?;

        let output = Command::new("git")
            .args(["-C", repo_str, "ls-files"])
            .output()
            .map_err(crate::domain::errors::ContextError::Io)?;

        if !output.status.success() {
            return Err(crate::domain::errors::ContextError::OptimizationError(
                "Failed to list files".to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect())
    }

    /// 두 커밋 간의 변경사항 조회
    #[allow(dead_code)]
    pub fn diff_changes<P: AsRef<Path>>(
        repo_path: P,
        base_sha: &str,
        head_sha: &str,
    ) -> Result<Vec<GitChange>> {
        let repo_str = repo_path
            .as_ref()
            .to_str()
            .ok_or_else(|| crate::domain::errors::ContextError::Other("Invalid repository path".to_string()))?;

        let output = Command::new("git")
            .args([
                "-C",
                repo_str,
                "diff",
                "--name-status",
                "-M",
                "--diff-filter=ACDMR",
                &format!("{}..{}", base_sha, head_sha),
            ])
            .output()
            .map_err(crate::domain::errors::ContextError::Io)?;

        if !output.status.success() {
            return Err(crate::domain::errors::ContextError::OptimizationError(
                "Failed to get diff changes".to_string(),
            ));
        }

        let mut changes = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() {
                continue;
            }

            match parts[0] {
                status if status.starts_with('R') && parts.len() == 3 => {
                    changes.push(GitChange {
                        status: status.to_string(),
                        path: parts[1].to_string(),
                        new_path: Some(parts[2].to_string()),
                    });
                }
                status if parts.len() == 2 => {
                    changes.push(GitChange {
                        status: status.to_string(),
                        path: parts[1].to_string(),
                        new_path: None,
                    });
                }
                _ => {}
            }
        }

        Ok(changes)
    }

    /// 파일이 마크다운인지 확인
    #[allow(dead_code)]
    pub fn is_markdown_file(path: &str) -> bool {
        path.ends_with(".md") || path.ends_with(".markdown")
    }

    /// 충돌 마커가 있는 파일인지 확인
    #[allow(dead_code)]
    pub fn has_conflict_markers(content: &str) -> bool {
        content.contains("<<<<<<<") && content.contains("=======") && content.contains(">>>>>>>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_markdown_file() {
        assert!(GitUtils::is_markdown_file("test.md"));
        assert!(GitUtils::is_markdown_file("README.markdown"));
        assert!(!GitUtils::is_markdown_file("test.txt"));
        assert!(!GitUtils::is_markdown_file("test.rs"));
    }

    #[test]
    fn test_has_conflict_markers() {
        let conflict_content = r#"
Some content
<<<<<<< HEAD
This is from HEAD
=======
This is from branch
>>>>>>> branch
More content
"#;
        assert!(GitUtils::has_conflict_markers(conflict_content));

        let clean_content = "This is clean content";
        assert!(!GitUtils::has_conflict_markers(clean_content));
    }
}
