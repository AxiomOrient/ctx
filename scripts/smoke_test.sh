#!/usr/bin/env bash
set -euo pipefail

# ===== Config =====
BASE_URL="${BASE_URL:-http://localhost:3000}"
CURL="curl -sS -D /tmp/ctxset_headers.txt -o /tmp/ctxset_body.json"
JQ="${JQ:-jq}"

green(){ printf "\033[32m%s\033[0m\n" "$1"; }
yellow(){ printf "\033[33m%s\033[0m\n" "$1"; }
red(){ printf "\033[31m%s\033[0m\n" "$1"; }
section(){ printf "\n\033[36m== %s ==\033[0m\n" "$1"; }

# Expect HTTP status
expect_status(){
  local want="$1"
  local got
  got="$(awk 'toupper($1)=="HTTP/"{code=$2} END{print code}' /tmp/ctxset_headers.txt)"
  if [[ "$got" != "$want" ]]; then
    red "❌ Expected HTTP $want but got $got"
    echo "---- Response body ----"
    cat /tmp/ctxset_body.json || true
    exit 1
  fi
  green "✅ HTTP $want"
}

# Pretty print last JSON
show_json(){
  if command -v "$JQ" >/dev/null 2>&1; then
    "$JQ" . /tmp/ctxset_body.json || cat /tmp/ctxset_body.json
  else
    cat /tmp/ctxset_body.json
  fi
}

# ===== 0. Health check (HTML root or ping) =====
section "0) Health Check"
$CURL "$BASE_URL/" || true
expect_status "200"
yellow "ℹ︎ Root served (HTML expected)."

# ===== 1. YAML Validation: rules (bad -> 400) =====
section "1) Validate Rules (BAD YAML -> 400)"
$CURL -X POST "$BASE_URL/v1/validate/rules"   -H 'Content-Type: text/yaml'   --data-binary @- <<'YAML'
rules:
  - id: bad-rule
    when: key: value   # <-- intentionally invalid YAML structure
    then:
      add_facet:
        ns: lang
        key: javascript
YAML
expect_status "400"
show_json

# ===== 2. YAML Validation: rules (good -> 200) =====
section "2) Validate Rules (GOOD YAML -> 200)"
$CURL -X POST "$BASE_URL/v1/validate/rules"   -H 'Content-Type: text/yaml'   --data-binary @- <<'YAML'
rules:
  - id: good-rule
    when:
      contains: "login"
    then:
      add_facet:
        ns: "framework"
        key: "react"
YAML
expect_status "200"
show_json

# ===== 3. PUT /v1/rules: reject bad YAML (400) =====
section "3) PUT /v1/rules (BAD -> 400)"
$CURL -X PUT "$BASE_URL/v1/rules"   -H 'Content-Type: text/yaml'   --data-binary @- <<'YAML'
rules:
  - id: broken
    when: [  # <-- invalid structure for your loader
      "oops"
    ]
    then:
      add_facet: { ns: lang, key: javascript }
YAML
expect_status "400"
show_json

# ===== 4. PUT /v1/rules: accept good YAML (200) =====
section "4) PUT /v1/rules (GOOD -> 200)"
$CURL -X PUT "$BASE_URL/v1/rules"   -H 'Content-Type: text/yaml'   --data-binary @- <<'YAML'
rules:
  - id: login-react
    when:
      contains: "login"
    then:
      add_facet:
        ns: "framework"
        key: "react"
  - id: lang-js
    when:
      contains: "javascript"
    then:
      add_facet:
        ns: "lang"
        key: "javascript"
YAML
expect_status "200"
show_json

# ===== 5. Validate Ontology: good sample (200) =====
section "5) Validate Ontology (GOOD -> 200)"
$CURL -X POST "$BASE_URL/v1/validate/ontology"   -H 'Content-Type: text/yaml'   --data-binary @- <<'YAML'
namespaces:
  - id: "lang"
    values: ["javascript","swift","rust"]
  - id: "framework"
    values: ["react","swiftui","axum"]
YAML
expect_status "200"
show_json

# ===== 6. Classify: natural language -> facets =====
section "6) POST /v1/classify"
$CURL -X POST "$BASE_URL/v1/classify"   -H 'Content-Type: application/json'   --data @<(cat <<'JSON'
{
  "text": "Create a login form using React and JavaScript.",
  "options": { "explain": true }
}
JSON
)
expect_status "200"
show_json

# ===== 7. Compose: facets -> prompt =====
section "7) POST /v1/compose"
$CURL -X POST "$BASE_URL/v1/compose"   -H 'Content-Type: application/json'   --data @<(cat <<'JSON'
{
  "facets": {
    "lang": ["javascript"],
    "framework": ["react"]
  },
  "parameters": {
    "token_budget": 1200
  }
}
JSON
)
expect_status "200"
show_json

yellow "ℹ︎ If compose tracking is enabled, the dashboard should reflect an increased total_compositions."

# ===== 8. Optional: Upload document (CRUD smoke) =====
section "8) Upload Document (optional)"
if [[ -f "README.md" ]]; then
  $CURL -X POST "$BASE_URL/v1/upload"     -H 'Content-Type: multipart/form-data'     -F "file=@README.md"
  expect_status "200"
  show_json
else
  yellow "README.md not found — skipping upload test."
fi

green "🎉 All smoke tests passed."
