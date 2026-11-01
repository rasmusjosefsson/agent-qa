#!/usr/bin/env bash
# Run the agent-qa launcher against this verification run's scenarios root.
# Usage: aq.sh <agent-qa args...>
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib.sh"
verify_load_run
NODE_DIR=$(dirname "$(verify_node)")
export PATH="$NODE_DIR:$PATH"
cd "$VERIFY_REPO"
exec node "$VERIFY_REPO/npm/agent-qa/bin/agent-qa.js" "$@"
