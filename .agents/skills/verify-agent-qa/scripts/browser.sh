#!/usr/bin/env bash
# Drive the workbench browser session for the current verification run.
# Usage: browser.sh <agent-browser args...>
# Example: browser.sh open "$VERIFY_URL/cases"
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib.sh"
verify_load_run
NODE_DIR=$(dirname "$(verify_node)")
export PATH="$NODE_DIR:$PATH"
cd "$VERIFY_REPO"
exec node "$VERIFY_REPO/npm/agent-qa/bin/agent-qa.js" browser --session "$VERIFY_BROWSER_SESSION" "$@"
