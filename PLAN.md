# GitAcorn 경쟁 기능 보강 계획

> 기준일: 2026-08-02  
> 비교 대상: Fork, GitKraken Desktop  
> 원칙: 각 항목은 UI, Tauri IPC, 애플리케이션 서비스, 실제 Git fixture 테스트를 포함하는 하나의 수직 기능으로 완성한다.

## 현재 기준선

GitAcorn은 다음 핵심 흐름을 이미 지원한다.

- 여러 저장소 탭과 세션 복원, 저장소 열기와 clone
- working tree 상태, 파일·hunk·선택 라인 stage/unstage/discard
- commit/amend, commit graph와 history/reference 검색
- branch/tag/remote/submodule 관리
- fetch/pull/push와 작업 진행률·취소·진단
- stash 생성·적용·삭제, 기본 merge conflict 해결
- merge, fast-forward, rebase와 interactive rebase
- worktree 조회와 전환

## P0 — 핵심 Git 클라이언트 격차

아래 순서대로 구현한다. 위험한 history 변경보다 복구 기반을 먼저 마련하고, 공통 preview/confirmation 모델을 재사용한다.

### P0.1 Operation recovery와 Undo/Redo

- [x] commit 직전·직후 HEAD를 복구 레코드로 저장
- [x] commit의 복구 가능 여부와 Undo/Redo를 Operations 화면에 표시
- [x] clean checkout과 로컬 branch 삭제의 안전한 Undo/Redo 지원
- [x] 복구 불가능한 discard는 실행 전에 Undo 불가로 명확히 구분
- [x] clean non-interactive rebase의 안전한 Undo/Redo 지원
- [x] reset과 interactive rebase의 안전한 Undo/Redo 지원
- [x] commit을 undo한 직후 redo 지원
- [x] 앱 외부에서 HEAD가 변경되었거나 복구 전제조건이 깨지면 실행을 거부
- [x] reflog 탐색과 선택 항목에서 branch/tag 복구 기능 제공
- [x] 자격 증명, 파일 내용, remote secret이 복구 기록에 포함되지 않는지 allowlist schema test로 검증

완료 조건:

- 실제 저장소 fixture에서 지원 작업별 undo/redo 왕복 테스트가 통과한다.
- 복구 불가능한 discard 등은 실행 전에 명확히 구분되며 거짓 Undo를 제공하지 않는다.

### P0.2 Cherry-pick, Revert, Reset

- [x] 단일·다중 commit cherry-pick과 실행 전 충돌 가능성 preview
- [x] revert와 revert 진행 중 continue/abort
- [x] soft/mixed/hard reset의 변경 범위 preview
- [x] hard reset은 대상 OID, 삭제될 working tree/index 변경과 복구 ref를 확인
- [x] 진행 중 cherry-pick의 continue/skip/abort
- [x] commit graph context menu와 키보드 접근성 제공

완료 조건:

- clean/conflict/empty commit/merge commit fixture를 포함한다.
- 모든 destructive 동작이 P0.1 복구 레코드와 연결된다.

### P0.3 Blame과 File/Directory History

- [x] 선택 commit 또는 working tree 기준 file blame
- [x] line별 author, commit, timestamp 표시와 commit graph 이동
- [x] rename 추적을 포함한 file history
- [x] directory/path history와 path filter
- [x] 큰 파일에서 취소 가능한 비동기 로딩과 가상화

완료 조건:

- rename, non-UTF-8 path, merge history fixture에서 결과가 Git CLI와 일치한다.

### P0.4 Worktree 전체 lifecycle

- [x] branch 또는 remote branch에서 worktree 생성
- [x] 기존 worktree 열기·전환과 새 탭으로 열기
- [x] worktree lock/unlock
- [x] 안전한 remove와 `--force`가 필요한 상태의 명시적 확인
- [x] worktree 제거와 branch 삭제를 결합한 선택적 흐름
- [x] submodule이 포함된 worktree와 같은 이름의 폴더를 구분

완료 조건:

- main/linked worktree 간 동일 저장소 ID와 독립 index가 유지된다.
- dirty/locked/missing/prunable worktree fixture를 모두 처리한다.

### P0.5 고급 Diff와 비교

- [x] unified/split 전환, word diff, syntax highlighting과 word wrap
- [x] 임의의 두 commit, branch, tag, WIP 간 비교
- [x] image side-by-side/overlay diff와 binary metadata 표시
- [x] hunk 탐색, diff 내 검색, large/minified file 보호
- [x] 외부 diff/merge tool 설정과 실행
- [x] text patch 생성·저장·적용 및 적용 전 검증

완료 조건:

- text, rename, binary, image, large file fixture의 렌더링과 메모리 제한을 검증한다.

### P0.6 Git LFS와 서명 상태

- [x] 저장소의 LFS 사용 여부와 tracked/pointer 상태 표시
- [x] LFS fetch/pull/prune와 진행률·취소
- [x] LFS lock 목록, lock/unlock과 소유자 표시
- [x] commit/tag의 GPG·SSH 서명 상태 표시
- [x] 시스템 Git 설정을 존중하는 commit/tag 서명 옵션

완료 조건:

- LFS가 설치되지 않은 환경과 인증 실패를 복구 가능한 오류로 안내한다.
- GitAcorn은 개인 키나 credential을 직접 저장하지 않는다.

## P1 — 생산성 및 forge 연동

- [x] built-in 3-way merge editor와 hunk 단위 conflict resolution
- [x] 저장소 init, `.gitignore`/license template 선택
- [x] bisect 시각화와 good/bad/skip 흐름
- [x] command palette, repository terminal, 사용자 정의 command
- [x] GitHub/GitLab/Bitbucket/Azure DevOps 계정과 저장소 탐색
- [x] PR/MR 생성·조회·checkout·merge와 review/CI 상태
- [x] 여러 저장소 Workspace와 일괄 clone/fetch/pull
- [ ] 다중 계정/profile, SSH key 및 Git identity profile
- [ ] Git-flow와 branch naming preset

## P2 — 협업 및 지능형 기능

- [ ] PR·issue·CI를 모은 개인/팀 dashboard와 알림
- [ ] cloud/shared patch 또는 호환 가능한 자체 호스팅 patch 공유
- [ ] worktree 기반 coding-agent session 관리
- [ ] 선택 가능한 provider를 사용하는 AI commit/PR 문안 생성
- [ ] commit/branch 설명과 opt-in AI code review
- [ ] AI conflict resolution은 항상 patch preview와 사용자 승인을 거쳐 적용
- [ ] Linux 패키징과 배포 검증

## 공통 품질 게이트

각 수직 기능은 다음 조건을 충족해야 완료로 표시한다.

- [x] 위험도, 실행할 Git 명령, 변경 범위와 복구 경로가 UI에 표시된다.
- [x] 저장소별 write 직렬화와 stale revision 검증을 통과한다.
- [x] 명령 인수는 shell 문자열이 아닌 배열로 전달되고 민감 정보가 마스킹된다.
- [x] Rust unit/integration test와 React UI test가 추가된다.
- [x] Windows와 macOS의 플랫폼 차이를 문서화하고 해당 CI를 통과한다.
- [x] 저장소 루트에서 `pnpm check`가 통과한다.

## 구현 진행 기록

| 항목 | 상태 | 비고 |
| --- | --- | --- |
| P0.1 Operation recovery와 Undo/Redo | 완료 | commit/checkout/branch 삭제/clean rebase/reset/interactive rebase Undo/Redo, reflog ref 복구, recovery schema allowlist 검증 |
| P0.2 Cherry-pick, Revert, Reset | 완료 | 단일·다중 cherry-pick, revert continue/abort, cherry-pick skip, soft/mixed/hard reset preview와 P0.1 recovery 연결 |
| P0.3 Blame과 File/Directory History | 완료 | byte-safe blame, rename-aware file/directory history, path filter와 비동기 검사기 |
| P0.4 Worktree 전체 lifecycle | 완료 | 생성·새 탭 열기·lock/unlock·안전/강제 제거·branch 결합 제거, missing/prunable 상태와 path 기반 식별자 |
| P0.5 고급 Diff와 비교 | 완료 | 임의 ref/WORKTREE 비교, split·word·syntax·wrap renderer, 이미지/바이너리 미리보기, 외부 diff 실행과 patch lifecycle |
| P0.6 Git LFS와 서명 상태 | 완료 | LFS 감지·pointer/lock 상태·fetch/pull/prune 진행/취소와 commit/tag 서명 상태·시스템 설정 옵션 |
| P1.1 Built-in 3-way merge editor | 완료 | base/current/incoming 원본 표시, hunk별 현재·incoming·양쪽 선택과 수동 편집, stale worktree 검증 및 해결 파일 stage 적용 |
| P1.2 저장소 초기화와 템플릿 | 완료 | 기존 폴더 보존형 init, 초기 브랜치 지정, Node·Rust·Python·Go `.gitignore`와 MIT·BSD 3-Clause 라이선스 선택, 기존 파일 덮어쓰기 방지 |
| P1.3 Bisect 시각화 | 완료 | known-good·known-bad 범위 시작, History 그래프의 현재 후보·good·bad·skip 표식, good·bad·skip 판정과 원래 브랜치 복원, clean worktree·revision 보호 |
| P1.4 명령 팔레트와 저장소 도구 | 완료 | 전역 명령 팔레트와 Ctrl+Shift+P, 저장소 터미널 열기, 배열 인자 기반 사용자 명령 저장·실행 확인·stdout/stderr 결과 팝업 |
| P1.5 Forge 계정과 저장소 탐색 | 완료 | GitHub·GitLab·Bitbucket·Azure DevOps 토큰 검증, Git credential helper 보관, 비밀값 없는 계정 메타데이터, 저장소 검색·복제 브라우저 |
| P1.6 Forge PR/MR workflow | 완료 | GitHub·GitLab·Bitbucket·Azure DevOps PR/MR 조회·생성·checkout·merge, review/CI 상태, source OID 검증과 merged 상태 보호 |
| P1.7 여러 저장소 Workspace | 완료 | 이름 있는 저장소 그룹 영속화, 열린 저장소 추가, 누락 경로 clone과 저장소별 fetch·fast-forward pull 결과, 부분 실패 계속 처리 및 실행 확인 팝업 |
