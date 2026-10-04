# Architecture

## Goal

Provide the smallest system that can deterministically prove:

- which things are connected,
- whether the declared graph is structurally valid,
- and which exact source text supports each relation.

The graph is not inferred truth. It is a set of typed assertions with evidence.

## Data model

```text
Ontology
  Type
  Relation { from, to, symmetric, acyclic }

Topology
  Entity { id, kind, title, aliases }
  Edge { id, from, relation, to, evidence[] }

Evidence
  { relative path, line range, optional SHA-256 digest }
```

The ontology is the contract. The topology is the asserted instance graph. Source files are proof material.

## Authority

```text
source files -------------+
ontology.yaml ------------+-> load -> validate -> graph query -> output
 topology.yaml -----------+
```

There is no hidden mutable state.

Derived caches may exist later, but deleting them must never change meaning.

## Validation states

A relation is structurally valid only when:

1. the ontology explicitly declares `from` and `to` (`["*"]` means any type),
2. both endpoints exist,
3. endpoint types satisfy the relation domain/range,
4. relation-level constraints hold,
5. every evidence pointer resolves inside the workspace,
6. cited line ranges exist,
7. pinned evidence still matches its digest.

Evidence states:

- `valid` -- source slice matches the pinned digest.
- `unpinned` -- source exists, but no historical integrity claim was made.
- `stale` -- source slice no longer matches the pinned digest.
- `missing` -- source file does not exist.
- `invalid` -- path or range is invalid.

`unpinned` is a warning. An edge with no evidence is also a warning. `stale`, `missing`, and `invalid` are errors.

## Topology semantics

Direction is meaningful.

```text
A --requires--> B
```

means A depends on B. `impact B` therefore follows incoming edges to A and then further incoming edges.

A relation may be declared symmetric. Symmetry changes traversal semantics; it does not require a duplicated reverse edge.

Only relations explicitly declared `acyclic: true` are checked for cycles. Cycles are not globally wrong: review/revision loops, social relations and conceptual associations may legitimately contain cycles.

## Determinism

All maps used for validation and traversal are ordered. Ambiguous graph results are emitted in stable lexical order. No clock, random source or model participates in validation.

## Security boundary

Evidence paths must be relative and resolve inside the canonical workspace root. Absolute paths and parent traversal are rejected. Symlinks that escape the workspace are rejected after canonicalization.

## Deliberately absent

### Database

Not needed for current authority or graph size assumptions. A future DB must be disposable and reconstructable from canonical files.

### Full Markdown AST

The current evidence contract is line-range based and language-neutral. AST parsing becomes justified only if semantic anchors such as "section X / list item Y" are required after edits.

### Search index

Entity IDs and explicit graph traversal need no retrieval index. Search is a separate candidate-generation problem and must not be mixed with graph truth.

### Decision model

A Jev-family model can reduce semantic-search cost, but it must never decide graph validity, source freshness, approval or truth. Model output is a proposal until grounded in an evidence pointer and accepted into topology.

## Extension rule

A new mechanism is accepted only if a frozen evaluation shows improvement on one of:

- evidence recall,
- exact-anchor precision,
- unsupported-assertion rate,
- p95 latency,
- memory,
- or local inference cost.

No technology is included because it is fashionable or because the old repository already had it.
