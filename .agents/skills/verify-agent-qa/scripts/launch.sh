#!/usr/bin/env bash
# Start an isolated agent-qa workbench. Prints VERIFY_URL on stdout when ready.
# State (deleted by cleanup.sh): /tmp/agent-qa-verify/<run-id>/
# Evidence (kept):               /tmp/agent-qa-verify-evidence/<run-id>/
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib.sh"

NODE=$(verify_node)
NODE_DIR=$(dirname "$NODE")
export PATH="$NODE_DIR:$PATH"

# Always a new run. A sourced env.sh from an earlier run must not reuse that id:
# a failed launch deletes the state directory for the id it picked.
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-${RANDOM}-$$"
STATE="$VERIFY_BASE/$RUN_ID"
ROOT="$STATE/scenarios"
RECORD="$STATE/record"
BROWSER_HOME="$STATE/browser-home"
EVIDENCE="$VERIFY_EVIDENCE_BASE/$RUN_ID"
SESSION="aqa-verify-$RUN_ID"
BROWSER_SESSION="aqa-workbench-$RUN_ID"
RECORD_SESSION="aqa-record-$RUN_ID"
LAUNCH_OK=0

mkdir -p "$ROOT" "$RECORD" "$BROWSER_HOME" "$EVIDENCE" "$STATE"

port_busy() {
  python3 - "$1" <<'PY'
import socket, sys
s = socket.socket()
s.settimeout(0.2)
code = s.connect_ex(("127.0.0.1", int(sys.argv[1])))
sys.exit(0 if code == 0 else 1)
PY
}

PORT=7879
while port_busy "$PORT"; do
  PORT=$((PORT + 1))
  if [[ "$PORT" -gt 7900 ]]; then
    echo "verify-agent-qa: no free port in 7879-7900" >&2
    exit 1
  fi
done

fail_launch() {
  if [[ "$LAUNCH_OK" == 1 ]]; then
    return 0
  fi
  if [[ -f "$STATE/server.pid" ]]; then
    kill "$(tr -d '[:space:]' <"$STATE/server.pid")" >/dev/null 2>&1 || true
  fi
  if [[ -f /exec-daemon/tmux.portal.conf ]]; then
    tmux -f /exec-daemon/tmux.portal.conf kill-session -t "=$SESSION" >/dev/null 2>&1 || true
  else
    tmux kill-session -t "=$SESSION" >/dev/null 2>&1 || true
  fi
  rm -rf "$STATE"
}
trap fail_launch EXIT

BIN="$VERIFY_REPO/cli/target/debug/agent-qa"
if [[ ! -x "$BIN" ]]; then
  echo "verify-agent-qa: building cli (cargo build --locked)" >&2
  if ! cargo build --locked --manifest-path "$VERIFY_REPO/cli/Cargo.toml"; then
    echo "verify-agent-qa: cargo build failed; installing stable and retrying" >&2
    rustup toolchain install stable --profile minimal
    rustup run stable cargo build --locked --manifest-path "$VERIFY_REPO/cli/Cargo.toml"
  fi
fi
if [[ ! -x "$BIN" ]]; then
  echo "verify-agent-qa: binary missing at $BIN" >&2
  exit 1
fi

AB_PKG="$VERIFY_REPO/npm/agent-qa/node_modules/agent-browser/package.json"
if [[ ! -f "$AB_PKG" ]]; then
  echo "verify-agent-qa: installing npm/agent-qa dependencies (agent-browser)" >&2
  (cd "$VERIFY_REPO/npm/agent-qa" && npm install --omit=optional --no-package-lock --ignore-scripts)
fi
AB="$VERIFY_REPO/npm/agent-qa/node_modules/agent-browser/bin/agent-browser-linux-x64"
if [[ ! -x "$AB" ]]; then
  echo "verify-agent-qa: agent-browser native binary missing at $AB" >&2
  exit 1
fi

if [[ ! -f "$VERIFY_REPO/npm/agent-qa/lib/public/index.html" ]]; then
  echo "verify-agent-qa: building workbench UI (npm run build:web)" >&2
  (cd "$VERIFY_REPO/npm/agent-qa" && npm --prefix web ci && npm --prefix web run build)
fi

echo "verify-agent-qa: ensuring Chrome for Testing (cached under ~/.agent-browser)" >&2
"$AB" install

cat >"$STATE/run-server.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
exec > >(tee -a "$STATE/server.log") 2>&1
echo \$\$ >"$STATE/server.pid"
export PATH="$NODE_DIR:\$PATH"
export AGENT_QA_BINARY_PATH="$BIN"
export AGENT_QA_SCENARIOS_DIR="$ROOT"
export AGENT_QA_RECORD_DIR="$RECORD"
export AGENT_BROWSER_HOME="$BROWSER_HOME"
export AGENT_BROWSER_BIN="$AB"
cd "$VERIFY_REPO"
exec "$NODE" npm/agent-qa/bin/agent-qa.js web --port $PORT --root "$ROOT" --no-open
EOF
chmod +x "$STATE/run-server.sh"

TMUX=(tmux)
if [[ -f /exec-daemon/tmux.portal.conf ]]; then
  TMUX=(tmux -f /exec-daemon/tmux.portal.conf)
fi
"${TMUX[@]}" new-session -d -s "$SESSION" -c "$VERIFY_REPO" -- bash "$STATE/run-server.sh"

URL="http://127.0.0.1:${PORT}"
ready=0
for _ in $(seq 1 100); do
  if [[ -f "$STATE/server.log" ]] && grep -q "serving:" "$STATE/server.log"; then
    ready=1
    break
  fi
  if [[ -f "$STATE/server.pid" ]] && ! kill -0 "$(tr -d '[:space:]' <"$STATE/server.pid")" 2>/dev/null; then
    echo "verify-agent-qa: server exited before it was ready" >&2
    [[ -f "$STATE/server.log" ]] && cat "$STATE/server.log" >&2
    exit 1
  fi
  sleep 0.1
done
if [[ "$ready" != 1 ]]; then
  echo "verify-agent-qa: timed out waiting for 'serving:'" >&2
  [[ -f "$STATE/server.log" ]] && cat "$STATE/server.log" >&2
  "${TMUX[@]}" capture-pane -t "$SESSION" -p >&2 || true
  exit 1
fi

# Usable means the UI shell is served, not only that the process accepted a socket.
html=$(curl --fail --silent --show-error "$URL")
if ! grep -q '/assets/' <<<"$html"; then
  echo "verify-agent-qa: $URL did not serve the built UI bundle" >&2
  printf '%s\n' "$html" >&2
  exit 1
fi
root_json=$(curl --fail --silent --show-error "${URL}/api/root")
python3 - "$root_json" "$ROOT" <<'PY'
import json, sys
body, root = sys.argv[1], sys.argv[2]
data = json.loads(body)
if data.get("scenariosRoot") != root:
    raise SystemExit(f"scenariosRoot {data.get('scenariosRoot')!r} != {root!r}")
if data.get("editor") is not True:
    raise SystemExit(f"editor unavailable: {data!r}")
PY

cat >"$STATE/env.sh" <<EOF
export VERIFY_RUN_ID="$RUN_ID"
export VERIFY_STATE="$STATE"
export VERIFY_ROOT="$ROOT"
export VERIFY_RECORD="$RECORD"
export VERIFY_PORT="$PORT"
export VERIFY_URL="$URL"
export VERIFY_EVIDENCE="$EVIDENCE"
export VERIFY_TMUX_SESSION="$SESSION"
export VERIFY_BROWSER_SESSION="$BROWSER_SESSION"
export VERIFY_RECORD_SESSION="$RECORD_SESSION"
export VERIFY_REPO="$VERIFY_REPO"
export AGENT_QA_BINARY_PATH="$BIN"
export AGENT_QA_SCENARIOS_DIR="$ROOT"
export AGENT_QA_RECORD_DIR="$RECORD"
export AGENT_BROWSER_HOME="$BROWSER_HOME"
export AGENT_BROWSER_BIN="$AB"
export PATH="$NODE_DIR:\$PATH"
EOF

printf '%s\n' "$RUN_ID" >"$VERIFY_BASE/current"
LAUNCH_OK=1
trap - EXIT
printf '%s\n' "$URL"
