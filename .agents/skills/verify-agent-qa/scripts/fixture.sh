#!/usr/bin/env bash
# Serve test-fixtures/ci-browser for record/replay proofs.
# Usage: fixture.sh start | url | stop
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib.sh"
verify_load_run

FIXTURE_DIR="$VERIFY_REPO/test-fixtures/ci-browser"
LOG="$VERIFY_STATE/fixture.log"
PIDFILE="$VERIFY_STATE/fixture.pid"
URLFILE="$VERIFY_STATE/fixture.url"
FIXTURE_SESSION="aqa-fixture-${VERIFY_RUN_ID}"

tmux_bin() {
  if [[ -f /exec-daemon/tmux.portal.conf ]]; then
    tmux -f /exec-daemon/tmux.portal.conf "$@"
  else
    tmux "$@"
  fi
}

cmd="${1:-}"
case "$cmd" in
  start)
    if [[ ! -f "$FIXTURE_DIR/index.html" ]]; then
      echo "fixture: missing $FIXTURE_DIR/index.html" >&2
      exit 1
    fi
    if [[ -f "$PIDFILE" ]] && kill -0 "$(tr -d '[:space:]' <"$PIDFILE")" 2>/dev/null; then
      echo "fixture: already running ($(tr -d '[:space:]' <"$URLFILE"))" >&2
      exit 1
    fi
    : >"$LOG"
    tmux_bin new-session -d -s "$FIXTURE_SESSION" -- bash -lc "exec env PYTHONUNBUFFERED=1 python3 -m http.server 0 --bind 127.0.0.1 --directory $(printf '%q' "$FIXTURE_DIR") > $(printf '%q' "$LOG") 2>&1"
    tmux_bin list-panes -t "$FIXTURE_SESSION" -F '#{pane_pid}' | head -1 >"$PIDFILE"
    url=""
    for _ in $(seq 1 50); do
      port=$(awk '/Serving HTTP/ { for (i = 1; i <= NF; i++) if ($i == "port") { print $(i + 1); exit } }' "$LOG" 2>/dev/null || true)
      if [[ -n "$port" ]]; then
        url="http://127.0.0.1:${port}/"
        break
      fi
      sleep 0.1
    done
    if [[ -z "$url" ]]; then
      echo "fixture: server did not report a port; see $LOG" >&2
      tmux_bin kill-session -t "=$FIXTURE_SESSION" >/dev/null 2>&1 || true
      exit 1
    fi
    printf '%s\n' "$url" >"$URLFILE"
    curl --fail --silent --show-error "$url" >/dev/null
    printf '%s\n' "$url"
    ;;
  url)
    if [[ ! -s "$URLFILE" ]]; then
      echo "fixture: not started" >&2
      exit 1
    fi
    tr -d '[:space:]' <"$URLFILE"
    echo
    ;;
  stop)
    tmux_bin kill-session -t "=$FIXTURE_SESSION" >/dev/null 2>&1 || true
    if [[ -f "$PIDFILE" ]]; then
      pid=$(tr -d '[:space:]' <"$PIDFILE")
      if [[ -n "$pid" ]]; then
        kill "$pid" >/dev/null 2>&1 || true
      fi
      rm -f "$PIDFILE"
    fi
    ;;
  *)
    echo "usage: fixture.sh start | url | stop" >&2
    exit 2
    ;;
esac
