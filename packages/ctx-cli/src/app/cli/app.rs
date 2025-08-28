//! CLI Application Entry Point
//!
//! CLI 애플리케이션의 메인 구조와 명령어 라우팅을 담당합니다.
//! 각 명령어는 독립적인 모듈로 구현되어 있으며, 이 파일은 이들을 연결하는 역할을 합니다.

use super::commands::{
    ClassifyArgs, ImportArgs, IndexArgs, ParseArgs, ValidateArgs, run_classify, run_import,
    run_index, run_mcp, run_parse, run_validate,
};
#[cfg(feature = "server")]
use super::commands::{ServerArgs, run_server};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ctxset")]
#[command(about = "Curated context set generator")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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
    },

    /// Validate a single context document against schema
    Validate {
        /// Input markdown file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "human")]
        format: String,
    },

    /// Start HTTP API server
    #[cfg(feature = "server")]
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
}

use ctx_core::Result;

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse { file, format } => run_parse(ParseArgs { file, format }),

        Commands::Classify {
            file,
            ontology,
            format,
            confidence_threshold,
        } => run_classify(ClassifyArgs {
            file,
            ontology,
            format,
            confidence_threshold,
        }),

        Commands::Import {
            input,
            contexts_dir,
            interactive,
            force,
            r#type: doc_type,
            domain,
            tags,
            dry_run,
        } => run_import(ImportArgs {
            input,
            contexts_dir,
            interactive,
            force,
            doc_type,
            domain,
            tags,
            dry_run,
        }),

        #[cfg(feature = "server")]
        Commands::Server { port, host } => run_server(ServerArgs { port, host }),

        Commands::Mcp => run_mcp(),

        Commands::Index {
            path,
            db,
            ontology,
            rules,
        } => run_index(IndexArgs {
            path,
            db,
            ontology,
            rules,
        }),

        Commands::Validate { file, format } => run_validate(ValidateArgs { file, format }),
    }
}
