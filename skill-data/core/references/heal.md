# Heal (scenario/2 — replay-time auto-correction with audit)

The scenario/2 heal pipeline auto-corrects two manifestations of step failure (locator-drift, value-rejection) at two lifecycle moments (record, replay) — one mechanism, one strategy registry, one row schema (`HealRowV1`), one disposition table. **The contract (`scenario.json`) is NEVER mutated by replay.** Replay-side corrections land as RFC 6902 patch files for operator review via `agent-qa heal-promote`.

**What it covers.** Replay-side locator-drift heal, `agent-qa heal-promote` with rebase guard, strict-mode CI default, caller-driven value-rejection heal at REPLAY (exit-7 + `heal-respond` + `replay --heal-from-run`), record-side locator heal in `smart-click`, and record-side value-rejection heal in `smart-click`'s submit-rejected branch (same caller-driven contract; `heal-respond` auto-detects record vs replay mode).

## What fires when a step fails at replay time

The dispatch order, recorded literally in `replays/<runId>/heal.jsonl`:

1. **scenario/2 heal pipeline — locator path** (`cli/src/runner.rs:handleLocatorMiss`) — fires when the resolver throws AND no recent GQL errors are present. Walks `DEFAULT_HEAL_REGISTRY` for the `(replay, locator)` cell:
   - `whitespace-collapsed` — folds Unicode whitespace (incl. NBSP, thin space, zero-width space), ellipsis (`\u2026 → ...`), smart quotes, en/em dashes, AND strips standalone required-field asterisks (`'Email *'` ↔ `'Email'`). Matches → emits `matched-drift-suggest-diff` row + writes `replays/<runId>/diffs/<stepId>.patch.json`.
   - `digit-tolerant` — strips trailing `\d+` suffix, matches against live name. Same disposition shape.
   - `generated-suffix` — strips 6-12 char trailing alphanumeric tokens (a randomly-generated id suffix). Same disposition shape.
   - `name-prefix` — fires when the recorded name is a whole-word prefix of a live candidate (`'Select a invoice'` → `'Select a invoice Submit 19848 items pending'`). Handles selection-augmented popup-opener labels — the canonical reproducer is a combobox / MUI Select whose accessible name gets the picked option appended after interaction. Requires ≥ 4 chars and word-boundary after the prefix. Runs LAST in the registry; the stricter strategies get first refusal.
2. **scenario/2 heal pipeline — value path** (`handleValueRejection`) — fires when the resolver throws AND recent GQL errors[] are present (either inline in the error message from `callGql`, or collected from network sidecars). The default registry has no rule-based value strategies, so:
   - When `AGENT_QA_HEAL_LLM=1` → writes a request file to `replays/<runId>/heal-requests/<stepId>.json` + emits a `caller-driven-pending` row + exits 7 (`HEAL_REQUEST_CALLER`). Caller resumes via `heal-respond` + `replay --heal-from-run <runId>`.
   - When `AGENT_QA_HEAL_LLM` is unset / `0` → emits `no-match-abort` row + run aborts.
3. **No strategy matched + no caller-driven path** → `no-match-abort` row + run aborts.

> **Note**: The engine's in-place tiers (`cli/src/scenario.rs`'s `whitespaceCollapsedEqual` / `digitTolerantEqual` / `alphanumericSuffixEqual` name-match helpers) were retired along with the `wouldHaveMatch` diagnostic. The scenario/2 heal pipeline is now the single source of truth — every drift produces an auditable row.

## Heal at record time (Phase 4)

When `smart-click "<name>" [--role <role>]` can't resolve the requested target against the live ARIA snapshot, the not-found branch invokes the same heal registry filtered to `(mode:'record', manifestation:'locator')` BEFORE returning the not-found error. If a strategy matches (e.g. `whitespace-collapsed` folds `\u2026 → ...`), smart-click swaps the requested name to the corrected one, re-resolves, and proceeds with the click. The correction is surfaced to stderr:

```
[smart-click] heal: whitespace-collapsed matched — 'More information...' → 'More information…' (row: …/recording/heal.jsonl)
[smart-click] "More information..." → @e2 clicked (URL change)
```

A heal row lands at `<scenarioDir>/recording/heal.jsonl` with `mode:'record'`, `manifestation:'locator'`, `disposition:'matched-drift-feed-forward'`. The CALLING AGENT (the LLM driving the recording) reads the stderr message and uses the CORRECTED name when it later calls `record-step` — that's how `steps.jsonl` ends up reflecting the live page rather than the original misguess. Because the scenario contract isn't sealed until `flush`, this feed-forward is non-destructive.

`AGENT_QA_NO_HEAL=1` disables both replay-side and record-side heal pipelines (the engine's in-place tiers stay on independently).

### Record-side value-rejection heal (Track 4)

When `smart-click "<submitButton>"` succeeds at clicking but the backend pushes back (an `[aria-live]` region appears with an error, OR — at staging — a GraphQL response carries `errors[]`), the submit-rejected branch invokes `tryRecordSideValueHeal`. When `AGENT_QA_HEAL_LLM=1`:

1. smart-click writes a request file to `<scenarioDir>/recording/heal-requests/<stepId>.json` carrying the alert text or networkErrors[], the same `responseSchema` Phase 5 emits, and record-mode-specific `instructionsForCaller`.
2. smart-click exits **7** (HEAL_REQUEST_CALLER) — same exit code as the replay-side caller-driven path.
3. The CALLING AGENT reads the request file, runs `agent-qa heal-respond <sid> --step <stepId> --value <X> --rationale "<text>"`. The verb auto-detects record vs replay mode (presence of `recording/heal-requests/<stepId>.json` without a `replays/latest.txt`) and writes the response under `recording/heal-responses/`.
4. The agent then re-narrates the fill + submit with the corrected value — **there is no auto-retry at record time**, unlike `replay --heal-from-run` which the runner handles automatically. The response file is the audit trail; the next `record-step` invocation persists whatever the live page actually accepts.

When `AGENT_QA_HEAL_LLM` is unset, smart-click still emits a `mode:'record', manifestation:'value', disposition:'no-match-abort'` row + falls through to the legacy exit-3 hint.

## Strict mode

```
AGENT_QA_HEAL_STRICT=1   # force on  — any matched-drift-* fails the run with exit 1
AGENT_QA_HEAL_STRICT=0   # force off — drift continues silently with suggested-diff
(unset)                  # CI default — on iff process.env.CI is truthy
```

Strict mode fires AFTER the run completes; the suggested-diff still lands. Only the exit code changes. CI is the dominant consumer — drift becomes a hard signal; the suggested-diff is the operator's TODO.

Resolved value is printed at runner start: `[v2-replay] heal: strict=on|off`.

## Caller-driven heal (value rejection)

```
AGENT_QA_HEAL_LLM=1      # enable caller-driven heal for value-rejection misses
(unset)                  # default — value-rejection misses abort as before
```

The full round-trip when a `callGql` (or other backend-touching) step fails because the value was rejected:

1. `agent-qa replay <sid>` exits **7** with a request file at `replays/<runId>/heal-requests/<stepId>.json`. The runner prints the exact `heal-respond` invocation to stderr.
2. The CALLING AGENT (the LLM session that invoked `agent-qa`) reads the request file. Each request carries `signal.networkErrors[]` with the rejection details, the `originalValue` (redacted if long), and a `responseSchema` JSON Schema 2020-12 the response MUST validate against.
3. Caller runs `agent-qa heal-respond <sid> --step <stepId> --value <X> --rationale "<text>"`. The verb writes `replays/<runId>/heal-responses/<stepId>.json` and appends an audit row to `heal.jsonl`. `--reject` records a refusal instead.
4. Caller re-invokes `agent-qa replay <sid> --heal-from-run <runId>`. The runner loads all heal-responses for that run as a `stepId → correctedValue` map. At step dispatch time, any step whose id is in the map gets its value overridden:
   - For `do` verbs with `step.value`, the override REPLACES `step.value` with `{from:'literal', literal: <override>}`.
   - For `callGql`, the override MERGES into `step.params.variables` when the override is an object.
5. The corrected step runs; if it succeeds, the run continues normally. Each `[heal-override applied]` step is annotated in stdout.

## Silencing benign drift — `Locator.tolerate`

Two distinct mechanisms operate on contract-affecting drift:

- **`heal-promote --apply`** — promote a specific replay-side suggested-diff INTO the contract. The locator's literal recorded `name` becomes the live name. Operator-gated, one drift at a time. Use when the drift IS the new truth (e.g. a product rename you want pinned).
- **`Locator.tolerate`** — declare that a _dimension_ of drift on this locator is known-volatile and should be silenced FOREVER. The literal recorded `name` stays in `scenario.json` for forensics; the heal pipeline absorbs matches against the tolerated dimension as `matched-drift-absorb-by-policy` — no patch file, no `[healed]` log, no `driftSeen()`. Use when the drift is noise (counters, ids, selection augmentation) and you don't want to keep promoting fresh values.

Shape on the role-arm `Locator`:

```jsonc
"on": {
  "role": "option",
  "name": "Submit 19832 items pending",
  "scope": [{ "role": "listbox" }],
  "tolerate": {
    "digits": true,                 // one of digits | generatedSuffix | selectionAugmentedLabel
    "reason": "invoice counter is noise"  // required, free-form, minLength: 1
  }
}
```

Schema constraints (enforced at preflight; exit 2 on violation):

- At least one of `digits`, `generatedSuffix`, `selectionAugmentedLabel` MUST be set to `true`. Empty `tolerate: {}` is rejected.
- `reason` is required.
- `whitespace-collapsed` is absorbed unconditionally at the pipeline level — there is no `tolerate.whitespace` flag. Whitespace drift is always benign in practice.

Strategy → dimension mapping enforced by `cli/src/runner.rs`:

| Strategy               | Dimension flag            |
| ---------------------- | ------------------------- |
| `whitespace-collapsed` | (always absorbed)         |
| `digit-anywhere`       | `digits`                  |
| `digit-tolerant`       | `digits`                  |
| `generated-suffix`        | `generatedSuffix`                  |
| `name-prefix`          | `selectionAugmentedLabel` |

Tolerance is per-dimension, not blanket: `tolerate.digits: true` does NOT silence a `name-prefix` match on the same locator. Authors must opt into each dimension separately. Genuine structural drift (stem rename — `Submit` → `Approval`, role change, dimension not tolerated) still fails the run loudly with `no-match-abort`. Schema authors who want true regex semantics still have `NameMatch.pattern + "regex"` as the heavier escape hatch.

## The `heal-promote` verb

```
agent-qa heal-promote <sid> [--run <runId>] [--steps <stepId,…>] [--apply]
```

- Default = dry-run; prints a unified diff per step to stdout.
- `--apply` atomically rewrites `scenario.json` (`.tmp` → rename) and appends one `mode:'promoted'` audit row per change to `recording/heal.jsonl`.
- `--run <runId>` defaults to `replays/latest.txt`.
- `--steps <stepId,…>` filters to a subset of stepIds.
- Rebase guard: compares `audit.json.scenarioContentHash` (recorded at run start) against the current sha256 of `scenario.json`. Mismatch → exit 3 with hint; no mutation. Re-run replay to refresh the diff against current contract, or hand-merge.

Exit codes:

| Code | Meaning                                                                           |
| ---- | --------------------------------------------------------------------------------- |
| 0    | Dry-run printed a diff (or `--apply` succeeded), or there was nothing to promote. |
| 2    | Bad CLI args / missing files.                                                     |
| 3    | `scenarioContentHash` mismatch — `--apply` only; rebase needed.                    |
| 1    | Other failures (read / parse / write / unexpected).                               |

## Row schema (`HealRowV1`)

One JSONL row per miss, written to either `replays/<runId>/heal.jsonl` (replay) or `recording/heal.jsonl` (record + `mode:'promoted'` audit). Schema-tagged `"scenario-heal/v1"` for additive evolution.

Key fields:

- `mode: 'record' | 'replay' | 'promoted'` — `'promoted'` rows are mutually exclusive with `tried[]` / `disposition` / `manifestation`; they're pure audit-trail entries emitted by `heal-promote --apply`.
- `manifestation: 'locator' | 'value'` (attempt rows only).
- `attempt: { locator?, value? }` — the hypothesis being heal-checked.
- `signal` — discriminated union: `{kind:'resolver-miss', matcherTrace, candidateNames}` or `{kind:'value-rejection', httpStatus, networkErrors[]}`.
- `tried: [{name, outcome: 'matched'|'rejected'|'skipped', reason?, proposed?, drift?}]` — every strategy invocation, in order.
- `disposition: 'no-match-abort' | 'matched-identical-continue' | 'matched-drift-suggest-diff' | 'matched-drift-feed-forward' | 'budget-exhausted' | 'caller-driven-pending'`.
- `suggestedDiff?: {path, targetStepId, targetField}` — present when disposition emitted a patch file.
- `promotedFrom?: {runId, eventRowId, diffPath, promotedAt, scenarioContentHashBefore, scenarioContentHashAfter}` — present iff `mode === 'promoted'`.

## Suggested-diff schema (`scenario-heal-diff/v1`)

Single JSON document at `replays/<runId>/diffs/<stepId>.patch.json`:

```jsonc
{
  "schema": "scenario-heal-diff/v1",
  "runId": "<runId>",
  "patches": [
    {
      "op": "replace",
      "targetStepId": "<stepId>",
      "targetField": "on", // 'on' | 'value' | 'value+inputs'
      "from": {
        /* recorded shape */
      },
      "value": {
        /* corrected shape */
      },
    },
  ],
}
```

`heal-promote` resolves `targetStepId` to the actual `/steps/<index>/<targetField>` JSON Pointer at apply time, against the CURRENT scenario.json. This decouples diff bytes from step ordering — a scenario can reorder steps between record and apply without invalidating diffs.

## Strategy registry

The Phase-2 default registry (`cli/src/runner.rs:DEFAULT_HEAL_REGISTRY`):

| Name                   | Kind | Modes          | Cells served |
| ---------------------- | ---- | -------------- | ------------ |
| `whitespace-collapsed` | rule | record, replay | locator      |
| `digit-tolerant`       | rule | record, replay | locator      |
| `generated-suffix`        | rule | record, replay | locator      |

`(record, *)` strategies are registered but the runner-side record integration is not wired (Phase 4 deferred). `(*, value)` and caller-driven strategies are Phase 5 deferred — placeholders in the type union are intentional so the eventual integration is additive, not breaking.

Per-strategy `timeoutMs` (default 10s) enforced by `runStrategy()` via `Promise.race`; a timed-out strategy counts as `rejected` with reason `'strategy timed out after Nms'`. A thrown exception is converted to `rejected` with the error message — no leaked promise rejections.

## Prior-art lens

`heal-promote` is intentionally a **reviewable-AI-suggestion** UX (operator runs the verb, sees the diff, decides accept/reject), not automatic write-back. Same shape as another-tool Studio — heal proposes, human approves. The `--apply` step is a deliberate second action with its own audit, never a flag on `replay`.

## Recovery at record time (`truncate` + agent-browser)

When a recording step fails irrecoverably (the agent typed the wrong value, the modal rejected the submit with a non-healable error, etc.) and you need to drop the bad rows and re-narrate from an earlier step, the scenario/2 recovery flow is a clean two-job split:

1. **Drive the live tab back to step N's pre-state YOURSELF** using `agent-browser` primitives — `open <url>`, `click "<Back>"`, `keypress Escape`, `fill <sel> ""`, `eval 'history.back()'`, `eval 'location.reload()'`. The agent has eyes on the live tab; agent-qa does not.
2. **Truncate the on-disk state** to step N with `agent-qa truncate <N> [--archive-tag <slug>]`. This drops rows ≥ N from `tmp/scenario.steps.jsonl` and stores the corresponding sidecars to `<sid>/failed/truncate-<isoTs>[-<tag>]/` so forensics survive. The verb never touches the live tab.

The full decision matrix + cheat-sheet for which tab gesture to use when lives in [`recovery.md`](recovery.md).

## See also

- [`docs/specs/scenario-sidecar-tree.md`](../../../docs/specs/scenario-sidecar-tree.md) — sidecar path convention (heal.jsonl + diffs/ + recording/heal.jsonl)
- [`recovery.md`](./recovery.md) — the recovery flow (`truncate <N>` + agent-browser tab gestures)
- [`verbs.md`](./verbs.md) — the `heal-promote` row in the verb catalog
- [`replay.md`](./replay.md) — strict-mode env var in the env-var inventory
