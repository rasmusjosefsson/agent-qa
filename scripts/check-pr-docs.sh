#!/usr/bin/env bash
# Fail a pull request that changes reader-visible code without a doc update
# or an explicit Docs-exempt line. Same shape as check-pr-visual-evidence.sh:
# diff the two SHAs, then read the PR body.
set -euo pipefail

: "${BASE_SHA:?set BASE_SHA to the pull request base commit}"
: "${HEAD_SHA:?set HEAD_SHA to the pull request head commit}"
: "${PR_BODY:=}"

# True when this path is product code a reader can notice.
# Web and launcher tests are excluded. Rust tests live in the same .rs
# files as the verb, so a test-only edit there still needs Docs-exempt.
is_product() {
  local f=$1
  case "$f" in
    *.test.ts|*.test.tsx|*.test.js|*.test.mjs|*.spec.ts|*.spec.tsx) return 1 ;;
  esac
  case "$f" in
    cli/src/*) [[ "$f" == *.rs ]] ;;
    npm/agent-qa/bin/*) return 0 ;;
    npm/agent-qa/lib/*) return 0 ;;
    npm/agent-qa/web/src/*) return 0 ;;
    *) return 1 ;;
  esac
}

# True when this path is a page a reader would open.
# Internal trackers (gap-map, qa-playground, specs) and the changelog
# do not satisfy the gate.
is_docs() {
  local f=$1
  case "$f" in
    docs/gap-map.md|docs/qa-playground.md|docs/specs/*) return 1 ;;
    docs/*.md|README.md|CONTRIBUTING.md) return 0 ;;
    skill-data/*.md|site/src/content/*.md|site/src/content/*.mdx) return 0 ;;
    *) return 1 ;;
  esac
}

# A real exemption is its own line:
#   Docs-exempt: <reason of at least 12 characters>
# HTML-comment examples in the template do not match.
has_exemption() {
  local line reason
  while IFS= read -r line || [[ -n "$line" ]]; do
    line=${line%$'\r'}
    [[ "$line" == Docs-exempt:* ]] || continue
    reason=${line#Docs-exempt:}
    reason=${reason#"${reason%%[![:space:]]*}"}
    reason=${reason%"${reason##*[![:space:]]}"}
    [[ ${#reason} -ge 12 ]] || continue
    [[ "$reason" == *"<!--"* ]] && continue
    [[ "$reason" =~ [A-Za-z] ]] || continue
    return 0
  done <<<"$PR_BODY"
  return 1
}

product=()
while IFS= read -r path; do
  [[ -n "$path" ]] || continue
  if is_docs "$path"; then
    exit 0
  fi
  if is_product "$path"; then
    product+=("$path")
  fi
done < <(git diff --name-only "$BASE_SHA" "$HEAD_SHA")

if [[ ${#product[@]} -eq 0 ]]; then
  exit 0
fi

if has_exemption; then
  exit 0
fi

{
  echo "This pull request changes reader-visible code and does not update the docs."
  echo "Update a page a reader would open (docs/*.md other than gap-map.md, qa-playground.md, and docs/specs/; skill-data/; README.md; CONTRIBUTING.md; site/src/content/), or add a line on its own:"
  echo
  echo "  Docs-exempt: <why a reader would not notice>"
  echo
  echo "The reason needs at least 12 characters. CHANGELOG.md does not satisfy the check."
  echo
  echo "Product files in this diff:"
  printf '  %s\n' "${product[@]}"
} >&2
exit 1
