# src/cli/commands

서브커맨드 구현.
- `classify.rs`: 온톨로지 기반 문서 분류
- `github.rs`: `gh` CLI를 이용한 GitHub 연동 기능
- `import.rs`: 외부 문서를 표준 형식으로 가져오기
- `index.rs`: 문서를 SQLite 데이터베이스로 인덱싱
- `mcp.rs`: MCP (Model Context Protocol) 서버 실행
- `parse.rs`: 문서의 Frontmatter 파싱 및 유효성 검사
- `server.rs`: HTTP API 서버 실행
- `validate.rs`: 단일 문서의 스키마 유효성 검사