# ctx-cli

Command-line interface and server for ctxset.

## ctxset.toml (optional)

Configure defaults for CLI/MCP/Server.

```toml
# contexts_dir: Optional base directory for context files
contexts_dir = "contexts"

# Optional paths for MCP/server
# Can be overridden by ENV: CTX_ONTOLOGY / CTX_RULES / CTX_DB
ontology = "conf/ontology.yaml"
rules    = "conf/rules.yaml"
db       = ".data/ctxindex.db"
```

If not provided, commands fall back to current working directory defaults.
Environment variables take precedence over TOML.

## Environment variables

- `CTX_ONTOLOGY`: Path to ontology YAML
- `CTX_RULES`: Path to rules YAML
- `CTX_DB`: Path to SQLite DB
- `CTX_BODY_LIMIT_MB`: Max request body size for server (default 10)
- `CTX_CORS_ANY`: If set, enables very permissive CORS (development only)
- `CTX_CORS_ALLOW`: Comma-separated allowed origins (simplified handling)

## Example: run server with externalized paths

```bash
CTX_ONTOLOGY=./conf/ontology.yaml \
CTX_RULES=./conf/rules.yaml \
CTX_DB=./.data/ctx.db \
cargo run -p ctx-cli --features server -- server
```
