#!/usr/bin/env bash
set -euo pipefail

: "${BASE_SHA:?set BASE_SHA to the pull request base commit}"
: "${HEAD_SHA:?set HEAD_SHA to the pull request head commit}"
: "${PR_BODY:=}"

if git diff --quiet "$BASE_SHA" "$HEAD_SHA" -- npm/agent-qa/web; then
  exit 0
fi

# Any embedded image counts — GitHub user-attachments, a Devin attachment
# proxy, a CDN link, or an <img> tag. The gate exists to force visual
# evidence, not to require one host (automation can't mint user-attachments
# URLs; they only come from the web editor).
if grep -Eq '!\[[^]]*\]\(https?://[^)]+\)|<img[^>]+src="https?://[^"]+"|https://github\.com/user-attachments/assets/[[:alnum:]-]+' <<<"$PR_BODY"; then
  exit 0
fi

printf '%s\n' \
  'Web UI changes need visual evidence in the PR body.' \
  'Attach a final-state image. Use a video when a still image cannot prove the interaction.' >&2
exit 1
