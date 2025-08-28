# Repository Guidelines

This document provides high-level guidelines for contributing to the repository.
For detailed technical specifications, architecture, and specific development commands, please refer to **`CLAUDE.md`**.

## Commit & Pull Request Guidelines
- **Commits**: Follow the Conventional Commits specification (e.g., `feat: add validator`, `fix(cli): handle missing marker`).
- **Pull Requests**: Include a clear description of changes and link to any relevant issues. For behavior changes, provide before/after examples.
- **Requirements**: All pull requests must pass the automated checks (testing, linting, formatting) defined in the CI pipeline. These checks are based on the strict commands found in `CLAUDE.md`.
- **Documentation**: Update `docs/` or the `README.md` when altering CLI flags, file layouts, or public APIs.

## General Principles
- **Architecture**: All changes must adhere to the layered architecture and unidirectional dependency rules described in `CLAUDE.md`.
- **Code Style**: Follow standard Rust conventions and the strict linting rules enforced by the CI pipeline.
- **Testing**: Add tests for new behaviors and edge cases. Ensure all tests pass before submitting a pull request.

## Security & Configuration
- **Schema**: Context documents use a `context.v1` YAML frontmatter. Ensure markers in the frontmatter match the Markdown headings.
- **Sensitive Information**: Avoid committing large binaries, generated `target/` artifacts, or any sensitive keys or configuration.

