# Recovery — when a recording step goes wrong

Two distinct recovery shapes, picked by what kind of failure you're recovering from:

| Failure shape                                                                                                            | Use                                                                                                                                         |
| ------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| **Value rejection** — the recorded value was wrong (bad email domain, denied invoice code, etc.). Later rows are fine.   | `agent-qa heal-respond` + `agent-qa heal-apply` — patches ONE row's value template in place, preserves later rows + sidecars byte-for-byte. |
| **Locator drift / structural step changes** — later rows are also wrong, or the gesture itself went somewhere unintended | `agent-qa truncate <N>` — drops rows ≥ N + stores sidecars. Agent re-drives the live tab via `agent-browser` first.                       |

Think of `heal-apply` as **the `--update-snapshot` of a recording**: a
narrow in-place patch driven by a peer review (the `heal-respond`
file). `truncate` is the broader reset for cases where the scenario's
shape needs to change.

## Value-rejection flow (the `heal-apply` path)

When `smart-click` exits 7 ("submit rejected"), it has already written
a request file at `recording/heal-requests/<stepId>.json` carrying the
GraphQL evidence (variables, errors, alert text). Two-step recovery:

```bash
# 1. Read the request, propose a corrected value, write the response file.
agent-qa heal-respond <sid> --step <stepId> \
    --value '<correctedValue>' \
    --rationale 'why this fixes the rejection'

# 2. Apply the correction to the recording in place.
agent-qa heal-apply <sid> --step <stepId>
```

`heal-apply` then:

- **Auto-resolves the target row** by matching GraphQL variable values
  in the request against earlier rows' `binding.seed`/`pattern` /
  `args[]` literal. Byte-equality on protocol bytes.
- **Patches that row's value template** in `tmp/scenario.steps.jsonl`
  in place. Atomic write under the scenario lock; `stepId` and every
  other field unchanged.
- **Preserves every later row + every sidecar byte-for-byte.**
  Captures, probes, snapshots, screenshots, network, perf — all
  untouched.
- **Appends a `caller-driven-resolved` row** to
  `recording/heal.jsonl`. Append-only audit; the earlier
  `caller-driven-pending` row from `cli/src/heal_respond.rs` stays in place.
- **Renames the request file** `<stepId>.json` →
  `<stepId>.applied.json` as a visible idempotency marker. A second
  call against the same stepId exits 7 (`already applied`).
- **Prints a re-execution checklist** the agent runs in the live tab
  via `agent-browser` directly (agent-qa never drives the
  tab).

After running `heal-apply`, the agent re-fires the corrected fill +
re-issues the rejected submit (`smart-click '<submit-name>'`) — that
appends the now-good submit row. `flush + verify + replay` should be
clean.

See [`heal-apply.md`](./heal-apply.md) for full flag reference, exit
codes, and worked example.

### Rebase guard

`heal-respond` (record mode) records a `stepsHashBefore` SHA-256 of
`tmp/scenario.steps.jsonl`. If the file changes between respond and
apply (e.g. you ran `truncate <M>` in between), `heal-apply` exits 3
with a "rebase required" message — re-run `heal-respond` first.

## Truncate flow (locator drift / structural changes)

When the failure isn't a value rejection — the gesture went to the
wrong target, the page navigated unexpectedly, the recording's later
rows are also stale — fall back to `truncate`. This is the **two-job
split**:

1. **The agent drives the live tab** back to step `N`'s pre-state
   using `agent-browser` primitives.
2. **`agent-qa truncate <N> [--archive-tag <slug>]`** does the disk
   bookkeeping: drops rows ≥ N from `tmp/scenario.steps.jsonl`,
   stores sidecars under
   `<sid>/failed/truncate-<isoTs>[-<tag>]/`, writes a manifest. The
   verb **never** touches the live tab.

  ones that "look already done".

## What `heal-apply` and `truncate` do NOT do

| Operation                                                            | Why not                                                                                                                                                    |
| -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Re-fire `agent-browser open / click / fill / wait` for any kept step | The agent already calls these primitives every other step. Re-firing them from agent-qa duplicates `agent-browser`'s job and gets it wrong on modal flows. |
| Re-mint `{{vars._unique}}` tokens                                    | The agent re-types via `fill-unique` (which mints honestly) or copies the value visible in the tab.                                                        |
| Settle-wait for SPA renders after a tab gesture                      | Settle gates are `record-step`'s job. If the agent ran a tab gesture, the next `record-step` (or `smart-click`) will settle correctly.                     |
| Probe the page for app-specific copy                                 | No per-app vocabulary in the generic recorder. Use `agent-browser snapshot` or `agent-browser screenshot` to inspect.                              |

## Common pitfalls

- **`heal-apply` is for value rejections only.** If the failure is a
  locator drift or structural step change, the auto-resolver will
  exit 3 (no GQL variable matched any row) — that's the signal to
  fall back to `truncate` + re-record.
- **`heal-respond --reject`** writes a `rejected: true` response.
  `heal-apply` then exits 2 (nothing to apply) — that's the correct
  behaviour. Use `--reject` to record "I refuse to propose a
  correction" without dirtying the recording.
- **Ambiguous auto-resolve.** Two prior fills with the same value
  template both match the GraphQL variable byte-equally. `heal-apply`
  exits 3 with the candidate list; pass `--target-step <stepIdOrIndex>`
  to disambiguate.
- **The live tab's visual state is NOT evidence the steps were
  recorded.** This is the #1 trap. `truncate` does
  not touch the live tab, so anything you typed / selected / opened
  during the dropped steps is STILL VISIBLE in the tab. Grep stderr
  for `[truncate] ⚠` after every truncate and re-record EVERY
  listed row in order.
- **Forgetting the tab gesture.** `truncate` does NOT move the tab.
  Always do the tab gesture first (or verify via `agent-browser
screenshot`).
- **Idempotency markers.** After `heal-apply` succeeds, the request
  file is renamed to `<stepId>.applied.json`. Don't grep for the
  bare `.json` suffix and assume "no heal exists" — also check
  `.applied.json`. Future tooling that reads `heal-requests/` MUST
  account for both suffixes.

## See also

- [`heal-apply.md`](./heal-apply.md) — full `heal-apply` reference
- [`verbs.md`](./verbs.md) — verb catalog
- [`heal.md`](./heal.md) — scenario/2 heal pipeline (replay-side)
- [`gotchas.md`](./gotchas.md) — record-step persistence beacon + SID resolution rules
