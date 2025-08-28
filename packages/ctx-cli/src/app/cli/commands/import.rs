//! Import 명령어 구현
//!
//! 비표준 마크다운 파일을 표준 contexts 형식으로 변환하여 가져옵니다.

use crate::app::cli::utils::parse_tags;
use ctx_core::{ContextError, LocalFsStorage, Result, Storage};

// Transformer availability shim
// If legacy-compat feature (old transformer) is present, import it.
// Otherwise, provide a minimal no-op transformer to keep Import command working.
#[cfg(feature = "legacy-compat")]
use ctx_core::core::transformer::{StandardTransformer, TransformHint, Transformer};

#[cfg(not(feature = "legacy-compat"))]
mod shim_transformer {
    use ctx_core::Result;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    pub struct TransformHint {
        pub suggested_id: Option<String>,
        pub suggested_type: Option<String>,
        pub suggested_domain: Option<String>,
        pub suggested_tags: Vec<String>,
        pub file_path: Option<String>,
    }

    pub trait Transformer {
        fn can_transform(&self, content: &str) -> bool;
        fn transform(&self, content: &str, hint: Option<TransformHint>) -> Result<String>;
    }

    #[derive(Debug, Clone, Default)]
    pub struct StandardTransformer;
    impl StandardTransformer {
        pub fn new() -> Self {
            Self
        }
    }

    impl Transformer for StandardTransformer {
        fn can_transform(&self, content: &str) -> bool {
            !content.trim_start().starts_with("---\n")
        }
        fn transform(&self, content: &str, hint: Option<TransformHint>) -> Result<String> {
            // Wrap into context.v1 with a single Content section
            let suggested_id = hint
                .as_ref()
                .and_then(|h| h.suggested_id.clone())
                .unwrap_or_else(|| format!("IMPORTED-{}", chrono::Utc::now().timestamp()));
            let title = hint
                .as_ref()
                .and_then(|h| h.file_path.as_ref())
                .and_then(|p| std::path::Path::new(p).file_stem().and_then(|s| s.to_str()))
                .map(|s| s.to_string())
                .unwrap_or_else(|| suggested_id.clone());
            let section = "content";
            let fm = format!(
                "---\nid: \"{}\"\ntitle: \"{}\"\nversion: \"1.0.0\"\nschema: \"context.v1\"\ntype: \"guide\"\nsections:\n  - id: \"{}\"\n    name: \"Content\"\n    marker: \"## Content\"\n    priority: 50\n---\n\n",
                suggested_id, title, section
            );
            let body = format!("## Content\n\n{}\n", content.trim());
            Ok(fm + &body)
        }
    }
}

use colored::*;
#[cfg(not(feature = "legacy-compat"))]
use shim_transformer::{StandardTransformer, TransformHint, Transformer};
use std::path::{Path, PathBuf};

/// Import 명령어 인자
#[derive(Debug, Clone)]
pub struct ImportArgs {
    pub input: PathBuf,
    pub contexts_dir: PathBuf,
    pub interactive: bool,
    pub force: bool,
    pub doc_type: Option<String>,
    pub domain: Option<String>,
    pub tags: Option<String>,
    pub dry_run: bool,
}

/// 파일 처리 결과
#[derive(Debug, Clone)]
pub enum ProcessResult {
    Imported,
    Skipped,
}

/// Import 명령어 실행
///
/// # Arguments
/// * `args` - import 명령어 인자
///
/// # Returns
/// * 성공 시 Ok(()), 실패 시 ContextError
pub fn run_import(args: ImportArgs) -> Result<()> {
    println!(
        "{}",
        format!("📥 Importing from: {}", args.input.display()).cyan()
    );

    let transformer = StandardTransformer::new();
    let storage = LocalFsStorage::new(&args.contexts_dir);

    // 입력이 파일인지 폴더인지 확인
    let files_to_process = collect_files_to_process(&args.input)?;

    if files_to_process.is_empty() {
        println!("{}", "❌ No markdown files found to import.".yellow());
        return Ok(());
    }

    println!(
        "{}",
        format!("📋 Found {} file(s) to process", files_to_process.len()).cyan()
    );

    let mut imported_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;

    for file_path in files_to_process {
        match process_single_file(&file_path, &args, &transformer, &storage) {
            Ok(ProcessResult::Imported) => imported_count += 1,
            Ok(ProcessResult::Skipped) => skipped_count += 1,
            Err(e) => {
                eprintln!("❌ Failed to process {}: {}", file_path.display(), e);
                error_count += 1;
            }
        }
    }

    // 결과 요약
    print_import_summary(imported_count, skipped_count, error_count, args.dry_run);

    Ok(())
}

/// 처리할 파일 목록 수집
fn collect_files_to_process(input: &PathBuf) -> Result<Vec<PathBuf>> {
    if input.is_file() {
        Ok(vec![input.clone()])
    } else if input.is_dir() {
        // 폴더에서 .md 파일들 찾기
        let mut md_files = Vec::new();
        let entries = std::fs::read_dir(input).map_err(ContextError::Io)?;

        for entry in entries {
            let entry = entry.map_err(ContextError::Io)?;
            let path = entry.path();

            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "md" {
                        md_files.push(path);
                    }
                }
            }
        }

        Ok(md_files)
    } else {
        Err(ContextError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Input path not found: {}", input.display()),
        )))
    }
}

/// 단일 파일 처리
fn process_single_file(
    file_path: &PathBuf,
    args: &ImportArgs,
    transformer: &StandardTransformer,
    storage: &LocalFsStorage,
) -> Result<ProcessResult> {
    println!("🔄 Processing: {}", file_path.display());

    // 파일 읽기 및 변환
    let transformed_content = read_and_transform(file_path, args, transformer)?;

    if let Some(content) = transformed_content {
        if args.dry_run {
            println!("  🔍 [DRY RUN] Would transform and save");
            return Ok(ProcessResult::Imported);
        }

        // 출력 경로 결정
        let output_path = determine_output_path(file_path, &args.contexts_dir)?;

        // 충돌 해결
        match resolve_conflict(&output_path, args, storage)? {
            ConflictResolution::Skip => {
                println!("  ⏭️  Skipping due to conflict");
                Ok(ProcessResult::Skipped)
            }
            ConflictResolution::Proceed => {
                // 파일 저장
                write_document(&output_path, &content, storage)?;
                println!("  ✅ Imported to: {}", output_path.display());
                Ok(ProcessResult::Imported)
            }
            ConflictResolution::Overwrite => {
                // 파일 덮어쓰기
                write_document(&output_path, &content, storage)?;
                println!("  ✅ Overwritten: {}", output_path.display());
                Ok(ProcessResult::Imported)
            }
            ConflictResolution::Rename(renamed) => {
                write_document(&renamed, &content, storage)?;
                println!("  ✅ Imported (renamed) to: {}", renamed.display());
                Ok(ProcessResult::Imported)
            }
            ConflictResolution::Abort => Err(ctx_core::ContextError::Other(
                "Import aborted by user".to_string(),
            )),
        }
    } else {
        println!("  ⏭️  Already in standard format, skipping");
        Ok(ProcessResult::Skipped)
    }
}

/// 파일 읽기 및 변환
fn read_and_transform(
    file_path: &PathBuf,
    args: &ImportArgs,
    transformer: &StandardTransformer,
) -> Result<Option<String>> {
    let content = std::fs::read_to_string(file_path)?;

    // 변환 필요 여부 확인
    if !transformer.can_transform(&content) {
        return Ok(None);
    }

    // 변환 힌트 생성 및 변환 수행
    let hint = create_transform_hint(file_path, args);
    let transformed_content = transformer.transform(&content, Some(hint))?;

    Ok(Some(transformed_content))
}

/// 충돌 해결 결과
#[derive(Debug)]
enum ConflictResolution {
    Skip,
    Proceed,
    Overwrite,
    Abort,
    Rename(PathBuf),
}

/// 파일 충돌 해결
fn resolve_conflict(
    output_path: &Path,
    args: &ImportArgs,
    storage: &LocalFsStorage,
) -> Result<ConflictResolution> {
    if !storage.exists(output_path) || args.force {
        return Ok(ConflictResolution::Proceed);
    }

    if args.interactive {
        println!("  ⚠️  File already exists: {}", output_path.display());
        println!("  [s]kip, [o]verwrite, [r]ename, [a]bort?");

        loop {
            use std::io::{self, Write};
            print!("  > ");
            io::stdout().flush().map_err(ctx_core::ContextError::Io)?;

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(ctx_core::ContextError::Io)?;

            match input.trim().to_lowercase().as_str() {
                "s" | "skip" => {
                    println!("  ⏭️  Skipping file");
                    return Ok(ConflictResolution::Skip);
                }
                "o" | "overwrite" => {
                    println!("  ✏️  Overwriting file");
                    return Ok(ConflictResolution::Overwrite);
                }
                "r" | "rename" => {
                    println!("  ✏️  Enter new filename (without path):");
                    print!("  > ");
                    io::stdout().flush().map_err(ctx_core::ContextError::Io)?;
                    let mut name = String::new();
                    io::stdin()
                        .read_line(&mut name)
                        .map_err(ctx_core::ContextError::Io)?;
                    let name = name.trim();
                    if name.is_empty() {
                        println!("  ❌ Invalid filename");
                        continue;
                    }
                    let renamed = output_path
                        .parent()
                        .unwrap_or_else(|| Path::new(""))
                        .join(name);
                    if storage.exists(&renamed) {
                        println!("  ❌ File already exists: {}", renamed.display());
                        continue;
                    }
                    return Ok(ConflictResolution::Rename(renamed));
                }
                "a" | "abort" => {
                    println!("  🛑 Aborting import");
                    return Ok(ConflictResolution::Abort);
                }
                _ => {
                    println!(
                        "  ❌ Invalid choice. Please enter 's' (skip), 'o' (overwrite), or 'a' (abort)"
                    );
                }
            }
        }
    } else {
        println!("  ⏭️  File exists, use --force to overwrite");
        Ok(ConflictResolution::Skip)
    }
}

/// 문서 저장
fn write_document(output_path: &Path, content: &str, storage: &LocalFsStorage) -> Result<()> {
    storage.write(output_path, content)
}

/// 변환 힌트 생성
fn create_transform_hint(file_path: &Path, args: &ImportArgs) -> TransformHint {
    let suggested_id = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_uppercase().replace(['-', '_', ' '], "-"));

    TransformHint {
        suggested_id,
        suggested_type: args.doc_type.clone(),
        suggested_domain: args.domain.clone(),
        suggested_tags: parse_tags(args.tags.clone()).unwrap_or_default(),
        file_path: Some(file_path.to_string_lossy().to_string()),
    }
}

/// 출력 경로 결정 - 고유한 경로 생성으로 충돌 방지
fn determine_output_path(file_path: &Path, _contexts_dir: &Path) -> Result<PathBuf> {
    // 기본적으로 원래 파일명 유지
    let file_name = file_path
        .file_name()
        .ok_or_else(|| ContextError::AssemblyError("Invalid file name".to_string()))?;
    Ok(PathBuf::from(file_name))
}

/// 가져오기 결과 요약 출력
fn print_import_summary(imported: usize, skipped: usize, errors: usize, dry_run: bool) {
    println!();
    println!("{}", "📊 Import Summary:".bold().blue());
    println!("  Imported: {}", imported.to_string().green());
    println!("  Skipped: {}", skipped.to_string().yellow());
    println!("  Errors: {}", errors.to_string().red());

    if dry_run {
        println!();
        println!(
            "{}",
            "💡 This was a dry run. Use without --dry-run to actually import files.".yellow()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_collect_files_single_file() -> Result<()> {
        let temp_dir = TempDir::new().map_err(ContextError::Io)?;
        let test_file = temp_dir.path().join("test.md");
        fs::write(&test_file, "# Test").map_err(ContextError::Io)?;

        let files = collect_files_to_process(&test_file)?;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], test_file);
        Ok(())
    }

    #[test]
    fn test_collect_files_directory() -> Result<()> {
        let temp_dir = TempDir::new().map_err(ContextError::Io)?;
        fs::write(temp_dir.path().join("test1.md"), "# Test 1").map_err(ContextError::Io)?;
        fs::write(temp_dir.path().join("test2.md"), "# Test 2").map_err(ContextError::Io)?;
        fs::write(temp_dir.path().join("test.txt"), "Not markdown").map_err(ContextError::Io)?;

        let files = collect_files_to_process(&temp_dir.path().to_path_buf())?;
        assert_eq!(files.len(), 2);
        Ok(())
    }

    #[test]
    fn test_run_import_dry_run() -> Result<()> {
        let temp_dir = TempDir::new().map_err(ContextError::Io)?;
        let input_file = temp_dir.path().join("input.md");
        fs::write(&input_file, "# Test Document\n\nThis is a test.").map_err(ContextError::Io)?;

        let contexts_dir = temp_dir.path().join("contexts");
        fs::create_dir_all(&contexts_dir).map_err(ContextError::Io)?;

        let args = ImportArgs {
            input: input_file,
            contexts_dir,
            interactive: false,
            force: false,
            doc_type: Some("guide".to_string()),
            domain: None,
            tags: None,
            dry_run: true,
        };

        let result = run_import(args);
        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_run_import_actual_write() -> Result<()> {
        let temp_dir = TempDir::new().map_err(ContextError::Io)?;
        let input_file = temp_dir.path().join("input.md");
        fs::write(&input_file, "# Test Document\n\nThis is a test.").map_err(ContextError::Io)?;

        let contexts_dir = temp_dir.path().join("contexts");
        fs::create_dir_all(&contexts_dir).map_err(ContextError::Io)?;

        let args = ImportArgs {
            input: input_file,
            contexts_dir: contexts_dir.clone(),
            interactive: false,
            force: false,
            doc_type: Some("guide".to_string()),
            domain: None,
            tags: None,
            dry_run: false,
        };

        let result = run_import(args);
        assert!(result.is_ok());

        // 파일이 실제로 생성되었는지 확인
        let files: Vec<_> = fs::read_dir(&contexts_dir)
            .map_err(ContextError::Io)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(ContextError::Io)?;
        assert_eq!(files.len(), 1);

        Ok(())
    }

    #[test]
    fn test_determine_output_path_keeps_name() -> Result<()> {
        let path1 = Path::new("/dir1/test.md");
        let path2 = Path::new("/dir2/test.md");
        let contexts_dir = Path::new("/contexts");

        let output1 = determine_output_path(path1, contexts_dir)?;
        let output2 = determine_output_path(path2, contexts_dir)?;

        // 같은 파일명은 동일한 출력 파일명을 유지
        assert_eq!(output1, output2);

        Ok(())
    }
}
