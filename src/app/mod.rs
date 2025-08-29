//! Single Binary Application Entry Point (PLAN.md Architecture)
//!
//! Mode switcher: CLI/HTTP/MCP based on command line arguments
//! Preserves all existing functionality from the original implementation

use crate::domain::{errors::Result, types::*};
use crate::drivers::{http, mcp};
#[cfg(any(feature = "ui_ipc", feature = "ui_render_rust", feature = "ui_tauri"))]
pub mod ui;
use crate::services;
use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ctx")]
#[command(about = "AI Context Management Library - Single Binary with Multiple Modes")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// PLAN.md Core Commands - New simplified API

    /// Compose context prompt from query
    Compose {
        /// Query text
        query: String,

        /// Maximum tokens
        #[arg(long)]
        max_tokens: Option<u32>,

        /// Filter by tags
        #[arg(long)]
        tags: Option<Vec<String>>,

        /// Output format (json|text)
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Complete AI work pipeline
    Work {
        /// Task description
        task: String,

        /// Additional context
        #[arg(long)]
        context: Option<String>,

        /// AI provider
        #[arg(long)]
        provider: Option<String>,

        /// Temperature
        #[arg(long)]
        temperature: Option<f32>,

        /// Maximum tokens
        #[arg(long)]
        max_tokens: Option<u32>,
    },

    /// Classify text to extract facets
    ClassifyText {
        /// Text to classify
        text: String,

        /// Output format (json|yaml|text)
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Legacy Commands - Preserved from original implementation

    /// Parse and validate frontmatter
    Parse {
        /// Input markdown file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "yaml")]
        format: String,
    },

    /// Classify document using ontology
    Classify {
        /// Input file to classify
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Ontology file path
        #[arg(long)]
        ontology: Option<PathBuf>,

        /// Rules file path
        #[arg(long)]
        rules: Option<PathBuf>,

        /// Output format
        #[arg(short, long, default_value = "human")]
        format: String,

        /// Confidence threshold for display
        #[arg(long, default_value = "0.5")]
        confidence_threshold: f32,
    },

    /// Import and transform documents to standard format
    Import {
        /// Input file or folder
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Output contexts folder
        #[arg(long, default_value = "./contexts")]
        contexts_dir: PathBuf,

        /// Interactive mode for conflict resolution
        #[arg(long)]
        interactive: bool,

        /// Force overwrite existing documents
        #[arg(long)]
        force: bool,

        /// Suggested document type
        #[arg(long)]
        r#type: Option<String>,

        /// Suggested domain
        #[arg(long)]
        domain: Option<String>,

        /// Suggested tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Dry run (show what would be done)
        #[arg(long)]
        dry_run: bool,
    },

    /// Index context documents into SQLite database
    Index {
        /// Root directory to scan
        #[arg(long, default_value = "./contexts")]
        path: PathBuf,

        /// Database file path
        #[arg(long, default_value = "ctxindex.db")]
        db: PathBuf,

        /// Ontology file path
        #[arg(long, default_value = "ontology.yaml")]
        ontology: PathBuf,

        /// Rules file path (optional)
        #[arg(long)]
        rules: Option<PathBuf>,

        /// Full reindex
        #[arg(long)]
        full: bool,
    },

    /// Validate a single context document against schema
    Validate {
        /// Input markdown file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "human")]
        format: String,

        /// Auto-fix validation errors
        #[arg(long)]
        fix: bool,
    },

    /// Analyze dependency graph
    Graph {
        /// Root directory to analyze
        #[arg(long, default_value = "./contexts")]
        path: PathBuf,

        /// Output format (dot|json|text)
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Search documents
    Search {
        /// Search query
        query: String,

        /// Filter by tags
        #[arg(long)]
        tags: Option<Vec<String>>,

        /// Results limit
        #[arg(long, default_value = "10")]
        limit: usize,
    },

    /// Mode Switchers - PLAN.md Architecture

    /// Start HTTP API server
    Server {
        /// Server port
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Server host
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Start MCP (Model Context Protocol) server
    Mcp,

    #[cfg(feature = "ui_tauri")]
    /// Start Tauri UI (Reader + Prompt)
    Ui,

    /// Show CLI help menu
    Cli,
}

pub async fn run_app() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or_else(|| {
        #[cfg(feature = "ui_tauri")]
        return Commands::Ui;
        
        #[cfg(not(feature = "ui_tauri"))]
        {
            eprintln!("UI feature not enabled. Available commands:");
            let mut cmd = Cli::command();
            let _ = cmd.print_help();
            std::process::exit(1);
        }
    }) {
        // PLAN.md Core Commands
        Commands::Compose {
            query,
            max_tokens,
            tags,
            format,
        } => {
            let input = ComposeInput {
                query,
                tags,
                max_tokens,
                exclude_sources: None,
                priority_sources: None,
            };

            let bundle = services::compose_prompt(input)?;

            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&bundle)?),
                _ => {
                    println!("=== Context ===");
                    println!("{}", bundle.context);
                    println!("\n=== Prompt ===");
                    println!("{}", bundle.prompt);
                    println!(
                        "\nTokens: {}, Sources: {}, Confidence: {:.2}",
                        bundle.tokens,
                        bundle.sources.len(),
                        bundle.confidence
                    );
                }
            }
            Ok(())
        }

        Commands::Work {
            task,
            context,
            provider,
            temperature,
            max_tokens,
        } => {
            let input = WorkInput {
                task,
                context,
                ai_provider: provider,
                max_tokens,
                temperature,
            };

            let result = services::work(input).await?;

            println!("=== AI Response ===");
            println!("{}", result.response.content);
            println!(
                "\nModel: {}, Tokens: {}, Execution ID: {}",
                result.response.model, result.response.tokens_used, result.execution_id
            );
            Ok(())
        }

        Commands::ClassifyText { text, format } => {
            let facets = services::classify_text(&text)?;

            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&facets)?),
                "yaml" => println!("{}", serde_yaml::to_string(&facets)?),
                _ => {
                    println!("Extracted facets: {}", facets.join(", "));
                }
            }
            Ok(())
        }

        // Mode Switchers
        Commands::Server { port, host } => {
            println!("Starting HTTP server on {}:{}", host, port);
            http::run_server(host, port).await
        }

        Commands::Mcp => {
            println!("Starting MCP server on stdio...");
            mcp::run_mcp_server().await
        }

        // Legacy Commands - Forward to existing CLI services
        Commands::Parse { file, format } => {
            crate::services::cli::run_parse(crate::services::cli::ParseArgs { file, format })
        }

        Commands::Classify {
            file,
            ontology,
            rules,
            format,
            confidence_threshold,
        } => crate::services::cli::run_classify(crate::services::cli::ClassifyArgs {
            file,
            ontology,
            rules,
            format,
            confidence_threshold,
        }),

        Commands::Import {
            input,
            contexts_dir,
            interactive,
            force,
            r#type,
            domain,
            tags,
            dry_run,
        } => crate::services::cli::run_import(crate::services::cli::ImportArgs {
            input,
            contexts_dir,
            interactive,
            force,
            doc_type: r#type,
            domain,
            tags,
            dry_run,
        }),

        Commands::Index {
            path,
            db,
            ontology,
            rules,
            full,
        } => crate::services::cli::run_index(crate::services::cli::IndexArgs {
            path,
            db,
            ontology,
            rules,
            full,
        }),

        Commands::Validate { file, format, fix } => {
            crate::services::cli::run_validate(crate::services::cli::ValidateArgs {
                file,
                format,
                fix,
            })
        }

        Commands::Graph { path, format } => {
            crate::services::cli::run_graph(crate::services::cli::GraphArgs { path, format })
        }

        Commands::Search { query, tags, limit } => {
            crate::services::cli::run_search(crate::services::cli::SearchArgs {
                query,
                tags,
                limit,
            })
        }

        #[cfg(feature = "ui_tauri")]
        Commands::Ui => {
            println!("Starting Tauri UI (Reader + Prompt)...");
            crate::app::ui::tauri_app::run_tauri_ui().await
        }

        Commands::Cli => {
            let mut cmd = Cli::command();
            cmd.print_help().map_err(|e| crate::domain::errors::ContextError::Other(e.to_string()))?;
            Ok(())
        }
    }
}
