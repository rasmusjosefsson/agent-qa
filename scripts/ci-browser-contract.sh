#!/usr/bin/env bash
set -euo pipefail

: "${AQ:?set AQ to the installed agent-qa launcher}"
: "${WORK:?set WORK to an isolated writable directory}"

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
AB="$(dirname "$AQ")/agent-browser"
FIXTURE_DIR="$ROOT/test-fixtures/ci-browser"
SCENARIOS_DIR="$WORK/scenarios"
RECORD_DIR="$WORK/record"
BROWSER_HOME="${BROWSER_HOME:-$WORK/browser-home}"
SERVER_LOG="$WORK/fixture-server.log"
RECORD_SESSION="ci-browser-contract-record"
REPLAY_ONE_SESSION="ci-browser-contract-replay-one"
REPLAY_TWO_SESSION="ci-browser-contract-replay-two"
SERVER_PID=""

require_nonempty() {
  if [[ ! -s "$1" ]]; then
    printf 'expected nonempty file: %s\n' "$1" >&2
    exit 1
  fi
}

screenshot_same() {
  awk -v step="$1" '
    /^## screenshots$/ { screenshots = 1; next }
    screenshots && index($0, "| " step " | SAME |") == 1 { found = 1 }
    END { exit !found }
  ' "$2"
}

cleanup() {
  "$AB" --session "$RECORD_SESSION" close >/dev/null 2>&1 || true
  "$AB" --session "$REPLAY_ONE_SESSION" close >/dev/null 2>&1 || true
  "$AB" --session "$REPLAY_TWO_SESSION" close >/dev/null 2>&1 || true
  if [[ -n "$SERVER_PID" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

rm -rf "$WORK" "$BROWSER_HOME"
mkdir -p "$WORK" "$BROWSER_HOME"

if [[ ! -x "$AQ" ]]; then
  printf 'AQ is not executable: %s\n' "$AQ" >&2
  exit 1
fi
if [[ ! -x "$AB" ]]; then
  printf 'agent-browser is not installed beside AQ: %s\n' "$AB" >&2
  exit 1
fi
if [[ ! -f "$FIXTURE_DIR/index.html" ]]; then
  printf 'fixture missing: %s/index.html\n' "$FIXTURE_DIR" >&2
  exit 1
fi

export AGENT_QA_SCENARIOS_DIR="$SCENARIOS_DIR"
export AGENT_QA_RECORD_DIR="$RECORD_DIR"
export AGENT_BROWSER_HOME="$BROWSER_HOME"

"$AB" --session "$RECORD_SESSION" close >/dev/null 2>&1 || true
"$AB" --session "$REPLAY_ONE_SESSION" close >/dev/null 2>&1 || true
"$AB" --session "$REPLAY_TWO_SESSION" close >/dev/null 2>&1 || true
"$AB" install 2>&1 | tee "$WORK/agent-browser-install.log"
"$AQ" doctor 2>&1 | tee "$WORK/doctor.log"

PYTHONUNBUFFERED=1 python3 -m http.server 0 --bind 127.0.0.1 --directory "$FIXTURE_DIR" >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 50); do
  PORT=$(awk '/Serving HTTP/ { for (i = 1; i <= NF; i++) if ($i == "port") { print $(i + 1); exit } }' "$SERVER_LOG")
  if [[ -n "$PORT" ]]; then
    break
  fi
  sleep 0.1
done
: "${PORT:?fixture server did not report a port; see $SERVER_LOG}"
FIXTURE_URL="http://127.0.0.1:$PORT/"
for _ in $(seq 1 50); do
  if curl --fail --silent --show-error "$FIXTURE_URL" >"$WORK/fixture-response.html"; then
    break
  fi
  sleep 0.1
done
require_nonempty "$WORK/fixture-response.html"

"$AQ" start "complete the browser contract" --session "$RECORD_SESSION" 2>&1 | tee "$WORK/start.log"
"$AB" --session "$RECORD_SESSION" open "$FIXTURE_URL" 2>&1 | tee "$WORK/record-navigation.log"
"$AQ" record-step navigation "{\"route\":\"$FIXTURE_URL\",\"intent\":\"open the fixture\"}" 2>&1 | tee "$WORK/record-step-s0.log"
"$AB" --session "$RECORD_SESSION" find role button click --name Complete 2>&1 | tee "$WORK/record-action.log"
"$AQ" record-step action '{"method":"clickRole","args":["button","Complete"],"intent":"complete the fixture"}' 2>&1 | tee "$WORK/record-step-s1.log"
"$AQ" record-step assert '{"kind":"present","args":["status","Completed"],"intent":"completion status is visible"}' 2>&1 | tee "$WORK/record-step-s2.log"
"$AQ" flush 2>&1 | tee "$WORK/flush.log"

SID=$(tr -d '[:space:]' <"$RECORD_DIR/scenario.last")
: "${SID:?scenario.last did not contain a scenario id}"
SCENARIO_DIR="$SCENARIOS_DIR/$SID"
if [[ ! -f "$SCENARIO_DIR/scenario.json" ]]; then
  printf 'scenario missing: %s/scenario.json\n' "$SCENARIO_DIR" >&2
  exit 1
fi
mv "$SERVER_LOG" "$SCENARIO_DIR/fixture-server.log"
SERVER_LOG="$SCENARIO_DIR/fixture-server.log"

for STEP in s0 s1 s2; do
  require_nonempty "$SCENARIO_DIR/recording/screenshots/$STEP.png"
  require_nonempty "$SCENARIO_DIR/recording/snapshots/$STEP.txt"
done

"$AQ" replay "$SID" --session "$REPLAY_ONE_SESSION" --output-audit "$SCENARIO_DIR/audit-replay-one.json" 2>&1 | tee "$WORK/replay-one.log"
RUN_ONE=$(tr -d '[:space:]' <"$SCENARIO_DIR/replays/latest.txt")
: "${RUN_ONE:?first replay did not write latest.txt}"
"$AQ" replay "$SID" --session "$REPLAY_TWO_SESSION" --output-audit "$SCENARIO_DIR/audit-replay-two.json" 2>&1 | tee "$WORK/replay-two.log"
RUN_TWO=$(tr -d '[:space:]' <"$SCENARIO_DIR/replays/latest.txt")
: "${RUN_TWO:?second replay did not write latest.txt}"
if [[ "$RUN_ONE" == "$RUN_TWO" ]]; then
  printf 'replays reused a run id: %s\n' "$RUN_ONE" >&2
  exit 1
fi

for RUN in "$RUN_ONE" "$RUN_TWO"; do
  for STEP in s0 s1 s2; do
    require_nonempty "$SCENARIO_DIR/replays/$RUN/screenshots/$STEP.png"
    require_nonempty "$SCENARIO_DIR/replays/$RUN/snapshots/$STEP.txt"
  done
done

"$AQ" compare "$SID" "$RUN_ONE" "$RUN_TWO" --strict 2>&1 | tee "$WORK/compare.log"
COMPARE_MD=$(find "$SCENARIO_DIR/compare" -name compare.md -type f -print -quit)
: "${COMPARE_MD:?compare did not write compare.md}"
for STEP in s0 s1 s2; do
  if ! screenshot_same "$STEP" "$COMPARE_MD"; then
    printf 'expected SAME screenshot row for %s in %s\n' "$STEP" "$COMPARE_MD" >&2
    exit 1
  fi
done
"$AQ" audit show "$SID" "$RUN_TWO" >"$SCENARIO_DIR/audit.txt"
