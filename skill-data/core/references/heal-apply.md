# `heal-apply` — record-time `--update-snapshot` of a recording

## What it is

`heal-apply` patches the offending row's value template in
`tmp/scenario.steps.jsonl` **in place** after `heal-respond` has
written a corrected value. Think of it as the `--update-snapshot` of a
recording: a narrow, peer-reviewed in-place edit. It preserves every
later row + every sidecar (probes, snapshots, screenshots,
network, perf) byte-for-byte.

`heal-apply` NEVER touches the live tab — agent-qa is bookkeeping;
the agent drives the tab via `agent-browser` directly using the
printed checklist.

## When to use it

Use `heal-apply` when:

- A submit was rejected by the backend (`smart-click` exited 7).
- The rejection's GraphQL evidence (variables / `errors[].pointer`)
  points at exactly one of your earlier `fill` rows.
- The recorded value template can be corrected without changing the
  shape of any later step (invoice click, option pick, navigation,
  etc.).

Do NOT use `heal-apply` when:

- The locator drifted between attempts. The auto-resolver matches on
  GraphQL variable byte-equality; it won't help you.
- Later rows are also wrong (e.g. the recording navigated to a
  different page after the bad submit). Use `truncate <N>` instead.
- The corrected value's shape doesn't match the recorded row's
  field shape (template-vs-literal mismatch). The verb refuses
  with exit 4; either correct via `heal-respond --value '<template>'`
  with `{{vars._unique}}`, or pass `--field value` explicitly.

## Usage

```text
agent-qa heal-apply <sid> --step <stepId>
                          [--target-step <stepIndex|stepId>]
                          [--field auto|seed|pattern|value]
                          [--dry-run]
```

| Flag                | Meaning                                                                                                                                                                                                                       |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `<sid>`             | Positional. Same shape as `heal-respond` — accepts a SID, an absolute path to a scenario dir, or a path to `scenario.json`.                                                                                                     |
| `--step <stepId>`   | Required. The stepId of the **rejected submit** (smart-click exit 7). Pairs the apply with the matching `recording/heal-requests/<stepId>.json` + `recording/heal-responses/<stepId>.json` round-trip.                        |
| `--target-step <r>` | Optional. Names the row in `tmp/scenario.steps.jsonl` whose value gets patched. Accepts an integer index (`4`) or a stepId (`fill-mxxxxx`). Default: auto-resolve from the request file's GraphQL evidence.                    |
| `--field <kind>`    | Which field to overwrite. `auto` (default) picks `binding.seed` for fill-unique-minted rows, else `binding.pattern`, else the last string element of `args[]`. Override with `seed` / `pattern` / `value` for unusual shapes. |
| `--dry-run`         | Print the planned patch (target row, before/after, checklist) and exit 0 without writing anything.                                                                                                                            |

## Auto-resolution (generic protocol bytes only)

The auto-resolver:

1. Reads `recording/heal-requests/<stepId>.json` written by
   `tryRecordSideValueHeal`.
2. Walks `signal.networkErrors[]`. For each entry, parses
   `postData` as JSON and extracts every leaf string from
   `variables`. (If `originalValue` is a string, adds that too.)
3. For each prior **action** row, tests whether any string leaf
   matches:
   - `binding.seed` interpreted as a `{{vars._unique}}` template
     (regex match against the leaf), OR
   - `binding.pattern` (same), OR
   - any literal element of `args[]` (template match if it has
     `{{vars._unique}}`, byte-equal otherwise).
4. Exactly one row matches → resolved. Zero or multiple → exit 3
   with the candidate list; pass `--target-step` to disambiguate.

No vocabulary. No copy-text matches. Renaming any GraphQL field name
to `xyz123` everywhere produces the same auto-resolve outcome.

## Side effects on disk

- **`tmp/scenario.steps.jsonl`** — exactly one field of one row
  changes. The atomic write rewrites the whole file under the
  scenario lock (same lock `record-step` uses); the row's `stepId`
  and every other field are byte-identical. When patching a `seed`
  or `pattern` field, the matching `args[<last-string>]` literal is
  updated in lockstep so replay's substitution reads the corrected
  template.
- **`<sid>/recording/heal.jsonl`** — one new row appended:
  `disposition: 'caller-driven-resolved'`, `attempt.value:
<correctedValue>`, `signal` copied from the request, plus
  `targetStepIndex`, `targetStepId`, `patchedField` for forensics.
  Append-only — never edits prior rows.
- **`<sid>/recording/heal-requests/<stepId>.json`** → renamed to
  `<stepId>.applied.json`. This is the visible idempotency marker.

## Idempotency markers

After a successful `heal-apply`, the request file lives at
`recording/heal-requests/<stepId>.applied.json`. A second
invocation against the same stepId:

- Detects the `.applied.json` suffix.
- Exits 7 (`already applied`) with stderr naming the prior patch.
- Does NOT re-patch the steps file or re-append heal.jsonl.

To deliberately re-apply (e.g. with a different `--field`), rename
the file back: `mv recording/heal-requests/<stepId>.applied.json
recording/heal-requests/<stepId>.json`. This is intentionally a
two-step gesture — `heal-apply` should never silently apply a
different value to the same row.

> Future tooling that reads `heal-requests/` MUST account for both
> `.json` and `.applied.json` suffixes. The lifecycle is documented
> here as a single canonical reference.

## Rebase guard

`heal-respond` (record mode) records a `stepsHashBefore` field on
the response payload — SHA-256 of `tmp/scenario.steps.jsonl` at
respond-time. `heal-apply` re-hashes the file before patching:

- Hashes match → patch proceeds.
- Hashes differ → exit 3 with `live tab unchanged; rebase required`.
  Re-run `heal-respond` against the current state, then `heal-apply`.

The field is optional on the response payload — older recordings
and replay-mode responses (which don't have a `tmp/` to hash) skip
the guard.

## Re-execution checklist

After a successful patch, `heal-apply` prints a numbered checklist
of the gestures the agent must re-issue in the live tab:

```text
[heal-apply]  1. Re-fire the corrected fill in the live tab. Template: fillByLabel("Email", "qa-{{vars._unique}}@example.com")
[heal-apply]     - If the row carries a {{vars._unique}} template, expand it with the recording's seed before typing,
[heal-apply]       or use `agent-qa fill-unique` against the same label to mint a fresh value with the corrected template.
[heal-apply]  2. Re-issue the rejected submit (`agent-qa smart-click '<submit-name>'`).
```

The checklist is structural — derived only from the patched row's
`type` / `method` / `args` / `target` / `binding`. It does
not name MUI selectors, framework component classes, or app-specific copy.

## Exit codes

| Code | Meaning                                                                                                                                                                |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0    | Patch applied (or dry-run produced); checklist printed.                                                                                                                |
| 1    | Filesystem error during atomic write or rename.                                                                                                                        |
| 2    | Bad args / missing flags. Also: response is `rejected: true` (nothing to apply) or `correctedValue` is not a string.                                                   |
| 3    | Auto-resolve could not pick exactly one target row, OR rebase guard fired (stepsHashBefore mismatch). Stderr enumerates candidates / names the hash drift.             |
| 4    | Target row exists but `--field auto` could not pick a field shape, OR template-vs-literal shape mismatch between the recorded field and `correctedValue`.              |
| 5    | Pre-conditions missing: `recording/heal-responses/<stepId>.json` not on disk (run `heal-respond` first), or request file missing, or `tmp/scenario.steps.jsonl` absent. |
| 6    | Request or response file present but JSON-corrupt.                                                                                                                     |
| 7    | Idempotent no-op: already applied (request file has `.applied.json` suffix).                                                                                           |

## Worked example

```bash
# A 9-step "create a user" flow. Step 4 = fill-unique 'Email' with
# 'qa-{{vars._unique}}@example.com'. Step 7 = submit. Submit rejected
# because the org excludes @example.com.

# smart-click 'Add user' wrote recording/heal-requests/submit-mpe6d9d8.json.

agent-qa heal-respond <sid> --step submit-mpe6d9d8 \
    --value 'qa-{{vars._unique}}@example.com' \
    --rationale 'org excludes @example.com per signal.networkErrors[0].errors[0].code'
# [heal-respond] (record mode) wrote .../recording/heal-responses/submit-mpe6d9d8.json
# [heal-respond] appended audit row to .../recording/heal.jsonl

agent-qa heal-apply <sid> --step submit-mpe6d9d8
# [heal-apply] auto-resolved target row 4 (action fillByLabel("Email", "qa-{{vars._unique}}@example.com") binding=userEmail   [fill-mq7p2k1])
# [heal-apply] Patched row 4 (seed: "qa-{{vars._unique}}@example.com" → "qa-{{vars._unique}}@example.com").
# [heal-apply] live tab unchanged; you must now re-execute the gestures below in order.
# [heal-apply] The recording does NOT change beyond row 4; the live tab does.
# [heal-apply]
# [heal-apply]  1. Re-fire the corrected fill in the live tab. Template: fillByLabel("Email" "qa-{{vars._unique}}@example.com")
# [heal-apply]     - If the row carries a {{vars._unique}} template, expand it with the recording's seed before typing, ...
# [heal-apply]  2. Re-issue the rejected submit (`agent-qa smart-click '<submit-name>'`).

# Agent re-fires the corrected fill + re-issues submit:
agent-browser --session default-user-session fill 'input[aria-label="Email"]' '<corrected-substituted>'
agent-qa smart-click 'Add user'

agent-qa flush && agent-qa verify
agent-qa replay <sid> --profile default-user
# SUMMARY: 9/9 (PASS)
```

Steps 5, 6 (the invoice click and option pick) were NOT re-recorded.
Their probes, snapshots, and network sidecars are byte-equal
to what they were before the heal.

## Composes with `truncate`

`heal-apply` and `truncate` are deliberately separate verbs:

- `heal-apply` for **value rejections** (single-row patch, later
  rows fine).
- `truncate` for **locator drift / structural step changes** (later
  rows are also bad, full re-record from `<N>`).

If `heal-apply` exits 3 with "no recorded action row matched any
GraphQL variable value from the rejection," that's the signal to
fall back to `truncate`.

## See also

- [`recovery.md`](./recovery.md) — top-level decision matrix
- [`heal.md`](./heal.md) — scenario/2 heal pipeline (replay + record-time)
- [`verbs.md`](./verbs.md) — verb catalog
