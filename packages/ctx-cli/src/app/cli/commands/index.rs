use crate::app::config::AppConfig;
use ctx_core::common::utils::extract_title_and_body;
use ctx_core::core::classifier::service::ClassifierService;
use ctx_core::data::index::pooled_sqlite::{DocumentIndex, PooledSqliteIndexManager};

use chrono::Utc;
use clap::Args;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Args, Clone)]
pub struct IndexArgs {
    /// Directory to scan (recursively)
    #[arg(long, value_name = "DIR", default_value = "./contexts")]
    pub path: PathBuf,

    /// Database path (SQLite)
    #[arg(long, value_name = "FILE", default_value = "ctxindex.db")]
    pub db: PathBuf,

    /// Ontology file path
    #[arg(long, value_name = "FILE", default_value = "ontology.yaml")]
    pub ontology: PathBuf,

    /// Rules file path (optional)
    #[arg(long, value_name = "FILE")]
    pub rules: Option<PathBuf>,
}

pub fn run_index(args: IndexArgs) -> ctx_core::Result<()> {
    println!("🔎 Scanning directory: {}", args.path.display());

    if !args.path.exists() {
        return Err(ctx_core::ContextError::Other(format!(
            "Path not found: {}",
            args.path.display()
        )));
    }

    // Config defaults
    let cfg = AppConfig::load_from_current_dir();
    let ontology_path = cfg
        .as_ref()
        .and_then(|c| c.ontology.clone())
        .unwrap_or_else(|| args.ontology.clone());
    let rules_path = args
        .rules
        .clone()
        .or_else(|| cfg.as_ref().and_then(|c| c.rules.clone()));
    let db_path_buf = cfg
        .as_ref()
        .and_then(|c| c.db.clone())
        .unwrap_or_else(|| args.db.clone());

    // Preserve original IO error context instead of wrapping as Other
    let ontology_yaml = fs::read_to_string(&ontology_path).map_err(ctx_core::ContextError::Io)?;
    let rules_yaml = match &rules_path {
        Some(p) if p.exists() => Some(fs::read_to_string(p).map_err(ctx_core::ContextError::Io)?),
        _ => None,
    };

    let db_path = db_path_buf.to_string_lossy().to_string();
    let index = PooledSqliteIndexManager::new_default(&db_path)?;

    let files = collect_markdown_files(&args.path)?;
    if files.is_empty() {
        println!("ℹ️ No markdown files found.");
        return Ok(());
    }
    println!("📋 Found {} markdown file(s)", files.len());

    let mut docs = Vec::with_capacity(files.len());
    for (i, file) in files.iter().enumerate() {
        if i % 50 == 0 {
            println!("  → {} / {}", i + 1, files.len());
        }
        match build_document_index(file, &ontology_yaml, rules_yaml.as_deref()) {
            Ok(doc) => docs.push(doc),
            Err(e) => eprintln!("  ❌ {}: {}", file.display(), e),
        }
    }

    if docs.is_empty() {
        println!("⚠️ Nothing to index.");
        return Ok(());
    }

    index.upsert_documents_batch(&docs)?;
    println!("✅ Indexed {} document(s) into {}", docs.len(), db_path);
    Ok(())
}

fn collect_markdown_files(root: &Path) -> ctx_core::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> ctx_core::Result<()> {
        for entry in fs::read_dir(dir).map_err(ctx_core::ContextError::Io)? {
            let entry = entry.map_err(ctx_core::ContextError::Io)?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out)?;
            } else if path.is_file()
                && path
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("md"))
                    .unwrap_or(false)
            {
                out.push(path);
            }
        }
        Ok(())
    }
    if root.is_file() {
        out.push(root.to_path_buf());
    } else if root.is_dir() {
        walk(root, &mut out)?;
    }
    Ok(out)
}

fn build_document_index(
    path: &Path,
    ontology_yaml: &str,
    rules_yaml: Option<&str>,
) -> ctx_core::Result<DocumentIndex> {
    let content = fs::read_to_string(path).map_err(ctx_core::ContextError::Io)?;

    // Extract title/body for classification
    let (title_opt, body) = extract_title_and_body(&content);
    let title = title_opt.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    });

    // Classify
    let classification =
        ClassifierService::classify_with_yaml(ontology_yaml, rules_yaml, &title, &body)?;

    // Hashes
    let sha = hex_sha256(&content);
    let content_hash = sha.clone();

    // Build facets map
    let facets = classification.facets().to_vec_map();

    Ok(DocumentIndex {
        doc_id: path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("DOC")
            .to_string(),
        repo: "local".to_string(),
        branch: "main".to_string(),
        path: path.to_string_lossy().to_string(),
        sha,
        content_hash,
        title,
        locale: "en".to_string(),
        trust: 0.8,
        freshness: Utc::now().date_naive().to_string(),
        source_type: "markdown".to_string(),
        maturity: "stable".to_string(),
        facets,
        confidence: classification.confidence(),
        warnings: classification.warnings().to_vec(),
        errors: classification.errors().to_vec(),
        ontology_version: ctx_core::common::constants::versioning::CONSTANTS_VERSION.to_string(),
        rules_version: ctx_core::common::constants::versioning::RULES_VERSION.to_string(),
    })
}

fn hex_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_collect_markdown_files() -> ctx_core::Result<()> {
        let dir = TempDir::new().map_err(ctx_core::ContextError::Io)?;
        let f1 = dir.path().join("a.md");
        let f2 = dir.path().join("b.txt");
        fs::write(&f1, "# A").map_err(ctx_core::ContextError::Io)?;
        fs::write(&f2, "B").map_err(ctx_core::ContextError::Io)?;
        let files = collect_markdown_files(dir.path())?;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name().unwrap(), "a.md");
        Ok(())
    }

    #[test]
    fn test_hex_sha256() {
        let h = hex_sha256("abc");
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
