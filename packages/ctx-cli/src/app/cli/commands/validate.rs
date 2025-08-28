use clap::Args;
use std::path::PathBuf;

use ctx_core::doc::parse::frontmatter::FrontmatterParser;
use ctx_core::doc::schema::v1::V1SchemaValidator;
use ctx_core::doc::schema::SchemaValidator;
use serde::Serialize;


#[derive(Debug, Args, Clone)]
pub struct ValidateArgs {
    /// Input markdown file
    pub file: PathBuf,
    /// Output format: human|json|yaml
    #[arg(short, long, default_value = "human")]
    pub format: String,
}

pub fn run_validate(args: ValidateArgs) -> ctx_core::Result<()> {
    let content = std::fs::read_to_string(&args.file)?;
    let parser = FrontmatterParser::new();
    let (frontmatter, _body) = parser.extract_frontmatter(&content)?;
    let doc: ctx_core::ContextDocument = serde_yaml::from_str(&frontmatter)
        .map_err(|e| ctx_core::ContextError::InvalidFrontmatter(e.to_string()))?;

    let validator = V1SchemaValidator::new();
    let result = validator.validate_document(&doc)?;

    match args.format.as_str() {
        "human" => {
            println!(
                "Validation: {}",
                if result.is_valid {
                    "✅ OK"
                } else {
                    "❌ INVALID"
                }
            );
            if !result.errors.is_empty() {
                println!("Errors:");
                for e in result.errors {
                    println!("  - [{}] {}", e.field, e.message);
                }
            }
            if !result.warnings.is_empty() {
                println!("Warnings:");
                for w in result.warnings {
                    println!("  - [{}] {}", w.field, w.message);
                }
            }
        }
        "json" | "yaml" => {
            #[derive(Serialize)]
            struct SimpleErr<'a> { field: &'a str, message: &'a str }
            #[derive(Serialize)]
            struct SimpleWarn<'a> { field: &'a str, message: &'a str }

            #[derive(Serialize)]
            struct Out<'a> {
                valid: bool,
                errors: Vec<SimpleErr<'a>>,
                warnings: Vec<SimpleWarn<'a>>,
            }

            let errors: Vec<SimpleErr> = result
                .errors
                .iter()
                .map(|e| SimpleErr { field: e.field.as_str(), message: e.message.as_str() })
                .collect();
            let warnings: Vec<SimpleWarn> = result
                .warnings
                .iter()
                .map(|w| SimpleWarn { field: w.field.as_str(), message: w.message.as_str() })
                .collect();
            let out = Out { valid: result.is_valid, errors, warnings };

            if args.format == "json" {
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("{}", serde_yaml::to_string(&out)?);
            }
        }
        other => {
            return Err(ctx_core::ContextError::Other(format!(
                "Unsupported format: {}",
                other
            )));
        }
    }

    Ok(())
}
