//! CLI 명령어 모듈들
//!
//! 각 CLI 명령어를 독립적인 모듈로 분리하여 관리합니다.

pub mod classify;
pub mod github;
pub mod import;
pub mod index;
pub mod parse;
#[cfg(feature = "http")]
pub mod server;
pub mod validate;

// 명령어 실행 함수들을 re-export
pub use classify::{ClassifyArgs, run_classify};
pub use import::{ImportArgs, run_import};
pub use index::{IndexArgs, run_index};
pub use parse::{ParseArgs, run_parse};
#[cfg(feature = "http")]
pub use server::{ServerArgs, run_server};
pub use validate::{ValidateArgs, run_validate};

// Additional CLI functions needed by app/mod.rs
use crate::domain::errors::Result;
use std::path::PathBuf;

#[derive(Debug)]
pub struct GraphArgs {
    pub path: PathBuf,
    pub format: String,
}

pub fn run_graph(args: GraphArgs) -> Result<()> {
    
    use tokio::runtime::Runtime;
    
    
    
    println!("🔍 Analyzing dependency graph in: {:?}", args.path);
    println!("📋 Output format: {}", args.format);
    
    // Create async runtime for database operations
    let rt = Runtime::new()
        .map_err(|e| crate::domain::errors::ContextError::Other(format!("Failed to create async runtime: {}", e)))?;
    
    rt.block_on(async {
        run_graph_async(args).await
    })
}

async fn run_graph_async(args: GraphArgs) -> Result<()> {
    use crate::drivers::storage::sqlite::SqliteIndexManager;
    use crate::domain::errors::ContextError;
    use std::collections::{HashMap, HashSet};
    
    // Connect to database
    let db_url = "sqlite://ctxindex.db";
    let index_manager = SqliteIndexManager::new(db_url).await
        .map_err(|e| ContextError::Other(format!("Failed to connect to database: {}", e)))?;
    
    // Get all documents
    let documents = index_manager.list_documents(None, None).await?;
    
    if documents.is_empty() {
        println!("ℹ️  No documents found in index. Run 'ctx index' first.");
        return Ok(());
    }
    
    println!("📊 Analyzing {} documents...", documents.len());
    
    // Analyze dependencies between documents
    let mut dependencies: HashMap<String, HashSet<String>> = HashMap::new();
    
    // Simple dependency analysis based on document references
    // This is a basic implementation - could be enhanced with more sophisticated analysis
    for doc in &documents {
        let mut deps = HashSet::new();
        
        // Look for references to other documents in the same repository
        if let Some(doc_content) = index_manager.get_document(&doc.id).await? {
            // Find markdown links and references
            let content = doc_content.content;
            for other_doc in &documents {
                if other_doc.id != doc.id {
                    let other_path = std::path::Path::new(&other_doc.path);
                    if let Some(other_name) = other_path.file_stem().and_then(|s| s.to_str()) {
                        // Check for references to this document
                        if content.contains(other_name) || content.contains(&other_doc.path) {
                            deps.insert(other_doc.id.clone());
                        }
                    }
                }
            }
        }
        
        dependencies.insert(doc.id.clone(), deps);
    }
    
    // Generate output based on requested format
    match args.format.as_str() {
        "dot" => {
            println!("\ndigraph dependencies {{");
            println!("  node [shape=box, style=rounded];");
            
            // Add nodes
            for doc in &documents {
                let label = doc.title.replace("\"", "\\\"")
                    .chars().take(30).collect::<String>();
                println!("  \"{}\" [label=\"{}\"];", doc.id, label);
            }
            
            // Add edges
            for (from_id, deps) in &dependencies {
                for to_id in deps {
                    println!("  \"{}\" -> \"{}\";", from_id, to_id);
                }
            }
            
            println!("}}");
        }
        "json" => {
            let mut nodes = Vec::new();
            let mut edges = Vec::new();
            
            for doc in &documents {
                nodes.push(serde_json::json!({
                    "id": doc.id,
                    "title": doc.title,
                    "path": doc.path,
                    "facets": doc.facets
                }));
            }
            
            for (from_id, deps) in &dependencies {
                for to_id in deps {
                    edges.push(serde_json::json!({
                        "from": from_id,
                        "to": to_id
                    }));
                }
            }
            
            let graph = serde_json::json!({
                "nodes": nodes,
                "edges": edges,
                "metadata": {
                    "total_nodes": documents.len(),
                    "total_edges": edges.len(),
                    "generated_at": chrono::Utc::now().to_rfc3339()
                }
            });
            
            println!("{}", serde_json::to_string_pretty(&graph)
                .map_err(|e| ContextError::Other(format!("JSON serialization failed: {}", e)))?);
        }
        "summary" | _ => {
            println!("\n📊 Dependency Graph Summary:");
            println!("{}", "=".repeat(50));
            println!("Total documents: {}", documents.len());
            
            let total_deps: usize = dependencies.values().map(|deps| deps.len()).sum();
            println!("Total dependencies: {}", total_deps);
            
            if total_deps > 0 {
                println!("Average dependencies per document: {:.1}", 
                    total_deps as f64 / documents.len() as f64);
                
                // Find most connected documents
                let mut sorted_docs: Vec<_> = dependencies.iter()
                    .map(|(id, deps)| (id, deps.len()))
                    .collect();
                sorted_docs.sort_by(|a, b| b.1.cmp(&a.1));
                
                println!("\nMost connected documents:");
                for (id, dep_count) in sorted_docs.iter().take(5) {
                    if let Some(doc) = documents.iter().find(|d| &d.id == *id) {
                        println!("  • {}: {} dependencies", doc.title, dep_count);
                    }
                }
            } else {
                println!("\nℹ️  No dependencies detected between documents.");
            }
        }
    }
    
    println!("\n✅ Graph analysis completed!");
    Ok(())
}

#[derive(Debug)]
pub struct SearchArgs {
    pub query: String,
    pub tags: Option<Vec<String>>,
    pub limit: usize,
}

pub fn run_search(args: SearchArgs) -> Result<()> {
    
    use tokio::runtime::Runtime;
    
    
    // Create async runtime for database operations
    let rt = Runtime::new()
        .map_err(|e| crate::domain::errors::ContextError::Other(format!("Failed to create async runtime: {}", e)))?;
    
    rt.block_on(async {
        run_search_async(args).await
    })
}

async fn run_search_async(args: SearchArgs) -> Result<()> {
    use crate::drivers::storage::sqlite::SqliteIndexManager;
    use crate::domain::errors::ContextError;
    
    println!("🔍 Searching for: '{}'", args.query);
    if let Some(tags) = &args.tags {
        println!("🏷️  Filtered by tags: {:?}", tags);
    }
    println!("🔢 Limit: {}", args.limit);
    
    // Connect to database (assume default location)
    let db_url = "sqlite://ctxindex.db";
    let index_manager = SqliteIndexManager::new(db_url).await
        .map_err(|e| ContextError::Other(format!("Failed to connect to database: {}", e)))?;
    
    // Perform search
    let results = index_manager.search_documents(&args.query, args.limit).await?;
    
    if results.is_empty() {
        println!("ℹ️  No documents found matching your query");
        return Ok(());
    }
    
    println!("\n📊 Found {} results:", results.len());
    println!("{}", "=".repeat(80));
    
    for (i, result) in results.iter().enumerate() {
        println!("\n{}. {} (score: {:.2}, confidence: {:.2})", 
            i + 1, result.title, result.trust_score, result.confidence);
        println!("   📄 Path: {}", result.path);
        
        // Show facets if available
        if !result.facets.is_empty() {
            print!("   🏷️  Tags: ");
            let mut facet_strs = Vec::new();
            for (ns, values) in &result.facets {
                for value in values {
                    facet_strs.push(format!("{}:{}", ns, value));
                }
            }
            println!("{}", facet_strs.join(", "));
        }
        
        // Show snippet if available
        if !result.snippet.is_empty() {
            println!("   📝 Snippet: {}", result.snippet);
        }
    }
    
    println!("\n{}", "=".repeat(80));
    println!("✅ Search completed in {} results", results.len());
    
    Ok(())
}

// IndexRepo는 제거됨 - 대신 server 명령어 사용
