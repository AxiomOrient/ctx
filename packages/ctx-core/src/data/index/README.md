# src/data/index

SQLite 인덱스/검색/스냅샷(SCD2) 관리.

## 파일
- `pooled_sqlite.rs`: **(권장)** r2d2 커넥션 풀을 사용하는 고성능 SQLite 인덱스 매니저.
- `sqlite.rs` : 단일 커넥션 SQLite 인덱스 매니저 (레거시/간단한 스크립트용).
- `pool.rs`: SQLite 커넥션 풀 관리 로직.
- `sqlite_schema.sql` : DDL

## 규칙
- 단위 트랜잭션 범위 명확히
- N+1 쿼리 금지 (준비/배치 사용)
- 동시성(concurrency)이 필요한 경우 반드시 `pooled_sqlite` 사용.