use ctx::pipeline::Pipeline;
use ctx::domain::types::ComposeInput;
use std::fs;
use std::path::PathBuf;

fn write_doc(dir: &std::path::Path, name: &str, content: &str) {
    let p = dir.join(name);
    fs::write(&p, content).expect("write md");
}

#[test]
fn compose_section_budget_and_determinism() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    // Two documents with sections
    let doc1 = r###"---
id: "DOC-1"
title: "Prompt Guidelines"
version: "1.0.0"
schema: "context.v1"
type: "guide"
sections:
  - id: "system"
    name: "System"
    marker: "## System"
    priority: 100
  - id: "constraints"
    name: "Constraints"
    marker: "## Constraints"
    priority: 90
---

## System
Follow these rules strictly.

## Constraints
Stay within budget.
"###;

    let doc2 = r###"---
id: "DOC-2"
title: "Rust Backend Tips"
version: "1.0.0"
schema: "context.v1"
type: "guide"
sections:
  - id: "system"
    name: "System"
    marker: "## System"
    priority: 100
---

## System
Use Axum and Tokio.
"###;

    write_doc(dir, "a.md", doc1);
    write_doc(dir, "b.md", doc2);

    // Build pipeline pointed at temp docs dir
    let mut pipeline = Pipeline::new().with_docs_root(PathBuf::from(dir));

    // Compose only the "system" part
    let input = ComposeInput {
        query: "parts: system".to_string(),
        tags: None,
        max_tokens: Some(200),
        exclude_sources: None,
        priority_sources: None,
    };

    let out1 = pipeline.execute(input.clone()).expect("compose");
    let out2 = pipeline.execute(input).expect("compose2");

    // Determinism: identical outputs
    assert_eq!(out1.prompt, out2.prompt);
    assert!(out1.tokens <= 200);
    assert!(out1.context.contains("### Prompt Guidelines — System"));
    assert!(out1.context.contains("### Rust Backend Tips — System"));
}
