use crate::domain::{
    Connection, CtxError, Entity, Neighborhood, Ontology, PathResult, Result, Topology,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub fn neighborhood(
    topology: &Topology,
    root: &str,
    depth: usize,
) -> Result<Neighborhood> {
    let entities = entity_map(topology);
    if !entities.contains_key(root) {
        return Err(CtxError(format!("unknown entity '{root}'")));
    }

    let mut distance: BTreeMap<String, usize> = BTreeMap::new();
    let mut queue = VecDeque::new();
    distance.insert(root.to_string(), 0);
    queue.push_back(root.to_string());

    while let Some(current) = queue.pop_front() {
        let current_depth = distance[&current];
        if current_depth >= depth {
            continue;
        }
        for next in adjacent_all(topology, &current) {
            if !distance.contains_key(&next) {
                distance.insert(next.clone(), current_depth + 1);
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

    let mut connections: Vec<Connection> = topology
        .edges
        .iter()
        .filter(|edge| included.contains(&edge.from) && included.contains(&edge.to))
        .map(|edge| Connection {
            edge_id: edge.id.clone(),
            from: edge.from.clone(),
            relation: edge.relation.clone(),
            to: edge.to.clone(),
        })
        .collect();
    connections.sort_by(|a, b| a.edge_id.cmp(&b.edge_id));

    Ok(Neighborhood {
        root: root.to_string(),
        depth,
        entities: output_entities,
        connections,
    })
}

pub fn impact(topology: &Topology, root: &str) -> Result<Vec<String>> {
    let entities = entity_map(topology);
    if !entities.contains_key(root) {
        return Err(CtxError(format!("unknown entity '{root}'")));
    }

    let mut seen = BTreeSet::from([root.to_string()]);
    let mut queue = VecDeque::from([root.to_string()]);
    let mut result = Vec::new();

    while let Some(current) = queue.pop_front() {
        let mut incoming: Vec<String> = topology
            .edges
            .iter()
            .filter(|edge| edge.to == current)
            .map(|edge| edge.from.clone())
            .collect();
        incoming.sort();
        incoming.dedup();

        for next in incoming {
            if seen.insert(next.clone()) {
                result.push(next.clone());
                queue.push_back(next);
            }
        }
    }
    Ok(result)
}

pub fn shortest_path(
    ontology: &Ontology,
    topology: &Topology,
    from: &str,
    to: &str,
) -> Result<Option<PathResult>> {
    let entities = entity_map(topology);
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
        for (next, connection) in outgoing(ontology, topology, &current) {
            if seen.insert(next.clone()) {
                previous.insert(next.clone(), (current.clone(), connection));
                if next == to {
                    return Ok(Some(reconstruct_path(from, to, previous)));
                }
                queue.push_back(next);
            }
        }
    }
    Ok(None)
}

fn reconstruct_path(
    from: &str,
    to: &str,
    previous: BTreeMap<String, (String, Connection)>,
) -> PathResult {
    let mut current = to.to_string();
    let mut connections = Vec::new();
    while current != from {
        let (parent, connection) = previous
            .get(&current)
            .expect("path predecessor must exist")
            .clone();
        connections.push(connection);
        current = parent;
    }
    connections.reverse();
    PathResult {
        from: from.to_string(),
        to: to.to_string(),
        connections,
    }
}

fn entity_map(topology: &Topology) -> BTreeMap<&str, &Entity> {
    topology
        .entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity))
        .collect()
}

fn adjacent_all(topology: &Topology, entity: &str) -> Vec<String> {
    let mut next = Vec::new();
    for edge in &topology.edges {
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
    ontology: &Ontology,
    topology: &Topology,
    entity: &str,
) -> Vec<(String, Connection)> {
    let mut result = Vec::new();
    for edge in &topology.edges {
        if edge.from == entity {
            result.push((
                edge.to.clone(),
                Connection {
                    edge_id: edge.id.clone(),
                    from: edge.from.clone(),
                    relation: edge.relation.clone(),
                    to: edge.to.clone(),
                },
            ));
        }
        if edge.to == entity
            && ontology
                .relations
                .get(&edge.relation)
                .is_some_and(|relation| relation.symmetric)
        {
            result.push((
                edge.from.clone(),
                Connection {
                    edge_id: edge.id.clone(),
                    from: edge.to.clone(),
                    relation: edge.relation.clone(),
                    to: edge.from.clone(),
                },
            ));
        }
    }
    result.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.edge_id.cmp(&b.1.edge_id))
    });
    result
}
