# GitAcorn Alpha release

## Release flow

1. Run `pnpm release:check` from the repository root. This uses the same quality gate as CI and creates an unsigned local bundle for the current OS.
2. Push a semantic version tag such as `v0.1.0`.
3. Confirm the `Desktop Alpha release` workflow produces all release targets:
   - Unsigned Windows current-user NSIS installer, followed by a clean-runner install/start smoke test.
   - Universal macOS app/DMG with an ad-hoc signature, followed by signature, dual-architecture, and launch checks.
   - Unsigned Linux Debian package and AppImage built on Ubuntu 22.04, followed by package-content and headless launch checks.
4. Review the generated draft prerelease, attach these notes, and publish it only after the P0/P1 triage is empty.

The Alpha workflow does not require signing or notarization secrets. Do not add certificates, private keys, or account passwords to the repository, application database, diagnostic output, or workflow logs.

## Alpha validation

- Open, close, reorder, and restore multiple repository tabs.
- Stage, partially stage, discard, commit, and amend in a real repository.
- Create a stash with tracked and untracked files, apply it, and explicitly confirm before dropping it.
- Enter a merge conflict, resolve with ours and theirs, mark edited content resolved, and abort a second merge.
- Clone, fetch, fast-forward pull, push, cancel, and retry against HTTPS and SSH test remotes.
- Restart GitAcorn during an operation and verify the operation center marks it interrupted.
- Copy diagnostics and verify credentials, remote URL secrets, and file contents are absent.
- Test keyboard navigation and accessible names at 100%, 150%, and 200% Windows scaling.
- On both Intel and Apple Silicon Macs, open a repository from Finder-launched GitAcorn and verify system Git discovery, file watching, credential helper/SSH agent access, and keyboard navigation.
- On Ubuntu 22.04 or Debian 12, install the Debian package and run the AppImage; verify system Git discovery, file watching, credential helper/SSH agent access, and keyboard navigation.

## Known Alpha limits

- Windows, macOS, and Linux Alpha builds require system Git 2.40 or newer.
- The macOS Alpha is a universal binary for macOS 11 or newer with an ad-hoc signature. It is not notarized, so users must explicitly allow it in Privacy & Security before first launch.
- Linux x86_64 builds use Ubuntu 22.04 as their glibc/WebKitGTK baseline and are distributed as a Debian package and AppImage.
- GitAcorn uses the configured system credential helper and SSH agent; it does not store credentials.
- Conflict resolution offers ours, theirs, or staging the content edited in an external editor. An embedded merge editor is not included.
- Updater artifacts are not generated because Tauri requires them to be signed. Automatic in-app update discovery is deferred until signed releases are introduced.
