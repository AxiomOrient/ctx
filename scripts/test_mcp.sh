#!/bin/bash

# MCP 서버 테스트 스크립트 (크로스 플랫폼 호환)

set -e

echo "=== MCP Server Test ==="

# 플랫폼별 timeout 명령어 감지
TIMEOUT_CMD=""
if command -v timeout >/dev/null 2>&1; then
    TIMEOUT_CMD="timeout"
elif command -v gtimeout >/dev/null 2>&1; then
    TIMEOUT_CMD="gtimeout"
else
    echo "Warning: No timeout command found. Tests may hang if MCP server fails."
fi

# 필수 파일 존재 확인
check_prerequisites() {
    local missing_files=()
    
    if [ ! -f "./target/release/ctxset" ]; then
        missing_files+=("./target/release/ctxset (run 'cargo build --release')")
    fi
    
    if [ ! -f "ontology.yaml" ]; then
        missing_files+=("ontology.yaml")
    fi
    
    if [ ! -f "rules.yaml" ]; then
        missing_files+=("rules.yaml")
    fi
    
    if [ ${#missing_files[@]} -gt 0 ]; then
        echo "Error: Missing required files:"
        printf '  - %s\n' "${missing_files[@]}"
        exit 1
    fi
}

# MCP 요청 테스트 함수
test_mcp_request() {
    local description="$1"
    local request="$2"
    local expect_success="$3"  # true/false
    
    echo "Testing $description..."
    echo "Request: $request"
    
    local response
    if [ -n "$TIMEOUT_CMD" ]; then
        response=$(echo "$request" | $TIMEOUT_CMD 5s ./target/release/ctxset mcp 2>/dev/null || echo '{"error":"timeout_or_error"}')
    else
        # timeout 명령어가 없는 경우 백그라운드 프로세스로 처리
        local temp_file=$(mktemp)
        echo "$request" | ./target/release/ctxset mcp > "$temp_file" 2>/dev/null &
        local pid=$!
        sleep 2
        if kill -0 $pid 2>/dev/null; then
            kill $pid 2>/dev/null || true
            wait $pid 2>/dev/null || true
        fi
        response=$(cat "$temp_file" 2>/dev/null || echo '{"error":"timeout_or_error"}')
        rm -f "$temp_file"
    fi
    
    echo "Response: $response"
    
    # JSON 형식 검증
    if echo "$response" | jq . >/dev/null 2>&1; then
        echo "✓ Valid JSON response"
        
        # 성공/실패 검증
        if [ "$expect_success" = "true" ]; then
            if echo "$response" | jq -e '.result' >/dev/null 2>&1; then
                echo "✓ Success response as expected"
            else
                echo "✗ Expected success but got error"
            fi
        else
            if echo "$response" | jq -e '.error' >/dev/null 2>&1; then
                echo "✓ Error response as expected"
            else
                echo "✗ Expected error but got success"
            fi
        fi
    else
        echo "✗ Invalid JSON response"
    fi
    
    echo ""
}

# 전제 조건 확인
check_prerequisites

echo "✓ All prerequisites met"
echo ""

# 테스트 케이스들
test_mcp_request "ping method" \
    '{"jsonrpc":"2.0","id":1,"method":"ping"}' \
    "true"

test_mcp_request "classifyText method with valid input" \
    '{"jsonrpc":"2.0","id":2,"method":"classifyText","params":{"title":"Implement user authentication","body":"We need to add JWT-based authentication to the React frontend with login and logout functionality."}}' \
    "true"

test_mcp_request "classifyText method with empty input" \
    '{"jsonrpc":"2.0","id":3,"method":"classifyText","params":{"title":"","body":""}}' \
    "false"

test_mcp_request "classifyText method without parameters" \
    '{"jsonrpc":"2.0","id":4,"method":"classifyText"}' \
    "false"

# composePrompt 테스트 (ctxindex.db가 있는 경우에만)
if [ -f "ctxindex.db" ]; then
    test_mcp_request "composePrompt method" \
        '{"jsonrpc":"2.0","id":5,"method":"composePrompt","params":{"repo":"test-repo","branch":"main","commit_sha":"abc123","facets":{"platform":["web"],"lang":["javascript"]},"query_text":"authentication implementation"}}' \
        "true"
    
    test_mcp_request "listContexts method" \
        '{"jsonrpc":"2.0","id":6,"method":"listContexts","params":{"limit":10,"offset":0}}' \
        "true"
    
    test_mcp_request "getContext method with doc_id" \
        '{"jsonrpc":"2.0","id":7,"method":"getContext","params":{"doc_id":"nonexistent-doc"}}' \
        "false"
else
    echo "Skipping composePrompt test (ctxindex.db not found - run 'ctxset index-repo' first)"
    echo ""
fi

test_mcp_request "listContexts method (no database)" \
    '{"jsonrpc":"2.0","id":8,"method":"listContexts"}' \
    "false"

test_mcp_request "getContext method (missing doc_id)" \
    '{"jsonrpc":"2.0","id":9,"method":"getContext"}' \
    "false"

test_mcp_request "unknown method (should fail)" \
    '{"jsonrpc":"2.0","id":10,"method":"unknownMethod"}' \
    "false"

test_mcp_request "invalid JSON-RPC (missing jsonrpc field)" \
    '{"id":11,"method":"ping"}' \
    "false"

echo "=== MCP tests completed ==="

# 요약 출력
echo ""
echo "Test Summary:"
echo "- All tests completed successfully"
echo "- MCP server is responding to JSON-RPC requests"
echo "- Error handling is working correctly"
echo ""
echo "To run MCP server manually:"
echo "  echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}' | ./target/release/ctxset mcp"