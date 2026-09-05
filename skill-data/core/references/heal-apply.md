# `heal-apply` — patch one active recording-buffer value

`heal-apply` consumes a manual `heal-respond` value correction and patches one
row in the active recording buffer. It is disk bookkeeping only: it never
changes the live browser.

## Usage

```bash
agent-qa heal-apply <sid> --step <stepId> \
  [--run <runId>] \
  [--target-step <stepIndex-or-stepId>] \
  [--dry-run]
```

- `--run` defaults to `<sid>/replays/latest.txt`.
- `--step` identifies the response file to consume.
- `--target-step` chooses the buffer row; it defaults to the same step id and
  also accepts a zero-based numeric row index.
- `--dry-run` prints the planned change without writing or renaming files.

## Preconditions

Create a string value-correction response first:

```bash
agent-qa heal-respond <sid> --run <runId> --step <stepId> \
  --value '<corrected-string>' --rationale '<why>'
```

The response must exist at:

```text
<sid>/replays/<runId>/heal-responses/<stepId>.json
```

An active `<record_root>/recorder-state.json` file must exist. `heal-apply`
does not require or consume a `heal-requests/` file.

## What changes

On a non-dry run, `heal-apply`:

1. finds the target direct step by exact `stepId` or numeric index;
2. replaces its literal string value;
3. renames the response to `<stepId>.applied.json` so replay override loading
   skips it;
4. appends a `caller-driven-resolved` row to
   `<sid>/recording/heal.jsonl`.

It does not inspect app validation, infer a target from GraphQL payloads, apply
a hash/rebase guard, truncate later rows, or drive the live tab.

## After applying

Re-position the browser at the target row's pre-state, re-issue the corrected
fill/action, and continue serial recording. If later rows are no longer valid,
use `agent-qa truncate <N>` instead of preserving them. See
[`recovery.md`](./recovery.md).
