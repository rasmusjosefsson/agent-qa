#!/usr/bin/env bash
# Example pre-commit hook: schema-validate + lint every staged
# scenario.json. Drop into .git/hooks/pre-commit and chmod +x.
#
# Fails the commit iff any staged scenario has a schema or lint error.
# Warnings (empty-intent, bare-do, unused-input) don't gate — add
# --strict to gate on those too.

set -euo pipefail

# Find staged scenario.json paths.
mapfile -t files < <(git diff --cached --name-only --diff-filter=ACM \
  | grep -E '(^|/)scenario\.json$' || true)

if [ "${#files[@]}" -eq 0 ]; then
  exit 0
fi

# Preflight: bail early if the harness can't write its scratch paths.
if ! agent-qa info --check >/dev/null 2>&1; then
  echo "agent-qa pre-commit: 'info --check' failed; run \`agent-qa info --check\` to see why."
  exit 1
fi

echo "agent-qa pre-commit: ${#files[@]} staged scenario(s)"
failed=0
for f in "${files[@]}"; do
  if ! agent-qa scenario check "$f"; then
    failed=$((failed + 1))
  fi
done

if [ "$failed" -gt 0 ]; then
  echo
  echo "agent-qa pre-commit: $failed scenario(s) failed validate or lint."
  echo "Fix and re-stage, or commit with --no-verify to skip this hook."
  exit 1
fi
