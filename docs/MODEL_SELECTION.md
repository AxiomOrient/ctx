# Local decision model selection

Research snapshot: **2026-10-04**.

The deterministic core does not require a model. This document selects candidates for a later semantic candidate-selection layer.

## Decision

### Default small candidate: Kev-0.8B v1.0

Use `jaredpalmer/kev-0.8b` as the first benchmark target.

Why:

- Qwen3.5-0.8B base with a typed-decision pointer head; no autoregressive answer generation.
- Apache-2.0 model/base licensing.
- TypeSafe-compatible `/v1/systemone` serving contract.
- Runs on Apple Silicon.
- Kev v1.0 pins the 0.8B release and validates an 8,192-token context.
- The current model card reports 0.851 accuracy on a locked real-document test (936 questions), and 0.697 on its current out-of-domain test row. These are project-reported numbers, not ctx-specific evidence.

Important: the published model card is English-focused. **Korean quality is unverified for ctx and must not be assumed.**

### Ultra-light comparison: TinyJev-0.6B

Benchmark `AnkitAI/TinyJev-0.6B` when memory/latency dominate.

- 596M parameters, about 1.2 GB fp16.
- MLX on Apple Silicon.
- Project-reported 85 ms/case on a base M1 and 88.0% on its OD-500 benchmark.
- MIT licensed.

Its published benchmark is not directly comparable with Kev's test suites, so it is a comparison candidate, not the default on headline accuracy alone.

### Accuracy escalation: Kev-4B

Use only when the small model fails the ctx evaluation set.

Kev's current release reports materially stronger new-source accuracy than 0.8B, at a much larger memory cost. The architecture should allow an endpoint swap without changing graph semantics.

## Why Laya is not the default

Laya remains useful, especially `laya-multilingual` for 100+ languages and the 421M typed-decisions checkpoint for its trained workflows. But current published evidence shows an important distinction:

- `laya-typed-decisions` is strong on the four workflows it was fine-tuned for.
- the general and multilingual checkpoints perform much worse on that typed-decisions benchmark.

That makes "Laya is smaller, therefore use it for every semantic decision" an invalid default. For Korean workspaces, `laya-multilingual` should be measured against Kev/TinyJev and a simple lexical baseline on the actual ctx evaluation set.

## Why AnyJev is not the default

AnyJev is strategically useful because it can turn Qwen models into Jev-style decision systems and its L2 readout can become strong with per-question labels. But L2 requires roughly 100-300 labels per question family and shipped heads are model/question specific. That is excellent for stable high-volume classifiers, not the simplest starting point for an open-ended ontology workspace.

## Integration boundary

A future model adapter may do only these tasks:

- choose likely entities from an explicit candidate list,
- choose likely relations from an explicit candidate list,
- rank candidate evidence blocks,
- decide whether retrieval is sufficient enough to stop searching.

It may not:

- create authoritative entities or edges silently,
- mark evidence valid,
- resolve stale digests,
- override domain/range or cycle checks,
- convert missing evidence into `true`,
- authorize an action.

The expected flow is:

```text
query
  -> deterministic exact lookup first
  -> local decision model only if ambiguity remains
  -> candidate evidence
  -> deterministic source verification
  -> explicit topology assertion
```

## Promotion test

Before any model becomes a product default, freeze a ctx-specific set containing Korean and English cases with:

- expected entity,
- expected relation,
- expected source file,
- expected line range,
- and expected `valid / violated / unknown` result.

Measure candidate-selection recall, confidence calibration, latency and memory separately. Model confidence is never evidence truth.

## Current research sources

- Kev v1.0 release and model cards: <https://github.com/jaredpalmer/kev>
- Kev-0.8B: <https://huggingface.co/jaredpalmer/kev-0.8b>
- TinyJev: <https://github.com/ankit-aglawe/tinyjev>
- Laya: <https://github.com/NandhaKishorM/laya>
- AnyJev: <https://github.com/nokia-applied-research/AnyJev>
- JevBench: <https://github.com/fstandhartinger/jevbench>
