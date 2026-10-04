use ctx::{
    Edge, Entity, Ontology, RelationSpec, Topology, TypeSpec, shortest_path, validate_workspace,
};
use std::collections::BTreeMap;

fn fixture() -> (Ontology, Topology) {
    let ontology = Ontology {
        version: 1,
        types: BTreeMap::from([
            ("Step".into(), TypeSpec::default()),
            ("Artifact".into(), TypeSpec::default()),
        ]),
        relations: BTreeMap::from([(
            "requires".into(),
            RelationSpec {
                from: vec!["Step".into()],
                to: vec!["Artifact".into()],
                acyclic: true,
                ..RelationSpec::default()
            },
        )]),
    };

    let topology = Topology {
        version: 1,
        entities: vec![
            Entity {
                id: "release".into(),
                kind: "Step".into(),
                title: "Release".into(),
                aliases: Vec::new(),
            },
            Entity {
                id: "privacy".into(),
                kind: "Artifact".into(),
                title: "Privacy".into(),
                aliases: Vec::new(),
            },
        ],
        edges: vec![Edge {
            id: "release-privacy".into(),
            from: "release".into(),
            relation: "requires".into(),
            to: "privacy".into(),
            evidence: Vec::new(),
        }],
    };
    (ontology, topology)
}

#[test]
fn path_follows_relation_direction() {
    let (ontology, topology) = fixture();
    let forward = shortest_path(&ontology, &topology, "release", "privacy")
        .unwrap()
        .unwrap();
    assert_eq!(forward.connections.len(), 1);
    assert!(shortest_path(&ontology, &topology, "privacy", "release")
        .unwrap()
        .is_none());
}

#[test]
fn wrong_relation_range_is_an_error() {
    let (ontology, mut topology) = fixture();
    topology.entities[1].kind = "Step".into();
    let report = validate_workspace(std::path::Path::new("."), &ontology, &topology);
    assert!(report.error_count() >= 1);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "edge.range_mismatch"));
}
