//! Index command implementation
//!
//! Provides document indexing functionality using SQLite FTS5 and classification.

use crate::domain::errors::{ContextError, Result};
use crate::drivers::storage::sqlite::SqliteIndexManager;
use crate::drivers::storage::local::LocalFsStorage;
use crate::drivers::storage::Storage;
use crate::pipeline::classifier::service::ClassifierService;
use crate::knowledge::ontology::OntologyRegistry;
use crate::knowledge::rules::builtin_rules;
use crate::doc::parse::document::DocumentParser;
use crate::util::hash::generate_document_id;
use std::path::{Path, PathBuf};
use tokio::runtime::Runtime;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct IndexArgs {
    pub path: PathBuf,
    pub db: PathBuf,
    pub ontology: PathBuf,
    pub rules: Option<PathBuf>,
    pub full: bool,
}

pub fn run_index(args: IndexArgs) -> Result<()> {
    println!("🔎 Indexing directory: {}", args.path.display());
    println!("Database: {}", args.db.display());
    println!("Ontology: {}", args.ontology.display());
    
    if let Some(ref rules) = args.rules {
        println!("Rules: {}", rules.display());
    }
    
    if args.full {
        println!("Full reindex enabled");
    }
    
    // Create async runtime for database operations
    let rt = Runtime::new()
        .map_err(|e| ContextError::Other(format!("Failed to create async runtime: {}", e)))?;
    
    rt.block_on(async {
        run_index_async(args).await
    })
}

async fn run_index_async(args: IndexArgs) -> Result<()> {
    let start_time = Instant::now();
    
    // Initialize components
    let storage = LocalFsStorage::new(&args.path);
    
    // Connect to SQLite database
    let db_url = format!("sqlite://{}", args.db.display());
    let index_manager = SqliteIndexManager::new(&db_url).await?;
    
    // Load ontology
    let ontology_path = args.ontology.strip_prefix(&args.path).unwrap_or(&args.ontology);
    let ontology_content = storage.read(ontology_path)?;
    let ontology = OntologyRegistry::from_yaml(&ontology_content)
        .map_err(|e| ContextError::Other(format!("Failed to load ontology: {}", e)))?;
    
    let rules_content = if let Some(ref rules_path) = args.rules {
        let rules_path = rules_path.strip_prefix(&args.path).unwrap_or(rules_path);
        Some(storage.read(rules_path)?)
    } else {
        None
    };
    
    // Initialize classifier
    let mut classifier = ClassifierService::new();
    classifier.set_ontology(ontology);
    if rules_content.is_some() {
        let rules = builtin_rules(); // Use builtin rules for now
        classifier.set_rules(rules);
    }
    
    // Clear index if full reindex requested
    if args.full {
        println!("🗑️  Clearing existing index...");
        index_manager.clear_index().await?;
    }
    
    // Get list of files to index
    println!("📄 Scanning for documents...");
    let files = discover_documents(&storage, &args.path, &index_manager, !args.full).await?;
    println!("Found {} documents to process", files.len());
    
    if files.is_empty() {
        println!("✅ No documents need indexing");
        return Ok(());
    }
    
    // Process documents
    let mut indexed_count = 0;
    let mut error_count = 0;
    
    for (i, file_path) in files.iter().enumerate() {
        if i % 10 == 0 {
            println!("📊 Progress: {}/{} ({:.1}%)", i + 1, files.len(), 
                ((i + 1) as f64 / files.len() as f64) * 100.0);
        }
        
        match process_document(&storage, &index_manager, file_path, &args.path, &ontology_content, &rules_content).await {
            Ok(()) => {
                indexed_count += 1;
            }
            Err(e) => {
                eprintln!("⚠️  Error processing {}: {}", file_path.display(), e);
                error_count += 1;
            }
        }
    }
    
    // Update metadata
    let now = chrono::Utc::now().to_rfc3339();
    index_manager.update_metadata("last_index_update", &now).await?;
    
    // Show final statistics
    let stats = index_manager.get_index_stats().await?;
    let duration = start_time.elapsed();
    
    println!("\n✅ Indexing completed!");
    println!("📊 Statistics:");
    println!("   • Processed: {} documents", indexed_count);
    println!("   • Errors: {} documents", error_count);
    println!("   • Total in index: {} documents", stats.total_documents);
    println!("   • Total tokens: {} tokens", stats.total_tokens);
    println!("   • Duration: {:.2}s", duration.as_secs_f64());
    println!("   • Rate: {:.1} docs/sec", indexed_count as f64 / duration.as_secs_f64());
    
    Ok(())
}

/// Discover documents that need indexing
async fn discover_documents(
    storage: &LocalFsStorage,
    base_path: &Path,
    index_manager: &SqliteIndexManager,
    incremental: bool,
) -> Result<Vec<PathBuf>> {
    let mut files_to_index = Vec::new();
    
    // Get all markdown and text files
    let patterns = ["*.md", "*.txt", "*.rst", "*.org"];
    for pattern in &patterns {
        let files = storage.list_files(Path::new("."), pattern)?;
        for file in files {
            let should_index = if incremental {
                // Check if file needs reindexing based on content hash
                needs_reindexing(&file, storage, index_manager, base_path).await?
            } else {
                true
            };
            
            if should_index {
                files_to_index.push(file);
            }
        }
    }
    
    Ok(files_to_index)
}

/// Check if a document needs reindexing based on content hash
async fn needs_reindexing(
    file_path: &Path,
    storage: &LocalFsStorage,
    index_manager: &SqliteIndexManager,
    base_path: &Path,
) -> Result<bool> {
    // Read current content
    let content = storage.read(file_path)?;
    
    // Calculate current hash
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let current_hash = format!("{:x}", hasher.finalize());
    
    // Generate document ID
    let id = generate_document_id(base_path, file_path)
        .map_err(|e| ContextError::Other(format!("Failed to generate document ID: {}", e)))?;
    
    // Check stored hash
    match index_manager.get_document_hash(&id).await? {
        Some(stored_hash) if stored_hash == current_hash => {
            // Content unchanged, no need to reindex
            Ok(false)
        }
        _ => {
            // New file or content changed
            Ok(true)
        }
    }
}

/// Process a single document for indexing
async fn process_document(
    storage: &LocalFsStorage,
    index_manager: &SqliteIndexManager,
    file_path: &Path,
    base_path: &Path,
    ontology_content: &str,
    rules_content: &Option<String>,
) -> Result<()> {
    // Read document content
    let content = storage.read(file_path)?;
    
    // Parse document structure
    let parser = DocumentParser::new();
    let (document, body) = parser.parse_content_with_body(&content, Some(file_path))
        .map_err(|e| ContextError::Other(format!("Failed to parse document: {}", e)))?;
    
    // Extract title (use document title or filename)
    let title = if !document.title.is_empty() {
        document.title.clone()
    } else {
        file_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    };
    
    // Generate unique document ID
    let id = generate_document_id(base_path, file_path)
        .map_err(|e| ContextError::Other(format!("Failed to generate document ID: {}", e)))?;
    
    // Classify document using available APIs
    let classification_result = ClassifierService::classify_with_yaml(
        ontology_content,
        rules_content.as_deref(),
        &title,
        &body
    )
    .map_err(|e| ContextError::Other(format!("Classification failed: {}", e)))?;
    
    // Convert classification to facets HashMap
    let facet_map = classification_result.facets();
    let facets = facet_map.to_hashmap();
    
    // Index the document
    index_manager.index_document(
        &id,
        &title,
        file_path,
        &body,
        &facets,
        classification_result.confidence().into(),
        classification_result.confidence().into(),
    ).await?;
    
    Ok(())
}