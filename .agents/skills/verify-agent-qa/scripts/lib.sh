#!/usr/bin/env bash
# Shared paths for the agent-qa verification scripts. Source this; do not execute it.
set -euo pipefail

verify_root() {
  local d
  d=$(cd "$(dirname "${BASH_SOURCE[1]:-${BASH_SOURCE[0]}}")" && pwd)
  while [[ "$d" != "/" ]]; do
    if [[ -f "$d/cli/Cargo.toml" && -f "$d/npm/agent-qa/bin/agent-qa.js" ]]; then
      printf '%s\n' "$d"
      return 0
    fi
    d=$(dirname "$d")
  done
  echo "verify-agent-qa: repo root not found from ${BASH_SOURCE[0]}" >&2
  return 1
}

VERIFY_REPO="$(verify_root)"
VERIFY_BASE="/tmp/agent-qa-verify"
VERIFY_EVIDENCE_BASE="/tmp/agent-qa-verify-evidence"
VERIFY_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Node 24+ binary. /exec-daemon/node on the cloud image is Node 22 and wins PATH,
# which agent-browser rejects. Prefer an already-good node, then nvm's v24.
verify_node() {
  local candidate major
  if command -v node >/dev/null 2>&1; then
    candidate=$(command -v node)
    major=$("$candidate" -p 'process.versions.node.split(".")[0]')
    if [[ "$major" -ge 24 ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  fi
  local nvm_node
  nvm_node=$(ls -d "${HOME}/.nvm/versions/node"/v24.*/bin/node 2>/dev/null | sort -V | tail -1 || true)
  if [[ -n "${nvm_node}" && -x "${nvm_node}" ]]; then
    printf '%s\n' "$nvm_node"
    return 0
  fi
  if [[ -s "${HOME}/.nvm/nvm.sh" ]]; then
    # shellcheck disable=SC1091
    . "${HOME}/.nvm/nvm.sh"
    nvm install 24 >/dev/null
    nvm_node=$(ls -d "${HOME}/.nvm/versions/node"/v24.*/bin/node 2>/dev/null | sort -V | tail -1 || true)
    if [[ -n "${nvm_node}" && -x "${nvm_node}" ]]; then
      printf '%s\n' "$nvm_node"
      return 0
    fi
  fi
  echo "verify-agent-qa: Node.js >= 24 is required (agent-browser). node on PATH is $(node -v 2>/dev/null || echo missing)." >&2
  return 1
}

verify_load_run() {
  local id="${1:-}"
  if [[ -z "$id" ]]; then
    if [[ ! -f "$VERIFY_BASE/current" ]]; then
      echo "verify-agent-qa: no current run. Start one with scripts/launch.sh" >&2
      return 1
    fi
    id=$(tr -d '[:space:]' <"$VERIFY_BASE/current")
  fi
  local env_file="$VERIFY_BASE/$id/env.sh"
  if [[ ! -f "$env_file" ]]; then
    echo "verify-agent-qa: missing $env_file" >&2
    return 1
  fi
  # shellcheck disable=SC1090
  source "$env_file"
}
