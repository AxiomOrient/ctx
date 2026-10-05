use clap::{Parser, Subcommand};
use ctx::{
    CtxError, EvidenceMatch, EvidenceState, QueryMatch, QueryMatchKind, Result, SemanticDecision,
    VerdictStatus, compile_workspace, neighborhood, query_entities, resolve_evidence,
    select_entity_with_command, semantic_candidates, shortest_path, validate_workspace,
};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ctx", version, about = "Deterministic evidence graph")]
struct Cli {
    #[arg(long, default_value = ".", global = true)]
    workspace: PathBuf,

    #[arg(long, default_value = "schema.yaml", global = true)]
    schema: PathBuf,

    #[arg(long, default_value = "knowledge", global = true)]
    knowledge: PathBuf,

    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate schema, compiled graph and evidence.
    Check,
    /// Explain nearby declarations and their evidence.
    Explain {
        id: String,
        #[arg(long, default_value_t = 1)]
        depth: usize,
    },
    /// Find a directed declared path. Symmetric relations work both ways.
    Path { from: String, to: String },
    /// Find entities by id/title/alias, optionally escalating ambiguity to an external decision adapter.
    Query {
        text: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        decider: Option<PathBuf>,
        #[arg(long = "decider-arg")]
        decider_args: Vec<String>,
    },
}

#[derive(Serialize)]
struct ExplainOutput {
    graph: ctx::Neighborhood,
    edges: Vec<ExplainEdge>,
}

#[derive(Serialize)]
struct ExplainEdge {
    connection: ctx::Connection,
    evidence: Vec<EvidenceMatch>,
}

#[derive(Serialize)]
struct QueryOutput {
    deterministic: Vec<QueryMatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    semantic: Option<SemanticDecision>,
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    let workspace = compile_workspace(&cli.workspace, &cli.schema, &cli.knowledge)?;
    let report = validate_workspace(&workspace.root, &workspace.schema, &workspace.graph);

    match cli.command {
        Command::Check => {
            if cli.json {
                print_json(&report)?;
            } else {
                print_report(&report);
            }
            Ok(if report.is_complete() { 0 } else { 1 })
        }
        Command::Explain { id, depth } => {
            require_structural_validity(&report)?;
            let graph = neighborhood(&workspace.graph, &id, depth)?;
            let mut edges = Vec::new();

            for connection in &graph.connections {
                if let Some(edge) = workspace.graph.edges.iter().find(|edge| {
                    edge.from.as_str() == connection.from.as_str()
                        && edge.relation.as_str() == connection.relation.as_str()
                        && edge.to.as_str() == connection.to.as_str()
                        && edge.declaration.as_str() == connection.declaration.as_str()
                }) {
                    edges.push(ExplainEdge {
                        connection: connection.clone(),
                        evidence: edge
                            .evidence
                            .iter()
                            .map(|selector| {
                                resolve_evidence(&workspace.root, &edge.declaration, selector)
                            })
                            .collect(),
                    });
                }
            }

            let output = ExplainOutput { graph, edges };
            if cli.json {
                print_json(&output)?;
            } else {
                print_explain(&output);
            }
            Ok(0)
        }
        Command::Path { from, to } => {
            require_structural_validity(&report)?;
            let path = shortest_path(&workspace.schema, &workspace.graph, &from, &to)?;
            if cli.json {
                print_json(&path)?;
            } else if let Some(path) = path {
                if path.connections.is_empty() {
                    println!("{from}");
                } else {
                    for edge in path.connections {
                        println!(
                            "{} --{}--> {}  ({})",
                            edge.from, edge.relation, edge.to, edge.declaration
                        );
                    }
                }
            } else {
                println!("no path");
            }
            Ok(0)
        }
        Command::Query {
            text,
            limit,
            decider,
            decider_args,
        } => {
            require_structural_validity(&report)?;
            let all_matches = query_entities(&workspace.graph, &text);
            let exact_matches: Vec<QueryMatch> = all_matches
                .iter()
                .filter(|item| item.kind != QueryMatchKind::Contains)
                .cloned()
                .collect();

            let semantic = match decider {
                Some(program) if exact_matches.len() != 1 => {
                    let candidates = semantic_candidates(&workspace.graph, &all_matches);
                    Some(select_entity_with_command(
                        &program,
                        &decider_args,
                        &text,
                        &candidates,
                    )?)
                }
                _ => None,
            };

            let output = QueryOutput {
                deterministic: all_matches.into_iter().take(limit).collect(),
                semantic,
            };

            if cli.json {
                print_json(&output)?;
            } else {
                print_query(&output);
            }
            Ok(0)
        }
    }
}

fn require_structural_validity(report: &ctx::ValidationReport) -> Result<()> {
    if report.can_query() {
        Ok(())
    } else {
        Err(CtxError(format!(
            "knowledge graph has {} structural violation(s); run ctx check",
            report.violated_count()
        )))
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_report(report: &ctx::ValidationReport) {
    for verdict in report
        .verdicts
        .iter()
        .filter(|verdict| verdict.status != VerdictStatus::Satisfied)
    {
        let status = match verdict.status {
            VerdictStatus::Satisfied => "SATISFIED",
            VerdictStatus::Violated => "VIOLATED",
            VerdictStatus::Unknown => "UNKNOWN",
        };
        println!(
            "{status} {} {}: {}",
            verdict.code, verdict.subject, verdict.message
        );
    }

    println!(
        "{} satisfied, {} violated, {} unknown",
        report.satisfied_count(),
        report.violated_count(),
        report.unknown_count()
    );
}

fn print_explain(output: &ExplainOutput) {
    println!("root: {}", output.graph.root);
    for entity in &output.graph.entities {
        println!(
            "entity {} [{}] {}  ({})",
            entity.id, entity.kind, entity.title, entity.document
        );
    }

    for item in &output.edges {
        println!(
            "edge: {} --{}--> {}  ({})",
            item.connection.from,
            item.connection.relation,
            item.connection.to,
            item.connection.declaration
        );
        if item.evidence.is_empty() {
            println!("  evidence: unknown (not declared)");
        }
        for evidence in &item.evidence {
            let state = match evidence.state {
                EvidenceState::Valid => "valid",
                EvidenceState::Relocated => "relocated",
                EvidenceState::Stale => "stale",
                EvidenceState::Ambiguous => "ambiguous",
                EvidenceState::Missing => "missing",
                EvidenceState::Invalid => "invalid",
            };
            let location = match (evidence.start_line, evidence.end_line) {
                (Some(start), Some(end)) => format!(":{start}-{end}"),
                _ => String::new(),
            };
            println!("  evidence {state}: {}{}", evidence.source, location);
            println!("    {}", evidence.exact.replace('\n', "\n    "));
        }
    }
}

fn print_query(output: &QueryOutput) {
    for item in &output.deterministic {
        println!(
            "{}  {} [{}] {}  ({})",
            match_kind(item.kind),
            item.entity.id,
            item.entity.kind,
            item.entity.title,
            item.entity.document
        );
    }

    if let Some(decision) = &output.semantic {
        match &decision.entity {
            Some(entity) => {
                let confidence = decision
                    .confidence
                    .map(|value| format!(" confidence={value:.4}"))
                    .unwrap_or_default();
                println!(
                    "semantic  {} [{}] {}{}",
                    entity.id, entity.kind, entity.title, confidence
                );
            }
            None => println!("semantic  no selection"),
        }
    }
}

fn match_kind(kind: QueryMatchKind) -> &'static str {
    match kind {
        QueryMatchKind::Id => "id",
        QueryMatchKind::Title => "title",
        QueryMatchKind::Alias => "alias",
        QueryMatchKind::Contains => "contains",
    }
}
