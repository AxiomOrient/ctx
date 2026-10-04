use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub struct CtxError(pub String);

impl Display for CtxError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for CtxError {}

impl From<std::io::Error> for CtxError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<serde_yaml::Error> for CtxError {
    fn from(value: serde_yaml::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<serde_json::Error> for CtxError {
    fn from(value: serde_json::Error) -> Self {
        Self(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CtxError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ontology {
    pub version: u32,
    #[serde(default)]
    pub types: BTreeMap<String, TypeSpec>,
    #[serde(default)]
    pub relations: BTreeMap<String, RelationSpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeSpec {
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationSpec {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub from: Vec<String>,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub symmetric: bool,
    #[serde(default)]
    pub acyclic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topology {
    pub version: u32,
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    pub id: String,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub relation: String,
    pub to: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRef {
    pub path: String,
    pub lines: LineRange,
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub witness: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ValidationReport {
    pub issues: Vec<Issue>,
}

impl ValidationReport {
    pub fn error(&mut self, code: &str, message: impl Into<String>, location: Option<String>) {
        self.issues.push(Issue {
            severity: Severity::Error,
            code: code.to_string(),
            message: message.into(),
            location,
            witness: Vec::new(),
        });
    }

    pub fn error_with_witness(
        &mut self,
        code: &str,
        message: impl Into<String>,
        location: Option<String>,
        witness: Vec<String>,
    ) {
        self.issues.push(Issue {
            severity: Severity::Error,
            code: code.to_string(),
            message: message.into(),
            location,
            witness,
        });
    }

    pub fn warning(&mut self, code: &str, message: impl Into<String>, location: Option<String>) {
        self.issues.push(Issue {
            severity: Severity::Warning,
            code: code.to_string(),
            message: message.into(),
            location,
            witness: Vec::new(),
        });
    }

    pub fn is_ok(&self) -> bool {
        !self
            .issues
            .iter()
            .any(|issue| matches!(issue.severity, Severity::Error))
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| matches!(issue.severity, Severity::Error))
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues.len() - self.error_count()
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Valid,
    Unpinned,
    Stale,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceInspection {
    pub state: EvidenceState,
    pub path: String,
    pub lines: LineRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Connection {
    pub edge_id: String,
    pub from: String,
    pub relation: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Neighborhood {
    pub root: String,
    pub depth: usize,
    pub entities: Vec<Entity>,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathResult {
    pub from: String,
    pub to: String,
    pub connections: Vec<Connection>,
}
