# ctx

A minimal local knowledge graph that can explain itself without trusting a model.

It answers three questions deterministically:

1. **What is connected?**
2. **What is violated or unknown?**
3. **Where is the supporting source text?**

## Canonical workspace

```text
workspace/
├── schema.yaml
└── knowledge/
    ├── release.md
    ├── privacy.md
    └── ...
```

There is no authoritative topology file or database. The graph is compiled from Markdown frontmatter on every load.

```text
schema.yaml + knowledge/**/*.md
              │
              ▼
        immutable graph
              │
              ▼
     deterministic validation
              │
              ▼
 Satisfied / Violated / Unknown
```

## Schema

Only the constraints needed by the core are supported:

- relation domain/range
- `min` / `max`
- `acyclic`
- `symmetric`

```yaml
version: 1

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
```

IDs, type names and relation names are stable machine keys: ASCII letters, digits, `.`, `_`, `:`, and `-`. Human-facing titles and aliases may use any Unicode text.

## Knowledge document

```markdown
---
id: step.release
type: Step
title: Release
aliases: [publish]

relations:
  - relation: requires
    target: artifact.privacy
    evidence:
      - exact: "Publishing requires the privacy notice to be reviewed."
        prefix: "# Release\n\n"
        suffix: "\nThe checklist remains blocked."
        hint:
          start: 14
          end: 14
---
# Release

Publishing requires the privacy notice to be reviewed.
The checklist remains blocked.
```

A relation is an assertion, not inferred truth.

### Evidence selector

`exact` is authoritative. If `prefix` or `suffix` is declared, that context is part of the selector and must still match. When context is omitted, `exact` must resolve to exactly one occurrence. A line `hint` is optional and never authoritative.

Resolution states:

- **valid** — unique quote found.
- **relocated** — unique quote still exists but moved away from the line hint.
- **stale** — exact quote no longer exists.
- **ambiguous** — multiple exact matches cannot be reduced to exactly one by context.
- **missing** — source file is absent.
- **invalid** — selector, line hint, path, or UTF-8 source violates the contract.

Markdown YAML frontmatter is excluded from evidence search, so an `exact` value cannot match its own declaration.

Text matching and entity lookup normalize Unicode to NFC. This prevents Korean/macOS NFC/NFD differences from becoming false misses.

## Evidence-carrying validation

`check --json` returns every check as one of:

```text
Satisfied + satisfaction trace
Violated  + failure witness
Unknown   + unresolved evidence
```

Missing or stale evidence is not silently converted into false or success.

## CLI

```bash
ctx --workspace ./workspace check
ctx --workspace ./workspace explain step.release --depth 2
ctx --workspace ./workspace path step.release artifact.privacy
ctx --workspace ./workspace query publish
```

### Optional semantic decision backend

The core does not depend on Laya, TinyJev, Kev, AnyJev, ONNX, MLX, PyTorch, or a vendor server.

When deterministic lookup is ambiguous or empty, `query` can delegate candidate selection to any executable:

```bash
ctx --workspace ./workspace query "배포 개인정보 문서" \
  --decider ./my-decision-adapter
```

Optional adapter arguments can be repeated:

```bash
ctx query "..." \
  --decider ./my-adapter \
  --decider-arg model=laya-ko
```

The adapter receives one JSON object on stdin:

```json
{
  "version": 1,
  "task": "select_entity",
  "query": "배포 개인정보 문서",
  "candidates": [
    {
      "id": "artifact.privacy",
      "kind": "Artifact",
      "title": "Privacy notice",
      "aliases": []
    }
  ]
}
```

It must write exactly one JSON object to stdout:

```json
{
  "entity_id": "artifact.privacy",
  "confidence": 0.91
}
```

`entity_id` may be `null`. Confidence is optional, validated to `0..=1`, and is never treated as evidence truth. An adapter cannot select an entity outside the candidate set.

Logs belong on stderr.

This boundary is the model-switching mechanism. No model name, tokenizer, runtime, checkpoint path, or confidence threshold is part of the graph contract. A wrapper may internally use Laya-Ko, TinyJev, Kev, AnyJev, a System One server, or a future local classifier without changing ctx data or public graph semantics.

## Query policy

`query` always uses deterministic lookup first:

1. ID
2. title
3. alias
4. substring

The old numeric heuristic score was removed. Results expose the match kind instead.

A decision adapter is invoked only when deterministic exact lookup does not resolve to one entity. If there is no exact ID/title/alias match, the adapter receives the full entity set; substring matches are display hints and never restrict semantic recall. If several exact matches exist, only that exact ambiguity set is sent.

## Deliberately absent

These are not authoritative core components:

| Mechanism | Add only when |
| --- | --- |
| SQLite / FTS5 / BM25 | corpus scanning or lexical recall is a measured bottleneck |
| vector search | lexical + graph candidate recall fails a frozen evaluation set |
| Markdown AST | quote selectors cannot express required structural anchors |
| PageIndex | long documents require hierarchical evidence navigation |
| model runtime | semantic ambiguity remains after deterministic lookup |

Any future index must be disposable and rebuildable.

## Development

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- --workspace examples check
cargo run -- --workspace examples explain step.release --depth 2
cargo run -- --workspace examples path step.release artifact.privacy
cargo run -- --workspace examples query publish
```

There is intentionally no CI requirement or generated index.
