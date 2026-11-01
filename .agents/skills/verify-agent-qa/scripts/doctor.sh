#!/usr/bin/env bash
# Read-only check: is the workbench this run started worth driving?
# Exit 0 and print "ok <url>" or exit 1 with the failing checks on stderr.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib.sh"
verify_load_run "${1:-}"

fail=0
note() {
  echo "doctor: $*" >&2
  fail=1
}

pid=""
if [[ -f "$VERIFY_STATE/server.pid" ]]; then
  pid=$(tr -d '[:space:]' <"$VERIFY_STATE/server.pid")
fi
if [[ -z "$pid" ]] || ! kill -0 "$pid" 2>/dev/null; then
  note "server pid ${pid:-missing} is not running (state $VERIFY_STATE)"
else
  echo "doctor: pid $pid alive" >&2
fi

if ! curl --fail --silent --show-error --max-time 5 "${VERIFY_URL}/api/root" >"$VERIFY_STATE/doctor-root.json"; then
  note "GET ${VERIFY_URL}/api/root failed"
else
  python3 - "$VERIFY_STATE/doctor-root.json" "$VERIFY_ROOT" <<'PY' || note "api/root did not match this run"
import json, sys
data = json.load(open(sys.argv[1]))
root = sys.argv[2]
errors = []
if data.get("scenariosRoot") != root:
    errors.append(f"scenariosRoot {data.get('scenariosRoot')!r} != {root!r}")
if data.get("editor") is not True:
    errors.append(f"editor is not available ({data.get('editor')!r})")
if errors:
    raise SystemExit("; ".join(errors))
print(f"doctor: scenarios root {root}", file=sys.stderr)
print("doctor: editor available", file=sys.stderr)
PY
fi

if ! curl --fail --silent --show-error --max-time 5 "${VERIFY_URL}/api/version" >"$VERIFY_STATE/doctor-version.json"; then
  note "GET ${VERIFY_URL}/api/version failed"
else
  python3 - "$VERIFY_STATE/doctor-version.json" <<'PY' || note "api/version missing"
import json, sys
data = json.load(open(sys.argv[1]))
version = data.get("version")
if not isinstance(version, str) or not version:
    raise SystemExit(f"version {version!r}")
print(f"doctor: version {version}", file=sys.stderr)
PY
fi

if ! curl --fail --silent --show-error --max-time 5 "$VERIFY_URL" | grep -q '/assets/'; then
  note "GET $VERIFY_URL did not serve the built UI (/assets/ missing)"
else
  echo "doctor: UI bundle served at $VERIFY_URL" >&2
fi

if [[ -n "$pid" ]] && command -v lsof >/dev/null 2>&1; then
  owners=$(lsof -nP -iTCP:"$VERIFY_PORT" -sTCP:LISTEN -t 2>/dev/null | sort -u || true)
  if [[ -z "$owners" ]]; then
    note "nothing is listening on port $VERIFY_PORT"
  elif ! grep -qx "$pid" <<<"$owners"; then
    note "port $VERIFY_PORT is owned by pid(s) $owners, not $pid"
  else
    echo "doctor: port $VERIFY_PORT owned by pid $pid" >&2
  fi
fi

if ! "$VERIFY_REPO/cli/target/debug/agent-qa" doctor --json >"$VERIFY_STATE/doctor-cli.json"; then
  note "agent-qa doctor --json failed"
else
  python3 - "$VERIFY_STATE/doctor-cli.json" <<'PY' || note "agent-qa doctor probe failed"
import json, sys
data = json.load(open(sys.argv[1]))
bad = [p for p in data.get("probes", []) if p.get("topic") == "agent-browser" and not p.get("ok")]
if bad:
    raise SystemExit(bad[0].get("detail"))
print("doctor: agent-browser probe ok", file=sys.stderr)
PY
fi

if [[ "$fail" != 0 ]]; then
  echo "doctor: not worth driving" >&2
  exit 1
fi
echo "ok $VERIFY_URL"
