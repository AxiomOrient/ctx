# ctx

`ctx` is a small deterministic evidence graph for local ontologies and topologies.

It answers three questions without a model:

1. **What is connected?** -- typed entities and relations.
2. **What is wrong?** -- schema, reference, cycle and evidence validation with witnesses.
3. **Where is the proof?** -- every asserted relation can point to an exact source slice and pinned digest.

This is a clean break from the previous context-composer implementation. There is no compatibility layer, UI, HTTP server, prompt pipeline, MMR, legacy schema, or database.

Copyright (c) Axient Inc. All rights reserved.

## Canonical data

A workspace has only three authoritative inputs:

```text
workspace/
|-- ontology.yaml   # type and relation contract
|-- topology.yaml   # concrete entities, relations, evidence pointers
`-- ...             # source files referenced by evidence
```

`ontology.yaml` defines **what may exist**. `topology.yaml` defines **what does exist**. Source files remain the evidence.

No generated index is authoritative.

## Commands

```bash
ctx --workspace ./workspace check
ctx --workspace ./workspace pin
ctx --workspace ./workspace explain step.release --depth 2
ctx --workspace ./workspace impact artifact.privacy
ctx --workspace ./workspace path step.release requirement.reviewed
```

Add `--json` for machine-readable output.

### `check`

Validates:

- schema version
- entity type existence
- relation existence
- relation domain/range
- duplicate IDs and duplicate assertions
- acyclic relation cycles with a concrete witness path
- evidence path containment
- evidence line ranges
- evidence digest freshness

Missing information is never converted into success. Unpinned evidence is a warning; stale or invalid evidence is an error.

### `pin`

Computes SHA-256 over each cited line slice and writes the digest into `topology.yaml`. `pin` rewrites that file in canonical YAML form.

A digest covers the cited evidence, not the whole file. Unrelated edits elsewhere do not invalidate a relation. If lines move or cited text changes, validation reports the evidence as stale.

### `explain`

Shows nearby entities and relations plus exact evidence snippets. The explanation is reconstructed from files every time; no model output is treated as truth.

### `impact`

Walks incoming relations transitively to answer "what can be affected if this entity changes?"

### `path`

Finds a directed relation path. Relations declared `symmetric: true` are traversable both ways.

## Why there is no DB, vector index, tokenizer, AST or local model in core

They are not currently required to satisfy the correctness contract. Adding them now would create extra state and failure modes without improving authoritative answers.

They may be added only as derived acceleration or semantic-discovery layers after an evaluation proves a real benefit:

- SQLite/FTS/BM25: when filesystem scanning or lexical candidate recall becomes a measured bottleneck.
- Vector search: when lexical + graph candidate generation misses required evidence.
- Markdown AST: when exact block-level anchors require syntax that the current line evidence contract cannot express.
- PageIndex: when long documents exceed the practical decision-model context and hierarchical navigation improves evidence recall.
- Jev-family model: for semantic candidate selection or routing only. It may propose evidence; `ctx check` remains the authority.

See [`docs/MODEL_SELECTION.md`](docs/MODEL_SELECTION.md).

## Example

```bash
cd examples
cargo run --manifest-path ../Cargo.toml -- check
cargo run --manifest-path ../Cargo.toml -- explain step.release --depth 2
```

The second edge in `examples/topology.yaml` is intentionally unpinned so `check` demonstrates the non-fatal warning. Run `pin` to make the example fully pinned.

## Development

Required local verification:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- --workspace examples check
cargo run -- --workspace examples explain step.release --depth 2
```

There is intentionally no CI configuration in this repository.
