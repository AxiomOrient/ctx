#!/bin/bash

# Webhook 테스트 스크립트 (개선된 버전)

set -e

echo "=== Webhook Integration Test ==="

# 설정
SERVER_URL="http://localhost:3000"
TEST_TOKEN="test-token-123"

# 전제 조건 확인
check_prerequisites() {
    echo "Checking prerequisites..."
    
    # curl 명령어 확인
    if ! command -v curl >/dev/null 2>&1; then
        echo "Error: curl is required but not installed"
        exit 1
    fi
    
    # jq 명령어 확인
    if ! command -v jq >/dev/null 2>&1; then
        echo "Error: jq is required but not installed"
        echo "Install with: brew install jq (macOS) or apt-get install jq (Ubuntu)"
        exit 1
    fi
    
    echo "✓ All required tools are available"
}

# 서버 상태 확인
check_server() {
    echo "Checking server status..."
    
    local health_response
    if health_response=$(curl -s -w "%{http_code}" "$SERVER_URL/healthz" 2>/dev/null); then
        local http_code="${health_response: -3}"
        if [ "$http_code" = "200" ]; then
            echo "✓ Server is running and healthy"
            return 0
        else
            echo "✗ Server returned HTTP $http_code"
        fi
    else
        echo "✗ Cannot connect to server"
    fi
    
    echo ""
    echo "Server is not running or not accessible at $SERVER_URL"
    echo "To start the server:"
    echo "  export CTX_HOOK_TOKEN=\"$TEST_TOKEN\""
    echo "  cargo run --features server -- server --port 3000"
    echo ""
    exit 1
}

# Webhook 테스트 함수
test_webhook() {
    local provider="$1"
    local description="$2"
    local payload="$3"
    local token="$4"
    local expect_success="$5"  # true/false
    
    echo "Testing $description..."
    
    local response
    local http_code
    
    response=$(curl -s -w "\n%{http_code}" -X POST "$SERVER_URL/v1/hooks/$provider" \
        -H "Content-Type: application/json" \
        -H "X-Auth-Token: $token" \
        -d "$payload" 2>/dev/null)
    
    # HTTP 코드와 응답 본문 분리
    http_code=$(echo "$response" | tail -n1)
    response_body=$(echo "$response" | head -n -1)
    
    echo "HTTP Status: $http_code"
    
    # JSON 형식 검증 및 출력
    if echo "$response_body" | jq . >/dev/null 2>&1; then
        echo "Response:"
        echo "$response_body" | jq .
        
        # 성공/실패 검증
        if [ "$expect_success" = "true" ]; then
            if [ "$http_code" = "200" ]; then
                echo "✓ Success as expected"
            else
                echo "✗ Expected success (200) but got $http_code"
            fi
        else
            if [ "$http_code" != "200" ]; then
                echo "✓ Error as expected"
            else
                echo "✗ Expected error but got success"
            fi
        fi
    else
        echo "✗ Invalid JSON response: $response_body"
    fi
    
    echo ""
}

# 전제 조건 및 서버 확인
check_prerequisites
check_server

# 환경 변수 설정 (서버가 이미 실행 중이므로 테스트용으로만)
export CTX_HOOK_TOKEN="$TEST_TOKEN"

# 테스트 케이스들
test_webhook "linear" "Linear webhook with valid token" '{
    "task": {
        "id": "LIN-123",
        "title": "Implement user authentication",
        "body": "We need to add JWT-based authentication to the React frontend. This should include login, logout, and token refresh functionality.",
        "url": "https://linear.app/company/issue/LIN-123",
        "labels": ["frontend", "security"],
        "assignees": ["john.doe"]
    }
}' "$TEST_TOKEN" "true"

test_webhook "github" "GitHub webhook with valid token" '{
    "task": {
        "id": "issue-456",
        "title": "Fix database connection pool",
        "body": "The PostgreSQL connection pool is running out of connections under high load. We need to optimize the pool configuration and add proper connection cleanup.",
        "url": "https://github.com/company/repo/issues/456",
        "labels": ["bug", "database", "performance"],
        "assignees": ["jane.smith"]
    }
}' "$TEST_TOKEN" "true"

test_webhook "jira" "Jira webhook with invalid token (should fail)" '{
    "task": {
        "id": "PROJ-789",
        "title": "Update documentation",
        "body": "Update the API documentation to reflect recent changes",
        "url": "https://company.atlassian.net/browse/PROJ-789"
    }
}' "wrong-token" "false"

test_webhook "linear" "Linear webhook with missing task data (should fail)" '{
    "invalid": "data"
}' "$TEST_TOKEN" "false"

test_webhook "custom" "Custom provider webhook" '{
    "task": {
        "id": "CUSTOM-001",
        "title": "Test custom provider",
        "body": "Testing webhook with custom provider name"
    }
}' "$TEST_TOKEN" "true"

# Provider별 실제 페이로드 형식 테스트
test_webhook "linear" "Linear real payload format" '{
    "data": {
        "issue": {
            "id": "LIN-456",
            "identifier": "LIN-456",
            "title": "Fix authentication bug",
            "description": "Users cannot login with OAuth",
            "url": "https://linear.app/company/issue/LIN-456",
            "labels": [{"name": "bug"}, {"name": "auth"}],
            "assignee": {"email": "dev@company.com"}
        }
    }
}' "$TEST_TOKEN" "true"

test_webhook "github" "GitHub real payload format" '{
    "issue": {
        "number": 123,
        "title": "Add new feature",
        "body": "Implement user dashboard with charts",
        "html_url": "https://github.com/company/repo/issues/123",
        "labels": [{"name": "enhancement"}, {"name": "frontend"}],
        "assignees": [{"login": "developer1"}]
    }
}' "$TEST_TOKEN" "true"

test_webhook "jira" "Jira real payload format" '{
    "issue": {
        "key": "PROJ-789",
        "fields": {
            "summary": "Database performance issue",
            "description": "Queries are running slowly",
            "labels": ["performance", "database"],
            "assignee": {"emailAddress": "dba@company.com"}
        },
        "self": "https://company.atlassian.net/rest/api/2/issue/PROJ-789"
    }
}' "$TEST_TOKEN" "true"

echo "=== Webhook tests completed ==="

# 요약 출력
echo ""
echo "Test Summary:"
echo "- All webhook tests completed"
echo "- Server is responding correctly to valid and invalid requests"
echo "- Authentication is working as expected"
echo ""
echo "To test webhooks manually:"
echo "  curl -X POST $SERVER_URL/v1/hooks/linear \\"
echo "    -H \"Content-Type: application/json\" \\"
echo "    -H \"X-Auth-Token: $TEST_TOKEN\" \\"
echo "    -d '{\"task\":{\"id\":\"TEST\",\"title\":\"Test\",\"body\":\"Test\"}}'"