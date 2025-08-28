# src/doc/parse

Markdown + Frontmatter 파싱, 섹션 추출.

## 파일
- `frontmatter.rs`: `FrontmatterParser`가 frontmatter를 파싱하고 `ContextMetadata`를 검증합니다.
- `document.rs`: `DocumentParser`가 파일/콘텐츠에서 `ContextDocument`를 생성합니다.
- `section.rs`: `SectionExtractor`가 문서 본문에서 정의된 섹션을 추출합니다.

## 원칙
- 파일 IO는 `storage` 통해 수행
- 구분자 등 상수는 `common/constants/parsing.rs`에서 가져옵니다.