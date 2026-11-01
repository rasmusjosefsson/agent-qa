#!/usr/bin/env bash
# Stop the workbench and browser sessions this verification run started.
# Deletes /tmp/agent-qa-verify/<run-id>/. Does not delete /tmp/agent-qa-verify-evidence/.
# Usage: cleanup.sh [run-id] [--all]
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib.sh"

ALL=0
RUN_ID=""
for arg in "$@"; do
  if [[ "$arg" == "--all" ]]; then
    ALL=1
  else
    RUN_ID="$arg"
  fi
done

tmux_bin() {
  if [[ -f /exec-daemon/tmux.portal.conf ]]; then
    tmux -f /exec-daemon/tmux.portal.conf "$@"
  else
    tmux "$@"
  fi
}

stop_one() {
  local id="$1"
  local state="$VERIFY_BASE/$id"
  if [[ ! -d "$state" ]]; then
    echo "cleanup: no state for $id" >&2
    return 0
  fi
  if [[ -f "$state/env.sh" ]]; then
    # shellcheck disable=SC1090
    source "$state/env.sh"
    if [[ -n "${AGENT_BROWSER_BIN:-}" && -x "$AGENT_BROWSER_BIN" ]]; then
      AGENT_BROWSER_HOME="${AGENT_BROWSER_HOME:-}" "$AGENT_BROWSER_BIN" --session "${VERIFY_BROWSER_SESSION:-}" close >/dev/null 2>&1 || true
      AGENT_BROWSER_HOME="${AGENT_BROWSER_HOME:-}" "$AGENT_BROWSER_BIN" --session "${VERIFY_RECORD_SESSION:-}" close >/dev/null 2>&1 || true
    fi
    if [[ -n "${VERIFY_TMUX_SESSION:-}" ]]; then
      tmux_bin kill-session -t "=$VERIFY_TMUX_SESSION" >/dev/null 2>&1 || true
    fi
    if [[ -n "${VERIFY_RUN_ID:-}" ]]; then
      tmux_bin kill-session -t "=aqa-fixture-${VERIFY_RUN_ID}" >/dev/null 2>&1 || true
    fi
  fi
  if [[ -f "$state/fixture.pid" ]]; then
    local fpid
    fpid=$(tr -d '[:space:]' <"$state/fixture.pid")
    if [[ -n "$fpid" ]]; then
      kill "$fpid" >/dev/null 2>&1 || true
    fi
  fi
  if [[ -f "$state/server.pid" ]]; then
    local pid
    pid=$(tr -d '[:space:]' <"$state/server.pid")
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" >/dev/null 2>&1 || true
      for _ in 1 2 3 4 5 6 7 8 9 10; do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.2
      done
      if kill -0 "$pid" 2>/dev/null; then
        kill -9 "$pid" >/dev/null 2>&1 || true
      fi
    fi
  fi
  rm -rf "$state"
  if [[ -f "$VERIFY_BASE/current" ]] && [[ "$(tr -d '[:space:]' <"$VERIFY_BASE/current")" == "$id" ]]; then
    rm -f "$VERIFY_BASE/current"
  fi
  echo "cleanup: removed $state" >&2
  if [[ -d "$VERIFY_EVIDENCE_BASE/$id" ]]; then
    echo "cleanup: kept evidence $VERIFY_EVIDENCE_BASE/$id" >&2
  fi
}

if [[ "$ALL" == 1 ]]; then
  shopt -s nullglob
  for dir in "$VERIFY_BASE"/*; do
    [[ -d "$dir" ]] || continue
    stop_one "$(basename "$dir")"
  done
  exit 0
fi

if [[ -z "$RUN_ID" ]]; then
  if [[ ! -f "$VERIFY_BASE/current" ]]; then
    echo "cleanup: no current run" >&2
    exit 0
  fi
  RUN_ID=$(tr -d '[:space:]' <"$VERIFY_BASE/current")
fi
stop_one "$RUN_ID"
