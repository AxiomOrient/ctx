//! Parse 명령어 구현
//!
//! Markdown 파일의 frontmatter를 파싱하고 검증합니다.

use ctx_core::doc::parse::FrontmatterParser;

// format_output은 사용하지 않으므로 제거
use colored::*;
use std::path::PathBuf;

/// Parse 명령어 인자
#[derive(Debug, Clone)]
pub struct ParseArgs {
    pub file: PathBuf,
    pub format: String,
}

/// Parse 명령어 실행
///
/// # Arguments
/// * `args` - parse 명령어 인자
///
/// # Returns
/// * 성공 시 Ok(()), 실패 시 ContextError
pub fn run_parse(args: ParseArgs) -> ctx_core::Result<()> {
    let content = std::fs::read_to_string(&args.file)?;
    let parser = FrontmatterParser::new();

    let (metadata, _) = parser.parse(&content)?;
    println!("{}", "✓ Frontmatter is valid".green());

    let output = match args.format.as_str() {
        "yaml" => serde_yaml::to_string(&metadata)?,
        "json" => serde_json::to_string_pretty(&metadata)?,
        _ => {
            return Err(ctx_core::ContextError::AssemblyError(format!(
                "Unsupported format: {}",
                args.format
            )));
        }
    };

    println!("{}", output);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_run_parse_valid() -> ctx_core::Result<()> {
        let mut temp_file = NamedTempFile::new().map_err(ctx_core::ContextError::Io)?;
        let content = "---\ntitle: \"Test Document\"\nversion: \"1.0.0\"\nsections:\n  - id: \"intro\"\n    name: \"Introduction\"\n    marker: \"## Introduction\"\n    priority: 90\n---\n\n## Introduction\nTest content.\n";

        temp_file
            .write_all(content.as_bytes())
            .map_err(ctx_core::ContextError::Io)?;

        let args = ParseArgs {
            file: temp_file.path().to_path_buf(),
            format: "yaml".to_string(),
        };

        let result = run_parse(args);
        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_run_parse_invalid_format() -> ctx_core::Result<()> {
        let mut temp_file = NamedTempFile::new().map_err(ctx_core::ContextError::Io)?;
        let content = "---\ntitle: \"Test Document\"\nversion: \"1.0.0\"\nsections:\n  - id: \"intro\"\n    name: \"Introduction\"\n    marker: \"## Introduction\"\n    priority: 90\n---\n\n## Introduction\nTest content.\n";

        temp_file
            .write_all(content.as_bytes())
            .map_err(ctx_core::ContextError::Io)?;

        let args = ParseArgs {
            file: temp_file.path().to_path_buf(),
            format: "invalid".to_string(),
        };

        let result = run_parse(args);
        assert!(result.is_err());
        Ok(())
    }
}
