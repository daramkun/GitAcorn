# GitAcorn

GitAcorn은 실행할 Git 명령과 복구 경로를 사용자가 확인할 수 있게 만든 Windows·macOS용 데스크톱 Git 클라이언트입니다. Tauri 2와 Rust 백엔드, React/TypeScript 프런트엔드로 구성되어 있습니다.

이 문서의 **저장소 루트 명령**이 개발자, 작업 에이전트, CI가 공통으로 사용하는 기준입니다. `apps/desktop`이나 개별 Rust crate에서 명령을 직접 조합하지 마세요.

## 빠른 시작

### 1. 사전 준비

- Git 2.40 이상
- Node.js 24.14.0과 pnpm 11.9.0
- Rust 1.88.0 (`clippy`, `rustfmt` 포함)
- Windows: Microsoft C++ Build Tools와 WebView2
- macOS: Xcode Command Line Tools

도구 버전은 `.node-version`, `package.json`, `rust-toolchain.toml`, `mise.toml`에 고정되어 있습니다. [mise](https://mise.jdx.dev/)를 사용하면 루트에서 다음 명령으로 Node와 pnpm을 맞출 수 있습니다.

```sh
mise install
rustup show
```

mise를 사용하지 않는 경우에도 위 버전을 직접 설치한 뒤 진행할 수 있습니다.

### 2. 의존성 설치

```sh
pnpm install --frozen-lockfile
```

의존성을 의도적으로 변경한 경우에만 `--frozen-lockfile` 없이 설치하고 `pnpm-lock.yaml` 변경을 함께 커밋합니다.

### 3. 개발 앱 실행

```sh
pnpm dev
```

Vite와 Tauri 개발 앱을 함께 실행하며 소스 변경을 반영합니다. 종료는 터미널에서 `Ctrl+C`를 사용합니다.

Tauri 기능 없이 브라우저 UI만 빠르게 확인하려면 다음 명령을 사용합니다. 네이티브 명령을 호출하는 기능은 브라우저에서 동작하지 않습니다.

```sh
pnpm dev:web
```

## 표준 명령

모든 명령은 저장소 루트에서 실행합니다.

| 목적 | 명령 | 결과 |
| --- | --- | --- |
| 데스크톱 개발 실행 | `pnpm dev` | Vite + Tauri 개발 앱 실행 |
| 브라우저 UI 실행 | `pnpm dev:web` | Vite 개발 서버만 실행 |
| 코드 포맷 적용 | `pnpm format` | Rust 워크스페이스 포맷 적용 |
| 전체 검증 | `pnpm check` | 포맷, lint, Rust/UI 테스트, 프런트엔드 빌드, 버전 일치 검사 |
| 전체 테스트 | `pnpm test` | Rust 워크스페이스와 UI 테스트 1회 실행 |
| UI 테스트 감시 | `pnpm test:web:watch` | Vitest watch 모드 실행 |
| 실행 파일만 빌드 | `pnpm build:app` | 번들 없이 release 실행 파일 생성 |
| 설치 패키지 빌드 | `pnpm build` | 현재 OS용 Tauri 번들 생성 |
| 릴리스 전 검증 | `pnpm release:check` | `pnpm check` 후 현재 OS용 번들 생성 |

`pnpm check`는 GitHub Actions의 `CI` 워크플로도 그대로 호출합니다. 작업을 넘기거나 완료하기 전에는 이 명령의 성공 여부를 함께 기록하세요.

## 빌드 산출물

### 프런트엔드만 빌드

```sh
pnpm build:web
```

산출물은 `apps/desktop/dist/`에 생성됩니다.

### 데스크톱 실행 파일

```sh
pnpm build:app
```

기본 산출물은 `target/release/`에 생성됩니다.

### 설치 패키지

```sh
pnpm build
```

- Windows: `target/release/bundle/nsis/`의 current-user NSIS 설치 프로그램
- macOS: `target/release/bundle/macos/`의 앱과 `target/release/bundle/dmg/`의 DMG

로컬 번들은 서명·공증된 공식 배포본이 아닙니다. 공식 산출물은 릴리스 워크플로에서만 생성합니다.

## 배포

배포는 `.github/workflows/release.yml`의 `Desktop Alpha release` 워크플로를 사용합니다.

1. 네 곳의 버전을 같은 값으로 변경합니다.
   - `package.json`
   - `apps/desktop/package.json`
   - `Cargo.toml`의 `workspace.package.version`
   - `apps/desktop/src-tauri/tauri.conf.json`
2. `pnpm version:check`로 버전 일치를 확인합니다.
3. `pnpm release:check`를 실행합니다.
4. 변경사항을 기본 브랜치에 반영한 뒤 버전과 같은 태그를 푸시합니다.

```sh
pnpm version:check -- --tag v0.1.0
git tag v0.1.0
git push origin v0.1.0
```

태그가 푸시되면 GitHub Actions가 Windows 설치 프로그램과 macOS universal DMG, updater artifact를 빌드하고 검증한 뒤 **draft prerelease**를 만듭니다. 담당자가 [Alpha 검증 체크리스트](docs/alpha-release.md)를 수행한 후 draft를 게시합니다.

서명과 공증에 필요한 GitHub Actions secrets는 `docs/alpha-release.md`에 정리되어 있습니다. 인증서, 개인 키, 계정 비밀번호는 저장소에 저장하지 마세요.

## 프로젝트 구조

```text
apps/desktop/              React UI와 Tauri 데스크톱 앱
crates/app-core/           애플리케이션 서비스
crates/git-cli/            시스템 Git 실행 계층
crates/git-domain/         Git 도메인 모델
crates/persistence/        SQLite 영속성
crates/test-support/       Rust 테스트 fixture
docs/                      아키텍처, 로드맵, 릴리스 체크리스트
.github/workflows/         CI와 공식 릴리스 자동화
```

설계 배경과 기능별 완료 조건은 [아키텍처 및 로드맵](docs/architecture-and-roadmap.md)을 참고하세요.

## 문제 해결

- `pnpm` 버전이 다르면 `corepack prepare pnpm@11.9.0 --activate` 또는 `mise install`로 맞춥니다.
- Rust 구성 요소가 없으면 `rustup component add clippy rustfmt`를 실행합니다.
- Windows linker 오류는 Visual Studio Build Tools의 **Desktop development with C++** 워크로드 설치 여부를 확인합니다.
- 앱은 사용자의 시스템 Git과 credential helper/SSH agent를 사용합니다. GUI에서 Git을 찾지 못하면 터미널에서 `git --version`이 먼저 동작하는지 확인합니다.
