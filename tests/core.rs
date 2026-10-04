use ctx::{
    EvidenceState, QueryMatchKind, VerdictStatus, compile_workspace, query_entities,
    resolve_evidence, shortest_path, validate_workspace,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ctx-test-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("knowledge")).unwrap();
        Self { path }
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn schema() -> &'static str {
    r#"version: 1
types:
  Step:
    constraints:
      requires:
        min: 1
  Artifact: {}
relations:
  requires:
    from: [Step]
    to: [Artifact]
    acyclic: true
"#
}

fn privacy() -> &'static str {
    r#"---
id: artifact.privacy
type: Artifact
title: Privacy
---
# Privacy
"#
}

fn release(exact: &str, body: &str, hint: Option<(usize, usize)>) -> String {
    let hint_yaml = hint
        .map(|(start, end)| format!("        hint:\n          start: {start}\n          end: {end}\n"))
        .unwrap_or_default();

    format!(
        "---\nid: step.release\ntype: Step\ntitle: Release\naliases: [publish]\nrelations:\n  - relation: requires\n    target: artifact.privacy\n    evidence:\n      - exact: \"{exact}\"\n{hint_yaml}---\n# Release\n\n{body}\n"
    )
}

fn compile(root: &Path) -> ctx::Workspace {
    compile_workspace(root, Path::new("schema.yaml"), Path::new("knowledge")).unwrap()
}

#[test]
fn markdown_is_the_single_graph_source() {
    let temp = TempWorkspace::new();
    temp.write("schema.yaml", schema());
    temp.write(
        "knowledge/release.md",
        &release(
            "Publishing requires privacy.",
            "Publishing requires privacy.",
            None,
        ),
    );
    temp.write("knowledge/privacy.md", privacy());

    let workspace = compile(&temp.path);
    assert_eq!(workspace.graph.entities.len(), 2);
    assert_eq!(workspace.graph.edges.len(), 1);
    assert!(!temp.path.join("topology.yaml").exists());

    let report = validate_workspace(&workspace.root, &workspace.schema, &workspace.graph);
    assert!(report.is_complete());
}

#[test]
fn moved_quote_remains_satisfied() {
    let temp = TempWorkspace::new();
    temp.write("schema.yaml", schema());
    temp.write(
        "knowledge/release.md",
        &release(
            "Publishing requires privacy.",
            "Intro\n\nPublishing requires privacy.",
            Some((1, 1)),
        ),
    );
    temp.write("knowledge/privacy.md", privacy());

    let workspace = compile(&temp.path);
    let edge = &workspace.graph.edges[0];
    let evidence = resolve_evidence(&workspace.root, &edge.declaration, &edge.evidence[0]);
    assert_eq!(evidence.state, EvidenceState::Relocated);

    let report = validate_workspace(&workspace.root, &workspace.schema, &workspace.graph);
    assert!(report.is_complete());
    assert!(report.verdicts.iter().any(|verdict| {
        verdict.status == VerdictStatus::Satisfied && verdict.code == "evidence.relocated"
    }));
}

#[test]
fn stale_quote_is_unknown_not_false() {
    let temp = TempWorkspace::new();
    temp.write("schema.yaml", schema());
    temp.write(
        "knowledge/release.md",
        &release("Old evidence.", "New evidence.", None),
    );
    temp.write("knowledge/privacy.md", privacy());

    let workspace = compile(&temp.path);
    let report = validate_workspace(&workspace.root, &workspace.schema, &workspace.graph);

    assert_eq!(report.violated_count(), 0);
    assert!(report.unknown_count() >= 1);
    assert!(report.verdicts.iter().any(|verdict| {
        verdict.status == VerdictStatus::Unknown && verdict.code == "evidence.unresolved"
    }));
}

#[test]
fn missing_required_relation_is_a_shape_violation() {
    let temp = TempWorkspace::new();
    temp.write("schema.yaml", schema());
    temp.write(
        "knowledge/release.md",
        "---\nid: step.release\ntype: Step\n---\n# Release\n",
    );
    temp.write("knowledge/privacy.md", privacy());

    let workspace = compile(&temp.path);
    let report = validate_workspace(&workspace.root, &workspace.schema, &workspace.graph);

    assert!(report.verdicts.iter().any(|verdict| {
        verdict.status == VerdictStatus::Violated && verdict.code == "shape.cardinality"
    }));
}

#[test]
fn deterministic_query_and_path_use_the_compiled_graph() {
    let temp = TempWorkspace::new();
    temp.write("schema.yaml", schema());
    temp.write(
        "knowledge/release.md",
        &release(
            "Publishing requires privacy.",
            "Publishing requires privacy.",
            None,
        ),
    );
    temp.write("knowledge/privacy.md", privacy());

    let workspace = compile(&temp.path);
    let matches = query_entities(&workspace.graph, "publish");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].kind, QueryMatchKind::Alias);
    assert_eq!(matches[0].entity.id, "step.release");

    let path = shortest_path(
        &workspace.schema,
        &workspace.graph,
        "step.release",
        "artifact.privacy",
    )
    .unwrap()
    .unwrap();
    assert_eq!(path.connections.len(), 1);
}


#[test]
fn declared_context_change_makes_evidence_unknown() {
    let temp = TempWorkspace::new();
    temp.write("schema.yaml", schema());
    temp.write(
        "knowledge/release.md",
        r#"---
id: step.release
type: Step
relations:
  - relation: requires
    target: artifact.privacy
    evidence:
      - exact: "Publishing requires privacy."
        prefix: "Old prefix "
        suffix: " old suffix"
---
# Release

Publishing requires privacy.
"#,
    );
    temp.write("knowledge/privacy.md", privacy());

    let workspace = compile(&temp.path);
    let edge = &workspace.graph.edges[0];
    let evidence = resolve_evidence(&workspace.root, &edge.declaration, &edge.evidence[0]);
    assert_eq!(evidence.state, EvidenceState::Stale);

    let report = validate_workspace(&workspace.root, &workspace.schema, &workspace.graph);
    assert!(report.verdicts.iter().any(|verdict| {
        verdict.status == VerdictStatus::Unknown && verdict.code == "evidence.unresolved"
    }));
}

#[test]
fn query_normalizes_korean_unicode() {
    let temp = TempWorkspace::new();
    temp.write("schema.yaml", schema());
    temp.write(
        "knowledge/release.md",
        &release(
            "Publishing requires privacy.",
            "Publishing requires privacy.",
            None,
        ),
    );
    temp.write("knowledge/privacy.md", privacy());

    let mut workspace = compile(&temp.path);
    workspace.graph.entities[0].aliases.push("가".into());

    let decomposed = "\u{1100}\u{1161}";
    let matches = query_entities(&workspace.graph, decomposed);
    assert!(matches.iter().any(|item| {
        item.kind == QueryMatchKind::Alias && item.entity.aliases.iter().any(|alias| alias == "가")
    }));
}
