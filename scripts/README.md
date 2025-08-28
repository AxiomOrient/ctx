# scripts

개발/검증/배포 도구 스크립트.

## 관례
- `set -euo pipefail` 기본
- 외부 의존 툴은 사전 체크
- 변경을 만드는 스크립트는 dry-run 옵션 제공 권장

## 예시
- `bootstrap.sh` : 로컬 초기 설정
- `verify_green.sh` : build+test+clippy 모두 통과 확인
- `clean_legacy.sh` : 레거시 코드 제거 배치