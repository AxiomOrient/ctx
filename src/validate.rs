use crate::domain::{
    Edge, EvidenceState, GraphSnapshot, Schema, VerdictStatus, ValidationReport, Witness,
};
use crate::evidence::resolve_evidence;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn validate_workspace(
    workspace: &Path,
    schema: &Schema,
    graph: &GraphSnapshot,
) -> ValidationReport {
    let mut report = ValidationReport::default();
    validate_schema(schema, &mut report);

    let mut entities = BTreeMap::new();
    for entity in &graph.entities {
        if !valid_key(&entity.id) {
            report.push(
                VerdictStatus::Violated,
                "entity.invalid_id",
                &entity.id,
                "entity id must contain only ASCII letters, digits, '.', '_', ':', or '-'",
                vec![Witness::Declaration {
                    path: entity.document.clone(),
                }],
            );
        }

        if entities.insert(entity.id.as_str(), entity).is_some() {
            report.push(
                VerdictStatus::Violated,
                "entity.duplicate_id",
                &entity.id,
                format!("entity '{}' is declared more than once", entity.id),
                vec![Witness::Declaration {
                    path: entity.document.clone(),
                }],
            );
            continue;
        }

        if schema.types.contains_key(&entity.kind) {
            report.push(
                VerdictStatus::Satisfied,
                "entity.type",
                &entity.id,
                format!("entity type '{}' is declared by schema", entity.kind),
                vec![
                    Witness::Schema {
                        path: format!("schema.yaml:types.{}", entity.kind),
                    },
                    Witness::Declaration {
                        path: entity.document.clone(),
                    },
                ],
            );
        } else {
            report.push(
                VerdictStatus::Violated,
                "entity.unknown_type",
                &entity.id,
                format!("unknown entity type '{}'", entity.kind),
                vec![Witness::Declaration {
                    path: entity.document.clone(),
                }],
            );
        }
    }

    let mut semantic_edges = BTreeSet::new();
    let mut valid_edges: Vec<&Edge> = Vec::new();

    for edge in &graph.edges {
        let subject = assertion_subject(edge);
        let Some(from_entity) = entities.get(edge.from.as_str()) else {
            report.push(
                VerdictStatus::Violated,
                "edge.missing_source",
                &subject,
                format!("source entity '{}' does not exist", edge.from),
                assertion_witness(edge),
            );
            continue;
        };
        let Some(to_entity) = entities.get(edge.to.as_str()) else {
            report.push(
                VerdictStatus::Violated,
                "edge.missing_target",
                &subject,
                format!("target entity '{}' does not exist", edge.to),
                assertion_witness(edge),
            );
            continue;
        };
        let Some(relation) = schema.relations.get(&edge.relation) else {
            report.push(
                VerdictStatus::Violated,
                "edge.unknown_relation",
                &subject,
                format!("relation '{}' is not declared by schema", edge.relation),
                assertion_witness(edge),
            );
            continue;
        };

        let key = semantic_key(edge, relation.symmetric);
        if !semantic_edges.insert(key) {
            report.push(
                VerdictStatus::Violated,
                "edge.duplicate_assertion",
                &subject,
                "the same semantic assertion is declared more than once",
                assertion_witness(edge),
            );
            continue;
        }

        let mut structural_ok = true;
        if !accepts(&relation.from, &from_entity.kind) {
            structural_ok = false;
            report.push(
                VerdictStatus::Violated,
                "edge.domain_mismatch",
                &subject,
                format!(
                    "relation '{}' does not accept source type '{}'",
                    edge.relation, from_entity.kind
                ),
                vec![
                    Witness::Schema {
                        path: format!("schema.yaml:relations.{}.from", edge.relation),
                    },
                    Witness::Declaration {
                        path: edge.declaration.clone(),
                    },
                    Witness::Assertion {
                        from: edge.from.clone(),
                        relation: edge.relation.clone(),
                        to: edge.to.clone(),
                    },
                ],
            );
        }
        if !accepts(&relation.to, &to_entity.kind) {
            structural_ok = false;
            report.push(
                VerdictStatus::Violated,
                "edge.range_mismatch",
                &subject,
                format!(
                    "relation '{}' does not accept target type '{}'",
                    edge.relation, to_entity.kind
                ),
                vec![
                    Witness::Schema {
                        path: format!("schema.yaml:relations.{}.to", edge.relation),
                    },
                    Witness::Declaration {
                        path: edge.declaration.clone(),
                    },
                    Witness::Assertion {
                        from: edge.from.clone(),
                        relation: edge.relation.clone(),
                        to: edge.to.clone(),
                    },
                ],
            );
        }

        if structural_ok {
            report.push(
                VerdictStatus::Satisfied,
                "edge.structure",
                &subject,
                "typed relation is structurally valid",
                vec![
                    Witness::Schema {
                        path: format!("schema.yaml:relations.{}", edge.relation),
                    },
                    Witness::Declaration {
                        path: edge.declaration.clone(),
                    },
                    Witness::Assertion {
                        from: edge.from.clone(),
                        relation: edge.relation.clone(),
                        to: edge.to.clone(),
                    },
                ],
            );
            valid_edges.push(edge);
        }

        validate_edge_evidence(workspace, edge, &subject, &mut report);
    }

    validate_cardinality(schema, graph, &valid_edges, &mut report);
    validate_cycles(schema, &valid_edges, &mut report);
    report
}

fn validate_schema(schema: &Schema, report: &mut ValidationReport) {
    if schema.version == crate::domain::SCHEMA_VERSION {
        report.push(
            VerdictStatus::Satisfied,
            "schema.version",
            "schema",
            format!("schema version {} is supported", schema.version),
            vec![Witness::Schema {
                path: "schema.yaml:version".into(),
            }],
        );
    } else {
        report.push(
            VerdictStatus::Violated,
            "schema.version",
            "schema",
            format!(
                "unsupported schema version {}; expected {}",
                schema.version,
                crate::domain::SCHEMA_VERSION
            ),
            vec![Witness::Schema {
                path: "schema.yaml:version".into(),
            }],
        );
    }

    for type_name in schema.types.keys() {
        if !valid_key(type_name) {
            report.push(
                VerdictStatus::Violated,
                "type.invalid_name",
                format!("type:{type_name}"),
                "type name must contain only ASCII letters, digits, '.', '_', ':', or '-'",
                vec![Witness::Schema {
                    path: format!("schema.yaml:types.{type_name}"),
                }],
            );
        }
    }

    for (name, relation) in &schema.relations {
        let subject = format!("relation:{name}");
        let mut ok = true;

        if !valid_key(name) {
            ok = false;
            report.push(
                VerdictStatus::Violated,
                "relation.invalid_name",
                &subject,
                "relation name must contain only ASCII letters, digits, '.', '_', ':', or '-'",
                vec![Witness::Schema {
                    path: format!("schema.yaml:relations.{name}"),
                }],
            );
        }

        if relation.from.is_empty() {
            ok = false;
            report.push(
                VerdictStatus::Violated,
                "relation.missing_domain",
                &subject,
                "relation must declare from; use ['*'] for any source type",
                vec![Witness::Schema {
                    path: format!("schema.yaml:relations.{name}.from"),
                }],
            );
        }
        if relation.to.is_empty() {
            ok = false;
            report.push(
                VerdictStatus::Violated,
                "relation.missing_range",
                &subject,
                "relation must declare to; use ['*'] for any target type",
                vec![Witness::Schema {
                    path: format!("schema.yaml:relations.{name}.to"),
                }],
            );
        }
        if relation.symmetric && relation.acyclic {
            ok = false;
            report.push(
                VerdictStatus::Violated,
                "relation.invalid_policy",
                &subject,
                "a symmetric relation cannot also be acyclic",
                vec![Witness::Schema {
                    path: format!("schema.yaml:relations.{name}"),
                }],
            );
        }

        for type_name in relation.from.iter().chain(relation.to.iter()) {
            if type_name != "*" && !schema.types.contains_key(type_name) {
                ok = false;
                report.push(
                    VerdictStatus::Violated,
                    "relation.unknown_type",
                    &subject,
                    format!("relation references unknown type '{type_name}'"),
                    vec![Witness::Schema {
                        path: format!("schema.yaml:relations.{name}"),
                    }],
                );
            }
        }

        if ok {
            report.push(
                VerdictStatus::Satisfied,
                "relation.contract",
                &subject,
                "relation contract is valid",
                vec![Witness::Schema {
                    path: format!("schema.yaml:relations.{name}"),
                }],
            );
        }
    }

    for (type_name, type_spec) in &schema.types {
        for (relation_name, cardinality) in &type_spec.constraints {
            let subject = format!("shape:{type_name}.{relation_name}");
            let path = format!("schema.yaml:types.{type_name}.constraints.{relation_name}");
            let Some(relation) = schema.relations.get(relation_name) else {
                report.push(
                    VerdictStatus::Violated,
                    "shape.unknown_relation",
                    &subject,
                    format!("constraint references unknown relation '{relation_name}'"),
                    vec![Witness::Schema { path }],
                );
                continue;
            };

            if cardinality.min.is_none() && cardinality.max.is_none() {
                report.push(
                    VerdictStatus::Violated,
                    "shape.empty",
                    &subject,
                    "constraint must declare min and/or max",
                    vec![Witness::Schema { path }],
                );
                continue;
            }
            if cardinality
                .min
                .zip(cardinality.max)
                .is_some_and(|(min, max)| min > max)
            {
                report.push(
                    VerdictStatus::Violated,
                    "shape.invalid_range",
                    &subject,
                    "constraint min must not exceed max",
                    vec![Witness::Schema { path }],
                );
                continue;
            }
            if !accepts(&relation.from, type_name) {
                report.push(
                    VerdictStatus::Violated,
                    "shape.invalid_domain",
                    &subject,
                    format!(
                        "type '{type_name}' cannot be a source of relation '{relation_name}'"
                    ),
                    vec![Witness::Schema { path }],
                );
                continue;
            }

            report.push(
                VerdictStatus::Satisfied,
                "shape.contract",
                &subject,
                "cardinality constraint is valid",
                vec![Witness::Schema { path }],
            );
        }
    }
}

fn validate_edge_evidence(
    workspace: &Path,
    edge: &Edge,
    subject: &str,
    report: &mut ValidationReport,
) {
    if edge.evidence.is_empty() {
        report.push(
            VerdictStatus::Unknown,
            "edge.evidence",
            subject,
            "relation is declared but has no evidence selector",
            assertion_witness(edge),
        );
        return;
    }

    for selector in &edge.evidence {
        let resolved = resolve_evidence(workspace, &edge.declaration, selector);
        let witness = vec![
            Witness::Declaration {
                path: edge.declaration.clone(),
            },
            Witness::Assertion {
                from: edge.from.clone(),
                relation: edge.relation.clone(),
                to: edge.to.clone(),
            },
            Witness::Evidence {
                source: resolved.source.clone(),
                state: resolved.state,
                exact: resolved.exact.clone(),
                start_line: resolved.start_line,
                end_line: resolved.end_line,
            },
        ];

        match resolved.state {
            EvidenceState::Valid => report.push(
                VerdictStatus::Satisfied,
                "evidence.resolved",
                subject,
                "evidence quote resolves uniquely",
                witness,
            ),
            EvidenceState::Relocated => report.push(
                VerdictStatus::Satisfied,
                "evidence.relocated",
                subject,
                resolved
                    .message
                    .unwrap_or_else(|| "evidence quote relocated successfully".into()),
                witness,
            ),
            EvidenceState::Stale | EvidenceState::Ambiguous | EvidenceState::Missing => {
                report.push(
                    VerdictStatus::Unknown,
                    "evidence.unresolved",
                    subject,
                    resolved
                        .message
                        .unwrap_or_else(|| "evidence cannot be resolved uniquely".into()),
                    witness,
                )
            }
            EvidenceState::Invalid => report.push(
                VerdictStatus::Violated,
                "evidence.invalid",
                subject,
                resolved
                    .message
                    .unwrap_or_else(|| "evidence selector is invalid".into()),
                witness,
            ),
        }
    }
}

fn validate_cardinality(
    schema: &Schema,
    graph: &GraphSnapshot,
    valid_edges: &[&Edge],
    report: &mut ValidationReport,
) {
    for entity in &graph.entities {
        let Some(type_spec) = schema.types.get(&entity.kind) else {
            continue;
        };

        for (relation_name, cardinality) in &type_spec.constraints {
            if !schema.relations.contains_key(relation_name) {
                continue;
            }

            let matches: Vec<&Edge> = valid_edges
                .iter()
                .copied()
                .filter(|edge| {
                    edge.from == entity.id
                        && edge.relation.as_str() == relation_name.as_str()
                })
                .collect();
            let count = matches.len();
            let too_few = cardinality.min.is_some_and(|min| count < min);
            let too_many = cardinality.max.is_some_and(|max| count > max);
            let subject = format!("{}:{}", entity.id, relation_name);
            let mut witness = vec![Witness::Schema {
                path: format!(
                    "schema.yaml:types.{}.constraints.{}",
                    entity.kind, relation_name
                ),
            }];
            witness.extend(matches.iter().map(|edge| Witness::Assertion {
                from: edge.from.clone(),
                relation: edge.relation.clone(),
                to: edge.to.clone(),
            }));

            if too_few || too_many {
                let expected = match (cardinality.min, cardinality.max) {
                    (Some(min), Some(max)) => format!("{min}..={max}"),
                    (Some(min), None) => format!(">={min}"),
                    (None, Some(max)) => format!("<={max}"),
                    (None, None) => unreachable!(),
                };
                report.push(
                    VerdictStatus::Violated,
                    "shape.cardinality",
                    subject,
                    format!(
                        "relation '{}' count is {}; expected {}",
                        relation_name, count, expected
                    ),
                    witness,
                );
            } else {
                report.push(
                    VerdictStatus::Satisfied,
                    "shape.cardinality",
                    subject,
                    format!("relation '{}' count {} satisfies shape", relation_name, count),
                    witness,
                );
            }
        }
    }
}

fn validate_cycles(schema: &Schema, valid_edges: &[&Edge], report: &mut ValidationReport) {
    for (relation_name, relation) in &schema.relations {
        if !relation.acyclic {
            continue;
        }

        let adjacency = relation_adjacency(valid_edges, relation_name);
        let subject = format!("relation:{relation_name}");
        if let Some(path) = find_cycle(&adjacency) {
            report.push(
                VerdictStatus::Violated,
                "relation.cycle",
                subject,
                format!("relation '{relation_name}' must be acyclic"),
                vec![
                    Witness::Schema {
                        path: format!("schema.yaml:relations.{relation_name}.acyclic"),
                    },
                    Witness::Cycle { path },
                ],
            );
        } else {
            report.push(
                VerdictStatus::Satisfied,
                "relation.acyclic",
                subject,
                "no cycle found",
                vec![Witness::Schema {
                    path: format!("schema.yaml:relations.{relation_name}.acyclic"),
                }],
            );
        }
    }
}

fn accepts(allowed: &[String], actual: &str) -> bool {
    allowed.iter().any(|value| value == "*" || value == actual)
}

fn semantic_key(edge: &Edge, symmetric: bool) -> String {
    if symmetric && edge.from.as_str() > edge.to.as_str() {
        format!("{}|{}|{}", edge.to, edge.relation, edge.from)
    } else {
        format!("{}|{}|{}", edge.from, edge.relation, edge.to)
    }
}

fn assertion_subject(edge: &Edge) -> String {
    format!("{} --{}--> {}", edge.from, edge.relation, edge.to)
}

fn assertion_witness(edge: &Edge) -> Vec<Witness> {
    vec![
        Witness::Declaration {
            path: edge.declaration.clone(),
        },
        Witness::Assertion {
            from: edge.from.clone(),
            relation: edge.relation.clone(),
            to: edge.to.clone(),
        },
    ]
}

fn relation_adjacency(edges: &[&Edge], relation: &str) -> BTreeMap<String, Vec<String>> {
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for edge in edges.iter().copied().filter(|edge| edge.relation == relation) {
        adjacency
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
        adjacency.entry(edge.to.clone()).or_default();
    }
    for next in adjacency.values_mut() {
        next.sort();
        next.dedup();
    }
    adjacency
}

fn find_cycle(adjacency: &BTreeMap<String, Vec<String>>) -> Option<Vec<String>> {
    let mut state = BTreeMap::<String, u8>::new();
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

    if let Some(next) = adjacency.get(node) {
        for target in next {
            match state.get(target).copied().unwrap_or(0) {
                0 => {
                    if let Some(cycle) = dfs_cycle(target, adjacency, state, stack) {
                        return Some(cycle);
                    }
                }
                1 => {
                    let start = stack.iter().position(|value| value == target).unwrap_or(0);
                    let mut cycle = stack[start..].to_vec();
                    cycle.push(target.clone());
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


fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}
