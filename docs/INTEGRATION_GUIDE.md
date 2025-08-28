# 외부 시스템 통합 가이드

이 문서는 ctxset을 외부 시스템과 통합하는 방법을 설명합니다.

## Webhook 통합

### 1. 서버 설정

```bash
# 환경변수로 인증 토큰 설정
export CTX_HOOK_TOKEN="your-secret-token-here"

# 서버 시작
ctxset server --port 3000 --host 0.0.0.0
```

### 2. Webhook 엔드포인트

**URL**: `POST /v1/hooks/:provider`

**Headers**:
- `Content-Type: application/json`
- `X-Auth-Token: your-secret-token-here`

**Request Body**:
```json
{
  "task": {
    "id": "TASK-123",
    "title": "Task title",
    "body": "Task description or content",
    "url": "https://example.com/task/123",
    "labels": ["tag1", "tag2"],
    "assignees": ["user1", "user2"]
  }
}
```

**Response**:
```json
{
  "provider": "linear",
  "task_id": "TASK-123",
  "suggested_facets": {
    "platform": ["web"],
    "lang": ["javascript"],
    "action": ["implement"]
  },
  "confidence": 0.85,
  "warnings": [],
  "errors": []
}
```

### 3. Provider별 설정

#### Linear
Linear에서 Webhook을 설정할 때:
- URL: `https://your-domain.com/v1/hooks/linear`
- Events: Issue created, Issue updated

#### GitHub
GitHub에서 Webhook을 설정할 때:
- URL: `https://your-domain.com/v1/hooks/github`
- Events: Issues, Pull requests

#### Jira
Jira에서 Webhook을 설정할 때:
- URL: `https://your-domain.com/v1/hooks/jira`
- Events: Issue created, Issue updated

## MCP (Model Context Protocol) 통합

### 1. MCP 서버 시작

```bash
# stdio 기반 MCP 서버 시작
ctxset mcp
```

### 2. 지원하는 메서드

#### ping
헬스체크용 메서드

**Request**:
```json
{"jsonrpc":"2.0","id":1,"method":"ping"}
```

**Response**:
```json
{"jsonrpc":"2.0","id":1,"result":{"status":"ok","service":"ctxset-mcp"}}
```

#### classifyText
텍스트 분류 메서드

**Request**:
```json
{
  "jsonrpc":"2.0",
  "id":2,
  "method":"classifyText",
  "params":{
    "title":"Implement user authentication",
    "body":"We need to add JWT-based authentication to the React frontend with login and logout functionality."
  }
}
```

**Response**:
```json
{
  "jsonrpc":"2.0",
  "id":2,
  "result":{
    "facets":{
      "platform":["web"],
      "lang":["javascript"],
      "framework":["react"],
      "action":["implement"]
    },
    "confidence":0.92,
    "warnings":[],
    "errors":[]
  }
}
```

#### composePrompt
쿼리 기반 프롬프트 조합 메서드

**Request**:
```json
{
  "jsonrpc":"2.0",
  "id":3,
  "method":"composePrompt",
  "params":{
    "repo":"my-project",
    "branch":"main",
    "commit_sha":"abc123",
    "facets":{
      "platform":["web"],
      "lang":["javascript"]
    },
    "query_text":"authentication implementation",
    "budget":2000,
    "reserve":200
  }
}
```

**Response**:
```json
{
  "jsonrpc":"2.0",
  "id":3,
  "result":{
    "merged_content":"Combined documentation content...",
    "source_documents":["doc1.md","doc2.md"],
    "rationale":"Selected documents based on facet matching and relevance",
    "tokens_used":1850,
    "confidence":0.88,
    "rejected_count":3
  }
}
```

### 3. LLM 에이전트 통합

MCP 서버는 표준 Model Context Protocol을 따르므로, 다양한 LLM 에이전트와 통합할 수 있습니다:

- **Claude Desktop**: MCP 서버로 등록하여 사용
- **Custom Agents**: stdio를 통한 JSON-RPC 통신
- **IDE Extensions**: MCP 클라이언트 라이브러리 사용

## 보안 고려사항

### 1. 인증
- `CTX_HOOK_TOKEN` 환경변수를 통한 간단한 토큰 인증
- 프로덕션 환경에서는 강력한 토큰 사용 권장
- 향후 HMAC-SHA256 서명 검증 지원 예정

### 2. 네트워크
- HTTPS 사용 권장
- 방화벽으로 접근 제한
- Rate limiting 고려

### 3. 데이터
- 민감한 정보가 포함된 태스크 처리 시 주의
- 로그에 민감한 정보 노출 방지

## 트러블슈팅

### 1. Webhook 문제
- **서버 연결 실패**: `curl -s http://localhost:3000/healthz`로 서버 상태 확인
- **토큰 검증 실패**: `CTX_HOOK_TOKEN` 환경변수가 올바르게 설정되었는지 확인
- **JSON 파싱 에러**: 요청 본문이 유효한 JSON 형식인지 확인
- **네트워크 연결**: 방화벽 및 포트 설정 확인

### 2. MCP 문제
- **파일 의존성**: `ontology.yaml`, `rules.yaml` 파일이 현재 디렉토리에 있는지 확인
- **데이터베이스**: `ctxindex.db` 파일이 있는지 확인 (없으면 `ctxset index-repo` 실행)
- **JSON-RPC 형식**: 요청이 유효한 JSON-RPC 2.0 형식인지 확인
- **stdio 통신**: MCP 서버는 stdin/stdout을 통해 통신하므로 stderr 출력 주의

### 3. 성능 최적화
- 데이터베이스 인덱스: 정기적인 `VACUUM` 실행
- 메모리 사용량: 대용량 문서 처리 시 배치 크기 조정
- 동시성: 여러 요청 처리 시 리소스 모니터링

## 예제 스크립트

테스트용 스크립트들이 `scripts/` 디렉토리에 제공됩니다:

- `scripts/test_webhook.sh`: Webhook 기능 테스트
- `scripts/test_mcp.sh`: MCP 서버 기능 테스트

```bash
# Webhook 테스트 실행
./scripts/test_webhook.sh

# MCP 테스트 실행
./scripts/test_mcp.sh
```
