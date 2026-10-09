# Replay healing: auto-heal, manual correction, and the repair loop

Two layers compose: replay heals **locator drift** automatically in-run, and
what it can't fix gets a bounded, evidence-driven repair loop between runs.

## 1. In-run auto-heal (built in, on by default)

A do-step whose role+name locator misses usually means accessible-name
drift ("Save" → "Save changes", a volatile count, a generated suffix). Instead
of failing, the runner collects the page's live role candidates and walks an
ordered strategy ladder — strict to permissive — that fires only when
**exactly one** candidate matches; it refuses to guess when ambiguous. On a
match the step retries once; a matched-but-still-failing retry is a hard
failure.

Each successful heal is persisted for review:

- `replays/<runId>/heal.jsonl` — `heal-row/v1` rows with mode
  `locator-correction`
- `replays/<runId>/diffs/<stepId>.patch.json` — a `heal-patch/v1` file that
  `heal-promote --apply` writes back into `scenario.json` when you accept it

Failures no strategy can heal are probed for a value rejection (visible
alert/toast/banner) and classified in the audit — never retried.

Env gates:

- `AGENT_QA_NO_HEAL` — disable auto-heal entirely (CI runs that must fail hard
  on any drift)
- `AGENT_QA_HEAL_STRICT` — a run that needed any heal exits non-zero even when
  every step passed, so drift surfaces for review instead of silently
  self-correcting

## 2. Correct a replay value manually

Use this when you have inspected a failed run and decided that a do-step needs
a different string value.

```bash
# Record the decision against a failed run.
agent-qa heal-respond <sid> --run <failedRunId> --step <stepId> \
  --value '<corrected-string>' --rationale '<why>'

# Re-run with that response loaded as a transient value override.
agent-qa replay <sid> --heal-from-run <failedRunId> [the original replay flags]
```

`heal-respond` writes:

```text
<sid>/replays/<failedRunId>/heal-responses/<stepId>.json
```

It also appends a decision row to `<sid>/recording/heal.jsonl`.
`--reject` records that no correction should be applied.

`replay --heal-from-run` loads only response files whose mode is
`value-correction`. For the matching do-step it replaces `step.value` with a
string literal before dispatch. Rejected responses, `.applied.json` files,
check steps, and unknown step ids do not produce an override.

Important limits:

- corrections are strings, not arbitrary JSON objects;
- the override is useful only for verbs that consume `step.value`;
- the scenario contract is not changed by replay.

Use `agent-qa heal-list <sid> [--run <runId>]` to inspect recorded responses.

## 3. Patch an in-flight recording buffer

`heal-apply` can consume the same value-correction response and patch one row in
the active recording buffer:

```bash
agent-qa heal-apply <sid> --run <runId> --step <stepId> [--target-step <index-or-id>]
```

It updates the value argument consumed by the recorded action in
`<record_root>/recorder-state.json`, renames the response to
`<stepId>.applied.json`, and appends an audit row. It never drives the live
browser; re-position the tab and re-issue the corrected gesture yourself. See
[`heal-apply.md`](./heal-apply.md) and [`recovery.md`](./recovery.md).

## 4. Promote a locator patch into the scenario

`heal-promote` consumes `heal-patch/v1` files — whether written by in-run
auto-heal or supplied externally:

```bash
agent-qa heal-promote <sid> [--run <runId>] [--steps <id,...>] [--apply]
```

It reads `<sid>/replays/<runId>/diffs/<stepId>.patch.json`. Without `--apply`
it is a dry run. With `--apply` it atomically updates the matching step
locator in `scenario.json`. A `scenarioContentHash` mismatch returns exit 3
rather than overwriting a changed contract.

## 5. The repair loop — what auto-heal cannot fix

When a run fails, the verdict comes first — then the classification decides
the action. Bounded: a few cycles, never an unbounded retry storm.

```text
replay → if FAIL: audit explain → classify →
  locator/value drift   → auto-heal already handled it, or
                          heal-respond + replay --heal-from-run →
                          heal-promote --apply to keep it
  wrong flow or route   → buffer load <sid> → buffer insert/delete/edit →
                          flush → replay
  known ambient noise   → demote the check: context.onFailure "ignore"
                          (reports, never gates) or flush --no-auto-errors
  auth/environment      → fix the connection/persona, retry
  product regression    → STOP — keep the run red and report the evidence
```

`agent-qa audit explain <sid> [runId]` is the single read that drives this —
it prints the failed step, screenshot/snapshot paths, console and network
signals, and the `next:` commands. `agent-qa triage` gives the same
classification for a batch.

Rules that keep this honest:

- never make a real regression green by weakening assertions — delete or
  demote only checks whose failure is proven ambient noise;
- `buffer` edits write through `flush` so the scenario stays schema-valid;
- stop after a small bounded number of cycles and mark the case blocked with
  the audit digest attached.

## Locator tolerance metadata

The scenario schema accepts `Locator.tolerate` metadata, but the current runner
does not enforce it. Do not rely on those fields to enable or disable matching.
See [`heal-opt-out.md`](./heal-opt-out.md).
