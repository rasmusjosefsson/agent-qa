#!/usr/bin/env bash
# capture-demo.sh — record a headed `agent-qa replay` of a produced scenario and
# emit a PR-embeddable demo with a two-line caption bar.
#
# Usage:
#   evals/capture-demo.sh --scenario <dir-with-scenario.json> --out <dir> \
#       --line1 "Fixture demo" [--line2 "alert accepted end-to-end"] \
#       [--session name] [--width 1024]
#
# Result: <out>/demo.webm (full capture) and <out>/demo.webp (scaled, captioned,
# looped — embed in a PR comment/body as ![alt](path).
#
# Recording uses ffmpeg x11grab on $DISPLAY; under CI (no DISPLAY) the whole
# capture runs inside xvfb-run so the headed browser still renders.
set -euo pipefail

SCENARIO_DIR="" OUT_DIR="" SESSION="" WIDTH=1024
LINE1="" LINE2=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --scenario) SCENARIO_DIR="$2"; shift 2 ;;
    --out)      OUT_DIR="$2";      shift 2 ;;
    --session)  SESSION="$2";      shift 2 ;;
    --width)    WIDTH="$2";        shift 2 ;;
    --line1)    LINE1="$2";        shift 2 ;;
    --line2)    LINE2="$2";        shift 2 ;;
    -h|--help)  sed -n '2,14p' "$0"; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done
[[ -n "$SCENARIO_DIR" && -n "$OUT_DIR" ]] || { echo "need --scenario and --out" >&2; exit 2; }
SCENARIO_DIR="$(cd "$SCENARIO_DIR" && pwd)"
OUT_DIR="$(mkdir -p "$OUT_DIR" && cd "$OUT_DIR" && pwd)"
[[ -f "$SCENARIO_DIR/scenario.json" ]] || { echo "no scenario.json in $SCENARIO_DIR" >&2; exit 2; }
[[ -n "$LINE1" ]] || LINE1="$(basename "$SCENARIO_DIR")"

REPO="$(cd "$(dirname "$0")/.." && pwd)"
AGENT_QA_BIN="${AGENT_QA_BIN:-$REPO/cli/target/debug/agent-qa}"
[[ -x "$AGENT_QA_BIN" ]] || AGENT_QA_BIN="$(command -v agent-qa || true)"
[[ -n "$AGENT_QA_BIN" ]] || { echo "agent-qa binary not found" >&2; exit 2; }
# The Rust binary needs AGENT_BROWSER_BIN (it never walks node_modules; the npm
# launcher resolves it itself). Fall back to agent-browser on PATH.
if [[ -z "${AGENT_BROWSER_BIN:-}" ]] && [[ "$AGENT_QA_BIN" == */target/* ]]; then
  export AGENT_BROWSER_BIN="$(command -v agent-browser || true)"
fi
SESSION="${SESSION:-demo-$(basename "$SCENARIO_DIR" | tr -c 'a-zA-Z0-9' '-')}"

FONT=/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"; kill "${FFMPEG_PID:-0}" 2>/dev/null || true' EXIT

# Caption overlay: dark bar, two centered lines (line2 optional).
caption_filter() {
  local esc1 esc2
  esc1="${LINE1//\'/\\\\\'}"; esc2="${LINE2//\'/\\\\\'}"
  echo "drawbox=y=ih-96:w=iw:h=96:color=black@0.65:t=fill,"\
"drawtext=fontfile=$FONT:text='$esc1':fontcolor=white:fontsize=30:x=(w-text_w)/2:y=h-84,"\
"drawtext=fontfile=$FONT:text='$esc2':fontcolor=#d7d7d7:fontsize=22:x=(w-text_w)/2:y=h-44"
}

run_under_display() {
  # $1.. = command to run while recording
  local geom delay
  geom="$(xdpyinfo | awk '/dimensions/{print $2}')"
  ffmpeg -y -loglevel error -f x11grab -video_size "$geom" -framerate 15 \
      -i "$DISPLAY" -c:v libvpx-vp9 -crf 32 -b:v 0 -pix_fmt yuv420p \
      "$WORKDIR/capture.webm" &
  FFMPEG_PID=$!
  sleep 0.4
  set +e; "$@"; local rc=$?; set -e
  sleep 0.4
  kill -INT "$FFMPEG_PID" 2>/dev/null || true
  wait "$FFMPEG_PID" 2>/dev/null || true
  FFMPEG_PID=0
  return "$rc"
}

REPLAY_CMD=("$AGENT_QA_BIN" replay "$SCENARIO_DIR/scenario.json" --session "$SESSION" --headed)
if [[ -z "${DISPLAY:-}" ]]; then
  # xvfb provides the display; ffmpeg inside the same xvfb-run can't wrap just
  # the ffmpeg, so record inside a nested script on the virtual display.
  xvfb-run -a --server-args="-screen 0 1600x1200x24" bash -c '
    set -e
    ffmpeg -y -loglevel error -f x11grab -video_size 1600x1200 -framerate 15 \
      -i "$DISPLAY" -c:v libvpx-vp9 -crf 32 -b:v 0 -pix_fmt yuv420p "$1" &
    fpid=$!; sleep 0.4
    set +e; shift; "$@"; rc=$?; set -e
    kill -INT $fpid 2>/dev/null || true; wait $fpid 2>/dev/null || true
    exit $rc
  ' _ "$WORKDIR/capture.webm" "${REPLAY_CMD[@]}"
else
  run_under_display "${REPLAY_CMD[@]}"
fi

[[ -s "$WORKDIR/capture.webm" ]] || { echo "capture produced no video" >&2; exit 1; }
cp "$WORKDIR/capture.webm" "$OUT_DIR/demo.webm"
ffmpeg -y -loglevel error -i "$WORKDIR/capture.webm" \
  -vf "scale=${WIDTH}:-2,fps=12,$(caption_filter)" \
  -c:v libwebp -loop 0 -quality 70 "$OUT_DIR/demo.webp"
echo "demo: $OUT_DIR/demo.webp"
