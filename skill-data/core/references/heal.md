# Manual correction and recovery

agent-qa currently provides **manual correction tools**. Replay does not yet
classify failures, generate heal requests, create locator patch suggestions, or
run an autonomous locator-heal strategy ladder.

## What replay does today

For each step, replay dispatches the recorded action or check. There are two
built-in robustness paths around clicks:

- role/name DOM activation tries exact, substring, and digit-normalized name
  matching for a broad set of interactive roles;
- when an option or menu-item click fails, replay can re-fire the previous
  opener and retry that popup-content click once.

Other failures stop the run. Replay does not write `heal.jsonl`, does not emit a
caller-driven exit code, and does not create files under `diffs/` or
`heal-requests/`.

## Correct a replay value manually

Use this when you have inspected a failed run and decided that a do-step needs a
different string value.

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

- core replay does not create the response or ask for one automatically;
- corrections are strings, not arbitrary JSON objects;
- the override is useful only for verbs that consume `step.value`;
- the scenario contract is not changed by replay.

Use `agent-qa heal-list <sid> [--run <runId>]` to inspect recorded responses.

## Patch an in-flight recording buffer

`heal-apply` can consume the same value-correction response and patch one row in
the active recording buffer:

```bash
agent-qa heal-apply <sid> --run <runId> --step <stepId> [--target-step <index-or-id>]
```

It updates the value argument consumed by the recorded action in
`<record_root>/scenario.steps.jsonl`, renames the response to
`<stepId>.applied.json`, and appends an audit row. It never drives the live
browser; re-position the tab and re-issue the corrected gesture yourself. See
[`heal-apply.md`](./heal-apply.md) and [`recovery.md`](./recovery.md).

## Promote an externally supplied locator patch

`heal-promote` is a consumer for locator patch files:

```bash
agent-qa heal-promote <sid> [--run <runId>] [--steps <id,...>] [--apply]
```

It reads files under:

```text
<sid>/replays/<runId>/diffs/<stepId>.patch.json
```

The current core replay does **not** generate those files. An external tool
may supply them using the `heal-patch/v1` shape expected by
`cli/src/heal_promote.rs`. Without `--apply`, the command is a dry run. With
`--apply`, it atomically updates the matching step locator in `scenario.json`.
A `scenarioContentHash` mismatch returns exit 3 rather than overwriting a
changed contract.

## Locator tolerance metadata

The scenario schema accepts `Locator.tolerate` metadata, but the current runner
does not enforce it. Do not rely on those fields to enable or disable matching.
See [`heal-opt-out.md`](./heal-opt-out.md).

## Planned work

The backlog for autonomous locator classification, one-shot retry, audit rows,
strict mode, and suggested patches is tracked in
[`docs/specs/replay-robustness-followups.md`](../../../docs/specs/replay-robustness-followups.md).
Do not document that design as shipped behavior until the runner and tests land.
