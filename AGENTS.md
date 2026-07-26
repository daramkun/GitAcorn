# Repository workflow

이 저장소에서 작업하는 에이전트와 개발자는 루트 `README.md`의 명령만 사용한다.

- 개발 앱 실행: `pnpm dev`
- 변경 검증: `pnpm check`
- 현재 OS의 설치 패키지 빌드: `pnpm build`
- 릴리스 전 로컬 검증: `pnpm release:check`

하위 패키지의 명령을 직접 조합하지 않는다. 표준 절차가 부족하면 임시 명령을 문서화하는 대신 루트 `package.json` 스크립트와 `README.md`를 함께 갱신한다.
