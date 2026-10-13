#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
mkdir -p .bench/security
export GDK_BACKEND=x11
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
"${TELEX_SECURITY_BINARY:-${CARGO_TARGET_DIR:-target}/debug/telex}" >.bench/security/webview-linux.log 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT
for ((i=0; i<900; i++)); do
  kill -0 "$pid" 2>/dev/null || { echo 'Security webview exited without a result'; exit 1; }
  title="$(xdotool search --all --pid "$pid" --name 'TELEX_SECURITY_(RESULT|ERROR):' 2>/dev/null | head -1 | xargs -r xdotool getwindowname || true)"
  case "$title" in
    TELEX_SECURITY_ERROR:*) echo "$title"; exit 1 ;;
    TELEX_SECURITY_RESULT:*)
      printf '%s\n' "${title#TELEX_SECURITY_RESULT:}" >.bench/security/webview-linux.json
      node -e 'const fs=require("fs"), r=JSON.parse(fs.readFileSync(".bench/security/webview-linux.json")); if(r.status!=="passed"||r.cases<14||!r.sandbox) process.exit(1); console.log("WebKitGTK: "+r.cases+" hostile-email render cases and sandbox execution regression passed.");'
      exit 0 ;;
  esac
  sleep .1
done
echo 'Security webview timed out'; exit 1
