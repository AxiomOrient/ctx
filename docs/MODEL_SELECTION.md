# Optional decision backends

Research snapshot: **2026-10-04**.

The deterministic graph has no model dependency and no default model.

A model is useful only when ID/title/alias lookup does not resolve the user's language to one entity. Model selection is therefore a replaceable adapter concern, not a graph concern.

## Stable ctx boundary

ctx speaks one local stdin/stdout protocol:

```text
ctx
  -> Decision Adapter Protocol v1
       -> Laya-Ko
       -> TinyJev
       -> Kev
       -> AnyJev
       -> another future backend
```

The adapter receives a query and an explicit candidate set and may return one candidate ID plus optional confidence.

ctx validates that the returned ID belongs to the candidate set. Confidence is informational only. It never changes evidence status, relation validity, cardinality, or authorization.

This external-process boundary is intentional:

- no Python/ONNX/MLX/PyTorch dependency enters the Rust core,
- no model download policy enters the workspace contract,
- no vendor HTTP API becomes a public ctx API,
- backend replacement requires no migration,
- backend crashes and malformed output fail visibly.

Several current System One projects expose compatible or similar typed-decision APIs, but their serving surfaces are still evolving. For example, upstream Laya has an Oct. 4 feature request for serving user checkpoints beside built-ins. ctx should not bind its public contract to that churn.

## Current benchmark candidates

### Korean-first: Laya-Ko 322M

`2nugu/laya-ko` is the first Korean benchmark candidate.

Current project facts:

- mmBERT-base decision encoder, 322M parameters.
- Korean fine-tune of Laya's non-generative typed-decision architecture.
- Apache-2.0.
- ONNX export and a Rust reference client are provided.
- The published Rust client is explicitly marked **not compiled as published**; its `ort` release-candidate API may require adjustment.
- Inputs are truncated from the right at 1,024 tokens and the project currently has no windowing.
- The project reports large gains over upstream multilingual Laya on KLUE-RE, YNAT, NLI, STS and calibration.

Those are project-reported benchmark results, not ctx evidence. The model is therefore a benchmark candidate, not a product default. Promotion requires a frozen ctx Korean set.

### Small Apple Silicon: TinyJev-0.6B

`AnkitAI/TinyJev-0.6B` is the small MLX comparison.

Current project facts:

- 596M parameters, about 1.2 GB fp16.
- MLX on Apple Silicon, PyTorch elsewhere.
- fully offline after download.
- one-forward Choice / Noul / Score decisions.
- System One-compatible server shape.
- 8-bit load-time quantization is supported; the project reports similar held-out accuracy at lower memory.

Do not use Noul confidence as evidence truth. It is a semantic decision aid only.

### Longer English documents: Kev-0.8B v1.0

Kev 1.0 pins the family as of 2026-10-01.

Kev-0.8B:

- Qwen3.5-0.8B-Base + LoRA/head.
- Apache-2.0 base/model family.
- one-forward typed decisions.
- validated context: 8,192 tokens.
- System One serving contract.

The current model card is English-focused. Korean performance is unverified.

### Stable repeated questions: AnyJev L2

AnyJev 0.1.0 (2026-09-26) adds closed-form per-question L2 heads.

Use it only when a small set of repeated decision questions has real labels:

- roughly 100-300 labels per question in the published method,
- fixed question/model-specific artifacts,
- early-exit hidden-state readout,
- calibrated probabilities.

This is appropriate for a mature repeated classifier, not the initial open-ended knowledge graph.

## Promotion policy

Do not promote a model based on cross-project headline scores.

Freeze Korean and English ctx cases containing:

- query,
- allowed candidate set,
- expected entity or no-selection,
- expected evidence source and exact quote where applicable.

Measure:

- candidate recall,
- wrong-selection rate,
- abstention quality,
- calibration if confidence is consumed,
- p50/p95 latency,
- peak memory,
- cold start.

If deterministic lookup already meets the requirement, use no model.

## Adapter policy

A backend may:

- select one entity from explicit candidates,
- abstain,
- provide optional confidence.

It may not:

- create entities or relations,
- return an entity outside the candidate set,
- alter evidence state,
- bypass schema/cardinality/cycle checks,
- convert Unknown into Satisfied,
- authorize an action.

## Sources

- Laya-Ko: <https://github.com/2nugu/laya-ko-decision-onnx>
- Laya upstream: <https://github.com/NandhaKishorM/laya>
- TinyJev: <https://github.com/ankit-aglawe/tinyjev>
- Kev 1.0: <https://github.com/jaredpalmer/kev/blob/main/docs/releases/kev-1.0.md>
- AnyJev: <https://github.com/nokia-applied-research/AnyJev>
