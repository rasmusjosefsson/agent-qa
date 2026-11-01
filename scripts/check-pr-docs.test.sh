#!/usr/bin/env bash
# Exercises scripts/check-pr-docs.sh against a throwaway git repo.
# Each case is a single commit on top of the same base, so earlier
# doc edits do not leak into the next diff.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
script=$root/scripts/check-pr-docs.sh
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

export GIT_AUTHOR_NAME=docs-check GIT_AUTHOR_EMAIL=docs-check@example.com
export GIT_COMMITTER_NAME=docs-check GIT_COMMITTER_EMAIL=docs-check@example.com

git -C "$work" init -q
mkdir -p "$work/cli/src" "$work/npm/agent-qa/web/src" "$work/npm/agent-qa/lib" \
  "$work/docs/specs" "$work/skill-data/core" "$work/site/src/content"

echo 'fn main() {}' >"$work/cli/src/main.rs"
echo '# readme' >"$work/README.md"
echo '# verbs' >"$work/docs/verbs.md"
echo '# gap' >"$work/docs/gap-map.md"
echo '# spec' >"$work/docs/specs/note.md"
echo '# heal' >"$work/skill-data/core/heal.md"
echo 'export const x = 1' >"$work/npm/agent-qa/web/src/app.tsx"
echo 'export const t = 1' >"$work/npm/agent-qa/web/src/app.test.ts"
echo 'export const lib = 1' >"$work/npm/agent-qa/lib/report-server.js"
git -C "$work" add .
git -C "$work" commit -q -m base
base=$(git -C "$work" rev-parse HEAD)

at_base() {
  git -C "$work" checkout -q -B case "$base"
}

snap() {
  git -C "$work" add "$@"
  git -C "$work" commit -q -m case
  git -C "$work" rev-parse HEAD
}

run_check() {
  local head=$1 body=$2
  (
    cd "$work"
    BASE_SHA=$base HEAD_SHA=$head PR_BODY=$body "$script"
  )
}

expect_fail() {
  local head=$1 body=$2 label=$3
  if run_check "$head" "$body"; then
    echo "expected failure: $label" >&2
    exit 1
  fi
}

expect_pass() {
  local head=$1 body=$2 label=$3
  if ! run_check "$head" "$body"; then
    echo "expected pass: $label" >&2
    exit 1
  fi
}

at_base
echo 'export const t = 2' >"$work/npm/agent-qa/web/src/app.test.ts"
head=$(snap npm/agent-qa/web/src/app.test.ts)
expect_pass "$head" '' 'web test only'

at_base
echo 'fn main() { println!("x"); }' >"$work/cli/src/main.rs"
head=$(snap cli/src/main.rs)
expect_fail "$head" '' 'cli without docs'
expect_fail "$head" $'<!-- Docs-exempt: tests only, no reader-facing change -->' 'commented exemption'
expect_fail "$head" 'Docs-exempt: n/a' 'short exemption'
expect_pass "$head" 'Docs-exempt: tests only, no reader-facing change' 'real exemption'
expect_pass "$head" $'## Docs\n\nDocs-exempt: internal refactor, no reader-facing change\n' 'exemption under a heading'

at_base
echo 'fn main() { println!("x"); }' >"$work/cli/src/main.rs"
echo '# gap 2' >"$work/docs/gap-map.md"
echo '# spec 2' >"$work/docs/specs/note.md"
echo '# log' >"$work/CHANGELOG.md"
head=$(snap cli/src/main.rs docs/gap-map.md docs/specs/note.md CHANGELOG.md)
expect_fail "$head" '' 'internal notes'

at_base
echo 'fn main() { println!("x"); }' >"$work/cli/src/main.rs"
echo '# verbs 2' >"$work/docs/verbs.md"
head=$(snap cli/src/main.rs docs/verbs.md)
expect_pass "$head" '' 'docs page'

at_base
echo 'fn main() { println!("y"); }' >"$work/cli/src/main.rs"
echo '# heal 2' >"$work/skill-data/core/heal.md"
head=$(snap cli/src/main.rs skill-data/core/heal.md)
expect_pass "$head" '' 'skill reference'

at_base
echo 'export const lib = 2' >"$work/npm/agent-qa/lib/report-server.js"
head=$(snap npm/agent-qa/lib/report-server.js)
expect_fail "$head" '' 'launcher without docs'

at_base
echo 'export const x = 2' >"$work/npm/agent-qa/web/src/app.tsx"
echo '# page' >"$work/site/src/content/quickstart.mdx"
head=$(snap npm/agent-qa/web/src/app.tsx site/src/content/quickstart.mdx)
expect_pass "$head" '' 'hand-written site page'

echo 'check-pr-docs: ok'
