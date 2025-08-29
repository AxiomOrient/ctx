Golden tests
===========

- Location: `tests/golden/cases/compose/*.in.json`
- Each `*.in.json` is a `ComposeInput` that runs through `services::compose_prompt`.
- Output is compared against `*.out.golden.json` next to each input file.

Usage
-----

- Generate or refresh goldens:

  UPDATE_GOLDEN=1 cargo test -q golden_compose_cases -- --nocapture

- Verify without updating:

  cargo test -q golden_compose_cases

MCP schema snapshots
--------------------

- Location: `tests/mcp_schema.rs` writes goldens under `tests/golden/mcp/*.out.golden.json`.
- Covers: `tools/list`, `tools/call (compose_prompt)`, `resources/list`, `resources/read` (content omitted), `prompts/list`, `prompts/get`.

- Generate/refresh MCP goldens:

  UPDATE_GOLDEN=1 cargo test -q --test mcp_schema -- --nocapture

- Verify without updating:

  cargo test -q --test mcp_schema

Notes
-----

- The test normalizes the `sources` array order for determinism.
- Keep inputs minimal and descriptive; larger scenarios can be added as needed.
