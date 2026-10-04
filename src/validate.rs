use crate::domain::{
    EvidenceState, Ontology, SCHEMA_VERSION, Topology, ValidationReport,
};
use crate::evidence::inspect_evidence;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn validate_workspace(
    workspace: &Path,
    ontology: &Ontology,
    topology: &Topology,
) -> ValidationReport {
    let mut report = ValidationReport::default();

    if ontology.version != SCHEMA_VERSION {
        report.error(
            "ontology.version",
            format!(
                "unsupported ontology version {}; expected {}",
                ontology.version, SCHEMA_VERSION
            ),
            Some("ontology.yaml:version".to_string()),
        );
    }
    if topology.version != SCHEMA_VERSION {
        report.error(
            "topology.version",
            format!(
                "unsupported topology version {}; expected {}",
                topology.version, SCHEMA_VERSION
            ),
            Some("topology.yaml:version".to_string()),
        );
    }

    validate_ontology(ontology, &mut report);

    let mut entities = BTreeMap::new();
    for (index, entity) in topology.entities.iter().enumerate() {
        let location = format!("topology.yaml:entities[{index}]");
        if entities.insert(entity.id.as_str(), entity).is_some() {
            report.error(
                "entity.duplicate_id",
                format!("duplicate entity id '{}'", entity.id),
                Some(location.clone()),
            );
        }
        if !ontology.types.contains_key(&entity.kind) {
            report.error(
                "entity.unknown_type",
                format!("entity '{}' uses unknown type '{}'", entity.id, entity.kind),
                Some(location),
            );
        }
    }

    let mut edge_ids = BTreeSet::new();
    let mut semantic_edges = BTreeSet::new();
    for (index, edge) in topology.edges.iter().enumerate() {
        let location = format!("topology.yaml:edges[{index}]");
        if !edge_ids.insert(edge.id.clone()) {
            report.error(
                "edge.duplicate_id",
                format!("duplicate edge id '{}'", edge.id),
                Some(location.clone()),
            );
        }

        let Some(from_entity) = entities.get(edge.from.as_str()) else {
            report.error(
                "edge.missing_from",
                format!("edge '{}' references missing entity '{}'", edge.id, edge.from),
                Some(location.clone()),
            );
            continue;
        };
        let Some(to_entity) = entities.get(edge.to.as_str()) else {
            report.error(
                "edge.missing_to",
                format!("edge '{}' references missing entity '{}'", edge.id, edge.to),
                Some(location.clone()),
            );
            continue;
        };
        let Some(relation) = ontology.relations.get(&edge.relation) else {
            report.error(
                "edge.unknown_relation",
                format!(
                    "edge '{}' uses unknown relation '{}'",
                    edge.id, edge.relation
                ),
                Some(location.clone()),
            );
            continue;
        };

        if !accepts(&relation.from, &from_entity.kind) {
            report.error(
                "edge.domain_mismatch",
                format!(
                    "relation '{}' does not accept '{}' as source type",
                    edge.relation, from_entity.kind
                ),
                Some(location.clone()),
            );
        }
        if !accepts(&relation.to, &to_entity.kind) {
            report.error(
                "edge.range_mismatch",
                format!(
                    "relation '{}' does not accept '{}' as target type",
                    edge.relation, to_entity.kind
                ),
                Some(location.clone()),
            );
        }

        if edge.evidence.is_empty() {
            report.warning(
                "edge.no_evidence",
                format!("edge '{}' has no evidence", edge.id),
                Some(location.clone()),
            );
        }

        let semantic_key = if relation.symmetric && edge.from > edge.to {
            format!("{}|{}|{}", edge.to, edge.relation, edge.from)
        } else {
            format!("{}|{}|{}", edge.from, edge.relation, edge.to)
        };
        if !semantic_edges.insert(semantic_key) {
            report.warning(
                "edge.duplicate_assertion",
                format!("edge '{}' duplicates an existing assertion", edge.id),
                Some(location.clone()),
            );
        }

        for (evidence_index, evidence) in edge.evidence.iter().enumerate() {
            let evidence_location = format!("{location}.evidence[{evidence_index}]");
            let inspected = inspect_evidence(workspace, evidence);
            match inspected.state {
                EvidenceState::Valid => {}
                EvidenceState::Unpinned => report.warning(
                    "evidence.unpinned",
                    format!(
                        "edge '{}' evidence is not pinned; run `ctx pin`",
                        edge.id
                    ),
                    Some(evidence_location),
                ),
                EvidenceState::Stale => report.error(
                    "evidence.stale",
                    format!(
                        "edge '{}' evidence digest changed: expected {}, actual {}",
                        edge.id,
                        inspected.expected_digest.as_deref().unwrap_or("<missing>"),
                        inspected.actual_digest.as_deref().unwrap_or("<missing>")
                    ),
                    Some(evidence_location),
                ),
                EvidenceState::Missing | EvidenceState::Invalid => report.error(
                    "evidence.invalid",
                    inspected
                        .message
                        .unwrap_or_else(|| format!("edge '{}' has invalid evidence", edge.id)),
                    Some(evidence_location),
                ),
            }
        }
    }

    for (relation_name, relation) in &ontology.relations {
        if relation.acyclic {
            let adjacency = relation_adjacency(topology, relation_name);
            if let Some(cycle) = find_cycle(&adjacency) {
                report.error_with_witness(
                    "relation.cycle",
                    format!("relation '{relation_name}' must be acyclic"),
                    Some(format!("ontology.yaml:relations.{relation_name}")),
                    cycle,
                );
            }
        }
    }

    report
}

fn validate_ontology(ontology: &Ontology, report: &mut ValidationReport) {
    for (name, relation) in &ontology.relations {
        if relation.from.is_empty() {
            report.error(
                "relation.missing_domain",
                format!("relation '{name}' must declare `from`; use ['*'] for any type"),
                Some(format!("ontology.yaml:relations.{name}.from")),
            );
        }
        if relation.to.is_empty() {
            report.error(
                "relation.missing_range",
                format!("relation '{name}' must declare `to`; use ['*'] for any type"),
                Some(format!("ontology.yaml:relations.{name}.to")),
            );
        }
        if relation.symmetric && relation.acyclic {
            report.error(
                "relation.invalid_policy",
                format!("relation '{name}' cannot be both symmetric and acyclic"),
                Some(format!("ontology.yaml:relations.{name}")),
            );
        }
        for type_name in relation.from.iter().chain(relation.to.iter()) {
            if type_name != "*" && !ontology.types.contains_key(type_name) {
                report.error(
                    "relation.unknown_type",
                    format!("relation '{name}' references unknown type '{type_name}'"),
                    Some(format!("ontology.yaml:relations.{name}")),
                );
            }
        }
    }
}

fn accepts(allowed: &[String], actual: &str) -> bool {
    allowed.iter().any(|value| value == "*" || value == actual)
}

fn relation_adjacency(topology: &Topology, relation: &str) -> BTreeMap<String, Vec<String>> {
    let mut adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for edge in topology.edges.iter().filter(|edge| edge.relation == relation) {
        adjacency
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
        adjacency.entry(edge.to.clone()).or_default();
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort();
        neighbors.dedup();
    }
    adjacency
}

fn find_cycle(adjacency: &BTreeMap<String, Vec<String>>) -> Option<Vec<String>> {
    let mut state: BTreeMap<String, u8> = BTreeMap::new();
    let mut stack = Vec::new();

    for node in adjacency.keys() {
        if state.get(node).copied().unwrap_or(0) == 0 {
            if let Some(cycle) = dfs_cycle(node, adjacency, &mut state, &mut stack) {
                return Some(cycle);
            }
        }
    }
    None
}

fn dfs_cycle(
    node: &str,
    adjacency: &BTreeMap<String, Vec<String>>,
    state: &mut BTreeMap<String, u8>,
    stack: &mut Vec<String>,
) -> Option<Vec<String>> {
    state.insert(node.to_string(), 1);
    stack.push(node.to_string());

    if let Some(neighbors) = adjacency.get(node) {
        for next in neighbors {
            match state.get(next).copied().unwrap_or(0) {
                0 => {
                    if let Some(cycle) = dfs_cycle(next, adjacency, state, stack) {
                        return Some(cycle);
                    }
                }
                1 => {
                    let start = stack.iter().position(|value| value == next).unwrap_or(0);
                    let mut cycle = stack[start..].to_vec();
                    cycle.push(next.clone());
                    return Some(cycle);
                }
                _ => {}
            }
        }
    }

    stack.pop();
    state.insert(node.to_string(), 2);
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Edge, RelationSpec, TypeSpec};

    #[test]
    fn cycle_detection_returns_witness() {
        let topology = Topology {
            version: 1,
            entities: Vec::new(),
            edges: vec![
                Edge {
                    id: "a-b".into(),
                    from: "a".into(),
                    relation: "depends_on".into(),
                    to: "b".into(),
                    evidence: Vec::new(),
                },
                Edge {
                    id: "b-a".into(),
                    from: "b".into(),
                    relation: "depends_on".into(),
                    to: "a".into(),
                    evidence: Vec::new(),
                },
            ],
        };
        let adjacency = relation_adjacency(&topology, "depends_on");
        let cycle = find_cycle(&adjacency).unwrap();
        assert_eq!(cycle.first(), cycle.last());
    }

    #[test]
    fn wildcard_accepts_any_type() {
        assert!(accepts(&["*".into()], "Anything"));
    }

    #[test]
    fn ontology_policy_rejects_symmetric_acyclic() {
        let ontology = Ontology {
            version: 1,
            types: BTreeMap::from([("Thing".into(), TypeSpec::default())]),
            relations: BTreeMap::from([(
                "linked".into(),
                RelationSpec {
                    symmetric: true,
                    acyclic: true,
                    ..RelationSpec::default()
                },
            )]),
        };
        let mut report = ValidationReport::default();
        validate_ontology(&ontology, &mut report);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "relation.invalid_policy"));
    }
}
