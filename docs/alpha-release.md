# GitAcorn Alpha release

## Release flow

1. Run every command in the quality gate documented in `architecture-and-roadmap.md`.
2. Generate one Tauri updater key pair and set `TAURI_SIGNING_PUBLIC_KEY`, `TAURI_SIGNING_PRIVATE_KEY`, and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as GitHub Actions secrets.
3. Push a semantic version tag such as `v0.1.0`.
4. Confirm the `Windows Alpha release` workflow builds the current-user NSIS installer, signs the updater artifacts, installs them on a clean Windows runner, and opens the installed executable.
5. Review the generated draft prerelease, attach these notes, and publish it only after the P0/P1 triage is empty.

The signing private key and password must never be stored in the repository, application database, diagnostic output, or workflow logs.

## Alpha validation

- Open, close, reorder, and restore multiple repository tabs.
- Stage, partially stage, discard, commit, and amend in a real repository.
- Create a stash with tracked and untracked files, apply it, and explicitly confirm before dropping it.
- Enter a merge conflict, resolve with ours and theirs, mark edited content resolved, and abort a second merge.
- Clone, fetch, fast-forward pull, push, cancel, and retry against HTTPS and SSH test remotes.
- Restart GitAcorn during an operation and verify the operation center marks it interrupted.
- Copy diagnostics and verify credentials, remote URL secrets, and file contents are absent.
- Test keyboard navigation and accessible names at 100%, 150%, and 200% Windows scaling.

## Known Alpha limits

- The first release targets Windows and requires system Git 2.40 or newer.
- GitAcorn uses the configured system credential helper and SSH agent; it does not store credentials.
- Conflict resolution offers ours, theirs, or staging the content edited in an external editor. An embedded merge editor is not included.
- Updater artifacts are generated and signed by CI. Automatic in-app update discovery is deferred until a stable public release endpoint exists.
