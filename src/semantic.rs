use crate::domain::{CtxError, Entity, GraphSnapshot, QueryMatch, QueryMatchKind, Result};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const DECIDER_PROTOCOL_VERSION: u32 = 1;

const DECIDER_TIMEOUT: Duration = Duration::from_secs(30);
const DECIDER_MAX_STDOUT_BYTES: usize = 64 * 1024;
const DECIDER_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Serialize)]
pub struct DecisionCandidate {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecisionRequest {
    pub version: u32,
    pub task: &'static str,
    pub query: String,
    pub candidates: Vec<DecisionCandidate>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionResponse {
    pub entity_id: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticDecision {
    pub entity: Option<Entity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

pub fn semantic_candidates(graph: &GraphSnapshot, matches: &[QueryMatch]) -> Vec<Entity> {
    let exact: Vec<Entity> = matches
        .iter()
        .filter(|item| item.kind != QueryMatchKind::Contains)
        .map(|item| item.entity.clone())
        .collect();

    if exact.is_empty() {
        graph.entities.clone()
    } else {
        exact
    }
}

pub fn select_entity_with_command(
    program: &Path,
    args: &[String],
    query: &str,
    candidates: &[Entity],
) -> Result<SemanticDecision> {
    if candidates.is_empty() {
        return Ok(SemanticDecision {
            entity: None,
            confidence: None,
        });
    }

    let request = DecisionRequest {
        version: DECIDER_PROTOCOL_VERSION,
        task: "select_entity",
        query: query.to_string(),
        candidates: candidates
            .iter()
            .map(|entity| DecisionCandidate {
                id: entity.id.clone(),
                kind: entity.kind.clone(),
                title: entity.title.clone(),
                aliases: entity.aliases.clone(),
            })
            .collect(),
    };

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| {
            CtxError(format!(
                "failed to start decision adapter '{}': {error}",
                program.display()
            ))
        })?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| CtxError("decision adapter stdin is unavailable".into()))?;
        serde_json::to_writer(&mut stdin, &request)?;
        stdin.write_all(b"\n")?;
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CtxError("decision adapter stdout is unavailable".into()))?;
    let reader = thread::spawn(move || read_bounded(stdout, DECIDER_MAX_STDOUT_BYTES));

    let deadline = Instant::now() + DECIDER_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(CtxError(format!(
                "decision adapter '{}' timed out after {} seconds",
                program.display(),
                DECIDER_TIMEOUT.as_secs()
            )));
        }
        thread::sleep(DECIDER_POLL_INTERVAL);
    };

    let (stdout, overflowed) = reader
        .join()
        .map_err(|_| CtxError("decision adapter stdout reader panicked".into()))??;

    if overflowed {
        return Err(CtxError(format!(
            "decision adapter '{}' exceeded {} bytes of stdout",
            program.display(),
            DECIDER_MAX_STDOUT_BYTES
        )));
    }

    if !status.success() {
        return Err(CtxError(format!(
            "decision adapter '{}' exited with {}",
            program.display(),
            status
        )));
    }

    let response: DecisionResponse = serde_json::from_slice(&stdout)?;
    validate_response(response, candidates)
}


fn read_bounded<R: Read>(mut reader: R, limit: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut output = Vec::with_capacity(limit.min(4096));
    let mut chunk = [0_u8; 8192];
    let mut overflowed = false;

    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }

        if output.len() < limit {
            let remaining = limit - output.len();
            let keep = remaining.min(read);
            output.extend_from_slice(&chunk[..keep]);
            if keep < read {
                overflowed = true;
            }
        } else {
            overflowed = true;
        }
    }

    Ok((output, overflowed))
}

fn validate_response(
    response: DecisionResponse,
    candidates: &[Entity],
) -> Result<SemanticDecision> {
    if let Some(confidence) = response.confidence {
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(CtxError(
                "decision adapter confidence must be finite and within 0..=1".into(),
            ));
        }
    }

    let entity = match response.entity_id {
        None => None,
        Some(id) => Some(
            candidates
                .iter()
                .find(|entity| entity.id == id)
                .cloned()
                .ok_or_else(|| {
                    CtxError(format!(
                        "decision adapter returned entity '{id}' outside the candidate set"
                    ))
                })?,
        ),
    };

    Ok(SemanticDecision {
        entity,
        confidence: response.confidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: &str) -> Entity {
        Entity {
            id: id.into(),
            kind: "Thing".into(),
            title: id.into(),
            aliases: Vec::new(),
            document: format!("knowledge/{id}.md"),
        }
    }

    #[test]
    fn contains_matches_do_not_limit_semantic_recall() {
        let graph = GraphSnapshot {
            entities: vec![entity("lexical"), entity("semantic")],
            edges: Vec::new(),
        };
        let matches = vec![QueryMatch {
            kind: QueryMatchKind::Contains,
            entity: entity("lexical"),
        }];

        let candidates = semantic_candidates(&graph, &matches);
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().any(|item| item.id == "semantic"));
    }

    #[test]
    fn exact_matches_bound_semantic_ambiguity() {
        let graph = GraphSnapshot {
            entities: vec![entity("a"), entity("b"), entity("c")],
            edges: Vec::new(),
        };
        let matches = vec![
            QueryMatch {
                kind: QueryMatchKind::Title,
                entity: entity("a"),
            },
            QueryMatch {
                kind: QueryMatchKind::Alias,
                entity: entity("b"),
            },
        ];

        let candidates = semantic_candidates(&graph, &matches);
        assert_eq!(candidates.len(), 2);
        assert!(!candidates.iter().any(|item| item.id == "c"));
    }

    #[test]
    fn bounded_reader_caps_memory_and_reports_overflow() {
        let input = vec![b'x'; 32];
        let (output, overflowed) = read_bounded(std::io::Cursor::new(input), 8).unwrap();
        assert_eq!(output.len(), 8);
        assert!(overflowed);
    }

    #[test]
    fn rejects_entity_outside_candidate_set() {
        let result = validate_response(
            DecisionResponse {
                entity_id: Some("missing".into()),
                confidence: Some(0.9),
            },
            &[entity("known")],
        );
        assert!(result.is_err());
    }

    #[test]
    fn rejects_invalid_confidence() {
        let result = validate_response(
            DecisionResponse {
                entity_id: Some("known".into()),
                confidence: Some(1.1),
            },
            &[entity("known")],
        );
        assert!(result.is_err());
    }
}
