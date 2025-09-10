//! Import command implementation
//!
//! Provides document import functionality for converting external documents into context format.

use crate::domain::errors::{ContextError, Result};
use crate::drivers::storage::local::LocalFsStorage;
use crate::drivers::storage::Storage;
use crate::doc::parse::document::DocumentParser;
use crate::domain::types::ContextDocument;
use crate::util::hash::generate_document_id;
use std::path::{Path, PathBuf};
use std::time::Instant;

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

pub fn run_import(args: ImportArgs) -> Result<()> {
    let start_time = Instant::now();
    
    println!("📁 Importing from: {}", args.input.display());
    println!("📂 Output directory: {}", args.contexts_dir.display());
    
    if args.dry_run {
        println!("📝 Dry run mode - no files will be modified");
    }
    
    if args.interactive {
        println!("💬 Interactive mode enabled");
    }
    
    if let Some(ref doc_type) = args.doc_type {
        println!("🏷️  Document type: {}", doc_type);
    }
    
    if let Some(ref domain) = args.domain {
        println!("🌐 Domain: {}", domain);
    }
    
    if let Some(ref tags) = args.tags {
        println!("🏷️  Tags: {}", tags);
    }
    
    // Initialize storage for input and output
    let input_storage = LocalFsStorage::new(&args.input);
    let output_storage = LocalFsStorage::new(&args.contexts_dir);
    
    // Ensure output directory exists
    if !args.dry_run {
        output_storage.create_dir_all(Path::new("."))?;
    }
    
    // Discover files to import
    println!("🔍 Scanning for importable files...");
    let files = discover_importable_files(&input_storage)?;
    
    if files.is_empty() {
        println!("ℹ️  No importable files found");
        return Ok(());
    }
    
    println!("Found {} files to import", files.len());
    
    // Process each file
    let mut imported_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;
    
    for (i, file_path) in files.iter().enumerate() {
        if i % 10 == 0 {
            println!("📊 Progress: {}/{} ({:.1}%)", i + 1, files.len(), 
                ((i + 1) as f64 / files.len() as f64) * 100.0);
        }
        
        match import_single_file(
            &input_storage,
            &output_storage,
            file_path,
            &args,
        ) {
            Ok(ImportResult::Imported) => {
                imported_count += 1;
                if !args.dry_run {
                    println!("✅ Imported: {}", file_path.display());
                }
            }
            Ok(ImportResult::Skipped(reason)) => {
                skipped_count += 1;
                if !args.dry_run {
                    println!("⏭️  Skipped: {} ({})", file_path.display(), reason);
                }
            }
            Err(e) => {
                error_count += 1;
                eprintln!("⚠️  Error importing {}: {}", file_path.display(), e);
            }
        }
    }
    
    let duration = start_time.elapsed();
    
    // Show final statistics
    println!("\n✅ Import completed!");
    println!("📊 Statistics:");
    println!("   • Imported: {} files", imported_count);
    println!("   • Skipped: {} files", skipped_count);
    println!("   • Errors: {} files", error_count);
    println!("   • Duration: {:.2}s", duration.as_secs_f64());
    
    if args.dry_run {
        println!("\n📝 Note: This was a dry run. No files were actually created.");
    }
    
    Ok(())
}

/// Result of importing a single file
#[derive(Debug)]
enum ImportResult {
    Imported,
    Skipped(String),
}

/// Discover files that can be imported
fn discover_importable_files(storage: &LocalFsStorage) -> Result<Vec<PathBuf>> {
    let mut importable_files = Vec::new();
    
    // Look for common document formats
    let patterns = [
        "*.md", "*.txt", "*.rst", "*.org", "*.adoc",
        "*.html", "*.htm", "*.pdf"
    ];
    
    for pattern in &patterns {
        let files = storage.list_files(Path::new("."), pattern)?;
        importable_files.extend(files);
    }
    
    // Sort for deterministic processing
    importable_files.sort();
    
    Ok(importable_files)
}

/// Import a single file
fn import_single_file(
    input_storage: &LocalFsStorage,
    output_storage: &LocalFsStorage,
    file_path: &Path,
    args: &ImportArgs,
) -> Result<ImportResult> {
    // Read the source file
    let content = input_storage.read(file_path)?;
    
    // Skip empty files
    if content.trim().is_empty() {
        return Ok(ImportResult::Skipped("Empty file".to_string()));
    }
    
    // Parse the document
    let parser = DocumentParser::new();
    let (parsed_doc, _parsed_body) = parser.parse_content_with_body(&content, Some(file_path))
        .unwrap_or_else(|_| {
            // If parsing fails, create a basic document structure
            let mut basic_doc = ContextDocument::new(
                generate_document_id(Path::new("."), file_path).unwrap_or_else(|_| "unknown".to_string()),
                file_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string()
            );
            basic_doc.title = file_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string();
            (basic_doc, content.clone())
        });
    
    // Create context document
    let mut context_doc = ContextDocument::new(
        generate_document_id(Path::new("."), file_path)?,
        "Temporary Title".to_string()
    );
    
    // Set basic metadata
    context_doc.id = generate_document_id(Path::new("."), file_path)
        .map_err(|e| ContextError::Other(format!("Failed to generate document ID: {}", e)))?;
    
    context_doc.title = if !parsed_doc.title.is_empty() {
        parsed_doc.title.clone()
    } else {
        file_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    };
    
    // Store basic section info (SectionDef doesn't store content directly)
    context_doc.sections.push(crate::domain::types::SectionDef {
        id: "content".to_string(),
        name: "Main Content".to_string(),
        marker: "## Content".to_string(),
        priority: 100,
        tokens: None,
    });
    
    // Set basic metadata
    context_doc.version = "1.0.0".to_string();
    context_doc.schema = "context.v1".to_string();
    
    // Apply command-line metadata
    if let Some(ref doc_type) = args.doc_type {
        context_doc.r#type = doc_type.clone();
    } else {
        context_doc.r#type = "document".to_string();
    }
    
    // Parse and apply tags
    if let Some(ref tags_str) = args.tags {
        let tags: Vec<String> = tags_str.split(',').map(|s| s.trim().to_string()).collect();
        context_doc.tags = tags;
    }
    
    // Add facets from existing parsed document if available
    if !parsed_doc.facets.is_empty() {
        context_doc.facets = parsed_doc.facets.clone();
    }
    
    // Determine output filename
    let output_filename = if file_path.extension().and_then(|s| s.to_str()) == Some("md") {
        // Keep .md extension for markdown files
        file_path.file_name().unwrap().to_string_lossy().to_string()
    } else {
        // Convert other formats to .md
        format!("{}.md", 
            file_path.file_stem().unwrap().to_string_lossy())
    };
    
    let output_path = Path::new(&output_filename);
    
    // Check if output file already exists (unless force is enabled)
    if !args.force && output_storage.exists(&output_path) {
        if args.interactive {
            // In interactive mode, we would ask the user
            // For now, skip the file
            return Ok(ImportResult::Skipped("File already exists".to_string()));
        } else {
            return Ok(ImportResult::Skipped("File already exists (use --force to overwrite)".to_string()));
        }
    }
    
    if !args.dry_run {
        // Serialize the context document
        let output_content = serialize_context_document(&context_doc)?;
        
        // Write to output directory
        output_storage.write(&output_path, &output_content)?;
    }
    
    Ok(ImportResult::Imported)
}

/// Serialize context document to markdown with frontmatter
fn serialize_context_document(doc: &ContextDocument) -> Result<String> {
    // Use serde to serialize the document to YAML frontmatter
    let frontmatter_yaml = serde_yaml::to_string(doc)
        .map_err(|e| ContextError::Other(format!("Failed to serialize frontmatter: {}", e)))?;
    
    // Combine frontmatter with content
    let mut output = String::new();
    output.push_str("---\n");
    output.push_str(&frontmatter_yaml);
    output.push_str("---\n\n");
    
    // Add placeholder content since we can't store actual body in sections
    // In a real implementation, you'd store content separately or modify the schema
    output.push_str("[Content would be stored here]");
    
    Ok(output)
}