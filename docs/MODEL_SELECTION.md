# Local decision model selection

Research snapshot: **2026-10-04**.

The deterministic core has **no model dependency and no default model**. A model is an optional semantic candidate-selection layer and must earn its place on a frozen ctx-specific evaluation set.

## Current recommendation

Benchmark three small candidates first. Do not choose from headline scores alone because their training data, language coverage and evaluation suites differ.

### 1. Korean-first candidate: Laya multilingual (322M)

Use `laya-multilingual` as the smallest first benchmark for Korean workspaces.

Why:

- mmBERT-base, 322M parameters.
- Explicitly supports 100+ languages including Korean.
- Default 1,024-token context; the implementation supports up to 8,192 with `max_len=8192`.
- Single-forward typed `choice`, `score`, and `noul` decisions; no text generation.
- Current public MASSIVE results report Korean intent accuracy 0.450 for the multilingual checkpoint versus 0.110 for the English checkpoint.

Limits:

- Those results do not prove ctx ontology/retrieval quality.
- Published calibration remains task dependent; fit and verify temperature on the ctx evaluation set before using confidence thresholds.
- The generic multilingual checkpoint is not a replacement for the separately fine-tuned typed-decisions checkpoint on that benchmark.
- Long-document results become variable beyond roughly 4k tokens in the project's own small benchmark, so long documents still need ctx-specific measurement.

### 2. Apple-Silicon ultra-light candidate: TinyJev-0.6B

Benchmark `AnkitAI/TinyJev-0.6B` when local memory, startup and MLX deployment matter most.

Why:

- 596M parameters, about 1.2 GB at fp16.
- Native MLX path on Apple Silicon and PyTorch elsewhere.
- Fully offline after the initial download.
- System One-compatible typed decisions without autoregressive generation.
- The project supports 8-bit load-time quantization on Apple Silicon; its authors report roughly half the memory and no score loss on their held-out set.

Limits:

- Project benchmarks are not directly comparable with Laya or Kev suites.
- Korean documentation exists, but Korean ctx accuracy is still **unverified**.

### 3. English/document candidate: Kev-0.8B v1.0

Benchmark `jaredpalmer/kev-0.8b@v1.0` when document decisions and a longer validated context matter more than the smallest footprint.

Why:

- Qwen3.5-0.8B-Base plus LoRA adapter and pointer head.
- Apache-2.0 model/base licensing.
- One-prefill typed decision architecture; no generated answer text.
- Kev 1.0 validates 8,192-token context for the 0.8B model.
- Current project results report 0.851 on its held-out real-document test and 0.697 on its locked out-of-domain transfer test.

Limits:

- The model card explicitly lists **English** as the language. Korean quality is **unverified** and must not be inferred from the Qwen backbone.
- The reported real-document set is partly in a trained family; the locked transfer row is the better indication of transfer, but neither is a ctx benchmark.

## Escalation candidates

### Kev-4B

Use only if the small models fail the frozen ctx accuracy/recall gate and the additional memory is justified. Kev 1.0 reports materially stronger results than 0.8B across its current transfer and document suites.

### AnyJev L2

Use when ctx stabilizes into a small set of repeated typed questions and labelled examples accumulate.

AnyJev's current L2 method fits a closed-form head per question from roughly 100-300 labels, reads hidden state around two thirds of model depth, and does not change backbone weights. That is attractive for a stable high-volume classifier, but it is not the simplest starting point for an open-ended ontology workspace and the head does not transfer to a different question.

## Product rule

Do **not** hard-code any of these models into graph truth.

A future model adapter may only:

- shortlist likely entities from explicit candidates,
- shortlist likely relations from explicit candidates,
- rank candidate evidence blocks,
- decide whether semantic retrieval should continue.

It may not:

- silently create authoritative entities or edges,
- validate evidence freshness,
- override relation domain/range or cycle checks,
- convert missing evidence into success,
- authorize an action.

The flow remains:

```text
query
  -> exact graph/source lookup first
  -> optional local decision model if ambiguity remains
  -> candidate evidence
  -> deterministic source verification
  -> explicit topology assertion
```

## Promotion test

Freeze a bilingual Korean/English ctx evaluation set before selecting a product model. Each case records:

- expected entity,
- expected relation,
- expected source,
- expected exact line range,
- expected `valid / violated / unknown`,
- and whether semantic model invocation was actually necessary.

Measure separately:

- candidate recall,
- evidence/anchor precision,
- unsupported-assertion rate,
- calibration (ECE/Brier where probabilities are used),
- p50/p95 latency,
- peak memory,
- cold-start cost.

Model confidence is never evidence truth.

## Current sources

- Laya: <https://github.com/NandhaKishorM/laya>
- Kev v1.0: <https://github.com/jaredpalmer/kev>
- Kev-0.8B: <https://huggingface.co/jaredpalmer/kev-0.8b>
- TinyJev: <https://github.com/ankit-aglawe/tinyjev>
- AnyJev: <https://github.com/nokia-applied-research/AnyJev>
