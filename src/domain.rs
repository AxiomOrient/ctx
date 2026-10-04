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
pub struct Schema {
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
    #[serde(default)]
    pub constraints: BTreeMap<String, Cardinality>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cardinality {
    #[serde(default)]
    pub min: Option<usize>,
    #[serde(default)]
    pub max: Option<usize>,
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
pub struct ConceptMeta {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub relations: Vec<RelationDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationDecl {
    pub relation: String,
    pub target: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceSelector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSelector {
    #[serde(default)]
    pub source: Option<String>,
    pub exact: String,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub suffix: Option<String>,
    #[serde(default)]
    pub hint: Option<LineHint>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LineHint {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entity {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub document: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub from: String,
    pub relation: String,
    pub to: String,
    pub evidence: Vec<EvidenceSelector>,
    pub declaration: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GraphSnapshot {
    pub entities: Vec<Entity>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Valid,
    Relocated,
    Stale,
    Ambiguous,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceMatch {
    pub state: EvidenceState,
    pub source: String,
    pub exact: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerdictStatus {
    Satisfied,
    Violated,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Witness {
    Schema {
        path: String,
    },
    Declaration {
        path: String,
    },
    Assertion {
        from: String,
        relation: String,
        to: String,
    },
    Evidence {
        source: String,
        state: EvidenceState,
        exact: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        start_line: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        end_line: Option<usize>,
    },
    Cycle {
        path: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Verdict {
    pub status: VerdictStatus,
    pub code: String,
    pub subject: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub witness: Vec<Witness>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ValidationReport {
    pub verdicts: Vec<Verdict>,
}

impl ValidationReport {
    pub fn push(
        &mut self,
        status: VerdictStatus,
        code: impl Into<String>,
        subject: impl Into<String>,
        message: impl Into<String>,
        witness: Vec<Witness>,
    ) {
        self.verdicts.push(Verdict {
            status,
            code: code.into(),
            subject: subject.into(),
            message: message.into(),
            witness,
        });
    }

    pub fn violated_count(&self) -> usize {
        self.verdicts
            .iter()
            .filter(|v| v.status == VerdictStatus::Violated)
            .count()
    }

    pub fn unknown_count(&self) -> usize {
        self.verdicts
            .iter()
            .filter(|v| v.status == VerdictStatus::Unknown)
            .count()
    }

    pub fn satisfied_count(&self) -> usize {
        self.verdicts
            .iter()
            .filter(|v| v.status == VerdictStatus::Satisfied)
            .count()
    }

    pub fn can_query(&self) -> bool {
        self.violated_count() == 0
    }

    pub fn is_complete(&self) -> bool {
        self.violated_count() == 0 && self.unknown_count() == 0
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Connection {
    pub from: String,
    pub relation: String,
    pub to: String,
    pub declaration: String,
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

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueryMatchKind {
    Id,
    Title,
    Alias,
    Contains,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryMatch {
    pub kind: QueryMatchKind,
    pub entity: Entity,
}
