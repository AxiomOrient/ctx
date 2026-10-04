use crate::domain::{
    Connection, CtxError, Entity, GraphSnapshot, Neighborhood, PathResult, QueryMatch,
    QueryMatchKind, RelationSpec, Result, Schema,
};
use unicode_normalization::UnicodeNormalization;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub fn neighborhood(graph: &GraphSnapshot, root: &str, depth: usize) -> Result<Neighborhood> {
    let entities = entity_map(graph);
    if !entities.contains_key(root) {
        return Err(CtxError(format!("unknown entity '{root}'")));
    }

    let mut distance = BTreeMap::new();
    let mut queue = VecDeque::new();
    distance.insert(root.to_string(), 0usize);
    queue.push_back(root.to_string());

    while let Some(current) = queue.pop_front() {
        let current_depth = distance[&current];
        if current_depth >= depth {
            continue;
        }
        for next in adjacent_all(graph, &current) {
            if let std::collections::btree_map::Entry::Vacant(entry) = distance.entry(next.clone()) {
                entry.insert(current_depth + 1);
                queue.push_back(next);
            }
        }
    }

    let included: BTreeSet<String> = distance.keys().cloned().collect();
    let mut output_entities: Vec<Entity> = included
        .iter()
        .filter_map(|id| entities.get(id.as_str()).map(|entity| (*entity).clone()))
        .collect();
    output_entities.sort_by(|a, b| a.id.cmp(&b.id));

    let mut connections: Vec<Connection> = graph
        .edges
        .iter()
        .filter(|edge| included.contains(&edge.from) && included.contains(&edge.to))
        .map(connection)
        .collect();
    connections.sort_by(|a, b| {
        a.from
            .cmp(&b.from)
            .then_with(|| a.relation.cmp(&b.relation))
            .then_with(|| a.to.cmp(&b.to))
            .then_with(|| a.declaration.cmp(&b.declaration))
    });

    Ok(Neighborhood {
        root: root.to_string(),
        depth,
        entities: output_entities,
        connections,
    })
}

pub fn shortest_path(
    schema: &Schema,
    graph: &GraphSnapshot,
    from: &str,
    to: &str,
) -> Result<Option<PathResult>> {
    let entities = entity_map(graph);
    if !entities.contains_key(from) {
        return Err(CtxError(format!("unknown entity '{from}'")));
    }
    if !entities.contains_key(to) {
        return Err(CtxError(format!("unknown entity '{to}'")));
    }
    if from == to {
        return Ok(Some(PathResult {
            from: from.to_string(),
            to: to.to_string(),
            connections: Vec::new(),
        }));
    }

    let mut queue = VecDeque::from([from.to_string()]);
    let mut seen = BTreeSet::from([from.to_string()]);
    let mut previous: BTreeMap<String, (String, Connection)> = BTreeMap::new();

    while let Some(current) = queue.pop_front() {
        for (next, edge) in outgoing(schema, graph, &current) {
            if seen.insert(next.clone()) {
                previous.insert(next.clone(), (current.clone(), edge));
                if next == to {
                    return Ok(Some(reconstruct_path(from, to, previous)));
                }
                queue.push_back(next);
            }
        }
    }
    Ok(None)
}

pub fn query_entities(graph: &GraphSnapshot, text: &str) -> Vec<QueryMatch> {
    let needle = normalize_key(text);
    if needle.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    for entity in &graph.entities {
        let id = normalize_key(&entity.id);
        let title = normalize_key(&entity.title);
        let aliases: Vec<String> = entity.aliases.iter().map(|value| normalize_key(value)).collect();

        let kind = if id == needle {
            QueryMatchKind::Id
        } else if title == needle {
            QueryMatchKind::Title
        } else if aliases.iter().any(|value| value == &needle) {
            QueryMatchKind::Alias
        } else if id.contains(&needle)
            || title.contains(&needle)
            || aliases.iter().any(|value| value.contains(&needle))
        {
            QueryMatchKind::Contains
        } else {
            continue;
        };

        matches.push(QueryMatch {
            kind,
            entity: entity.clone(),
        });
    }

    matches.sort_by(|a, b| {
        query_rank(a.kind)
            .cmp(&query_rank(b.kind))
            .then_with(|| a.entity.id.cmp(&b.entity.id))
    });
    matches
}

fn normalize_key(value: &str) -> String {
    value.nfc().collect::<String>().to_lowercase()
}

fn query_rank(kind: QueryMatchKind) -> u8 {
    match kind {
        QueryMatchKind::Id => 0,
        QueryMatchKind::Title => 1,
        QueryMatchKind::Alias => 2,
        QueryMatchKind::Contains => 3,
    }
}

fn reconstruct_path(
    from: &str,
    to: &str,
    previous: BTreeMap<String, (String, Connection)>,
) -> PathResult {
    let mut current = to.to_string();
    let mut connections = Vec::new();
    while current != from {
        let (parent, edge) = previous
            .get(&current)
            .expect("path predecessor must exist")
            .clone();
        connections.push(edge);
        current = parent;
    }
    connections.reverse();
    PathResult {
        from: from.to_string(),
        to: to.to_string(),
        connections,
    }
}

fn entity_map(graph: &GraphSnapshot) -> BTreeMap<&str, &Entity> {
    graph
        .entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity))
        .collect()
}

fn adjacent_all(graph: &GraphSnapshot, entity: &str) -> Vec<String> {
    let mut next = Vec::new();
    for edge in &graph.edges {
        if edge.from == entity {
            next.push(edge.to.clone());
        }
        if edge.to == entity {
            next.push(edge.from.clone());
        }
    }
    next.sort();
    next.dedup();
    next
}

fn outgoing(
    schema: &Schema,
    graph: &GraphSnapshot,
    entity: &str,
) -> Vec<(String, Connection)> {
    let mut result = Vec::new();
    for edge in &graph.edges {
        if edge.from == entity {
            result.push((edge.to.clone(), connection(edge)));
        }
        if edge.to == entity && is_symmetric(schema.relations.get(&edge.relation)) {
            result.push((
                edge.from.clone(),
                Connection {
                    from: edge.to.clone(),
                    relation: edge.relation.clone(),
                    to: edge.from.clone(),
                    declaration: edge.declaration.clone(),
                },
            ));
        }
    }
    result.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.relation.cmp(&b.1.relation))
            .then_with(|| a.1.declaration.cmp(&b.1.declaration))
    });
    result
}

fn is_symmetric(relation: Option<&RelationSpec>) -> bool {
    relation.is_some_and(|value| value.symmetric)
}

fn connection(edge: &crate::domain::Edge) -> Connection {
    Connection {
        from: edge.from.clone(),
        relation: edge.relation.clone(),
        to: edge.to.clone(),
        declaration: edge.declaration.clone(),
    }
}
