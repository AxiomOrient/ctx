use clap::{Parser, Subcommand};
use ctx::{
    CtxError, EvidenceState, Result, inspect_evidence, load_ontology, load_topology,
    neighborhood, pin_topology, shortest_path, validate_workspace,
};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "ctx", version, about = "Deterministic evidence graph")]
struct Cli {
    #[arg(long, default_value = ".", global = true)]
    workspace: PathBuf,

    #[arg(long, default_value = "ontology.yaml", global = true)]
    ontology: PathBuf,

    #[arg(long, default_value = "topology.yaml", global = true)]
    topology: PathBuf,

    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate types, relations, cycles, references and evidence.
    Check,
    /// Pin every evidence slice to its current SHA-256 digest.
    Pin,
    /// Explain the local graph around one entity, including evidence.
    Explain {
        id: String,
        #[arg(long, default_value_t = 1)]
        depth: usize,
    },
    /// Find a directed path. Symmetric relations work in both directions.
    Path { from: String, to: String },
}

#[derive(Serialize)]
struct ExplainOutput {
    graph: ctx::Neighborhood,
    evidence: Vec<EdgeEvidence>,
}

#[derive(Serialize)]
struct EdgeEvidence {
    edge_id: String,
    evidence: Vec<ctx::EvidenceInspection>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let workspace = cli.workspace.canonicalize()?;
    let ontology_path = resolve_inside(&workspace, &cli.ontology)?;
    let topology_path = resolve_inside(&workspace, &cli.topology)?;

    match cli.command {
        Command::Pin => {
            let mut topology = load_topology(&topology_path)?;
            let count = pin_topology(&workspace, &topology_path, &mut topology)?;
            if cli.json {
                println!("{}", serde_json::json!({ "pinned": count }));
            } else {
                println!("pinned {count} evidence reference(s)");
            }
            Ok(())
        }
        command => {
            let ontology = load_ontology(&ontology_path)?;
            let topology = load_topology(&topology_path)?;
            let report = validate_workspace(&workspace, &ontology, &topology);

            if matches!(&command, Command::Check) {
                if cli.json {
                    print_json(&report)?;
                } else {
                    print_report(&report);
                }
                if report.is_ok() {
                    return Ok(());
                }
                return Err(CtxError(format!(
                    "validation failed with {} error(s)",
                    report.error_count()
                )));
            }

            if !report.is_ok() {
                return Err(CtxError(format!(
                    "workspace is invalid; run `ctx check` ({} error(s))",
                    report.error_count()
                )));
            }

            match command {
                Command::Explain { id, depth } => {
                    let graph = neighborhood(&topology, &id, depth)?;
                    let mut evidence = Vec::new();
                    for connection in &graph.connections {
                        if let Some(edge) = topology.edges.iter().find(|edge| edge.id == connection.edge_id) {
                            evidence.push(EdgeEvidence {
                                edge_id: edge.id.clone(),
                                evidence: edge
                                    .evidence
                                    .iter()
                                    .map(|item| inspect_evidence(&workspace, item))
                                    .collect(),
                            });
                        }
                    }
                    let output = ExplainOutput { graph, evidence };
                    if cli.json {
                        print_json(&output)?;
                    } else {
                        print_explain(&output);
                    }
                    Ok(())
                }
                Command::Path { from, to } => {
                    let path = shortest_path(&ontology, &topology, &from, &to)?;
                    if cli.json {
                        print_json(&path)?;
                    } else if let Some(path) = path {
                        if path.connections.is_empty() {
                            println!("{from}");
                        } else {
                            for connection in path.connections {
                                println!(
                                    "{} --{}--> {}  [{}]",
                                    connection.from,
                                    connection.relation,
                                    connection.to,
                                    connection.edge_id
                                );
                            }
                        }
                    } else {
                        println!("no path");
                    }
                    Ok(())
                }
                Command::Check | Command::Pin => unreachable!(),
            }
        }
    }
}

fn resolve_inside(workspace: &Path, path: &Path) -> Result<PathBuf> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    };
    let canonical = joined.canonicalize()?;
    if !canonical.starts_with(workspace) {
        return Err(CtxError(format!(
            "configuration path escapes workspace: {}",
            path.display()
        )));
    }
    Ok(canonical)
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_report(report: &ctx::ValidationReport) {
    if report.issues.is_empty() {
        println!("ok");
        return;
    }
    for issue in &report.issues {
        let severity = match issue.severity {
            ctx::Severity::Error => "ERROR",
            ctx::Severity::Warning => "WARN",
        };
        match &issue.location {
            Some(location) => println!("{severity} {} {location}: {}", issue.code, issue.message),
            None => println!("{severity} {}: {}", issue.code, issue.message),
        }
        if !issue.witness.is_empty() {
            println!("  witness: {}", issue.witness.join(" -> "));
        }
    }
    println!(
        "{} error(s), {} warning(s)",
        report.error_count(),
        report.warning_count()
    );
}

fn print_explain(output: &ExplainOutput) {
    println!("root: {}", output.graph.root);
    for entity in &output.graph.entities {
        println!("entity {} [{}] {}", entity.id, entity.kind, entity.title);
    }
    for connection in &output.graph.connections {
        println!(
            "edge {}: {} --{}--> {}",
            connection.edge_id, connection.from, connection.relation, connection.to
        );
        if let Some(edge_evidence) = output
            .evidence
            .iter()
            .find(|item| item.edge_id == connection.edge_id)
        {
            for evidence in &edge_evidence.evidence {
                let state = match evidence.state {
                    EvidenceState::Valid => "valid",
                    EvidenceState::Unpinned => "unpinned",
                    EvidenceState::Stale => "stale",
                    EvidenceState::Missing => "missing",
                    EvidenceState::Invalid => "invalid",
                };
                println!(
                    "  evidence {state}: {}:{}-{}",
                    evidence.path, evidence.lines.start, evidence.lines.end
                );
                if let Some(snippet) = &evidence.snippet {
                    for line in snippet.lines() {
                        println!("    {line}");
                    }
                }
            }
        }
    }
}
