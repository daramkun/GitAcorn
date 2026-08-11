#!/usr/bin/env bash

set -euo pipefail

appimage="${1:?usage: smoke-linux-appimage.sh <path-to-AppImage>}"
smoke_seconds="${GIT_ACORN_SMOKE_SECONDS:-8}"
log_root="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"
log_path="$log_root/git-acorn-appimage-smoke.log"
process_id=""

cleanup() {
  if [[ -n "$process_id" ]]; then
    kill -KILL -- "-$process_id" 2>/dev/null || true
    wait "$process_id" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

chmod +x "$appimage"
setsid env APPIMAGE_EXTRACT_AND_RUN=1 xvfb-run -a "$appimage" >"$log_path" 2>&1 &
process_id=$!
sleep "$smoke_seconds"

if ! kill -0 "$process_id" 2>/dev/null; then
  cat "$log_path"
  exit 1
fi

echo "AppImage remained running for ${smoke_seconds}s"
