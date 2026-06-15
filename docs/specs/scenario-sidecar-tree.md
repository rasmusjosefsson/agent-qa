# `scenario-sidecar-tree/v1` — sidecar path convention

**Spec version:** `scenario-sidecar-tree/v1`
**Status:** convention spec for the `scenario/2` artifact contract.
**Companion schema:** [`../../cli/src/scenario-schema.json`](../../cli/src/scenario-schema.json) — the schema's top-level `description` defers to this document.

## Purpose

`scenario/2` is a _contract_, not a log. The first principle of proposal-A (P1 — "the artifact is a contract, not a log") says it plainly: every field in `scenario.json` is author input, and the runner is forbidden from reading any field the author was not allowed to write. Audit data — what the page looked like at record time, what network fired, what heal did, what locators were tried — is observation _about_ the contract. The moment it lives _inside_ the contract, two things go wrong: PR review drowns in diff noise on every replay (the contract bytes change even when nothing semantic changed), and the runner is tempted to make replay decisions from recorded observations rather than from the contract the author wrote.

The fix is structural. `scenario.json` carries the contract; everything else lives in a sibling directory tree addressed by stable paths. Consumers find audit data by _path_, not by chasing a `evidence:` field on a step. The schema does not encode this layout (see [Relation to JSON Schema](#relation-to-json-schema) below) — the convention IS the contract for sidecar discovery, and convention does not need a JSON field.

This document is what a tool needs to read once to know where to write or look up sidecar files for a scenario. It is the discovery contract that pairs with the data contract in `scenario-schema.json`.

## Top-level layout

A scenario directory is identified by a scenario id (`SID`) of the form `<isoTimestamp>__<shortHash>`, e.g. `2026-05-19T09-22-33-832Z__d3ab716d`. Scenario directories live under the runtime-configured root (today `tmp/agent-qa-scenarios/` under the repo, overridable via `AGENT_QA_SCENARIOS_DIR`; the root itself is a runtime concern and not part of this spec).

The on-disk tree for one scenario:

```
<SID>/
├── scenario.json                              # the contract (validates against scenario-schema.json)
├── recording/                                # record-time observations (one tree; one recording session per SID)
│   ├── audit.json                            #   session metadata: started/finished, exit, summary
│   ├── heal.jsonl                            #   per-step heal events recorded during authoring
│   ├── heal-requests/<stepId>.json           #   pending record-side heal request (smart-click exit 7)
│   ├── heal-requests/<stepId>.applied.json   #   idempotency marker after `heal-apply`
│   ├── heal-responses/<stepId>.json          #   caller-driven correction + rationale + stepsHashBefore
│   ├── snapshots/<stepId>.txt                #   ARIA snapshot text (role + accessible-name tree)
│   ├── screenshots/<stepId>.png              #   full-page screenshot
│   ├── network/<stepId>.json                 #   network sidecar (GraphQL/REST calls; see § network/ for shape)
│   ├── probes/<stepId>.json                  #   DOM probe (outerHTML, computed style, ARIA attrs)
│   ├── perf/<stepId>.json                    #   perf sidecar (vitals, suspense, optional renders/cpu/trace pointers)
│   └── failed/                               #   assert-quality refusals + their evidence (see § failed/ below)
│       └── assert-refused-<stepId>-<isoTs>/
│           ├── payload.json                  #     structured diagnostic the gate emitted
│           ├── snapshot.txt                  #     ARIA snapshot at refusal time
│           ├── screenshot.png                #     full-page screenshot at refusal time
│           ├── probe.json                    #     DOM probe at refusal time
│           └── network.json                  #     recent network calls at refusal time
├── failed/                                   # store of truncate operations (see § failed/truncate-/ below)
│   └── truncate-<isoTs>[-<archive-tag>]/     #   one directory per `truncate` call
│       ├── snapshots/
│       ├── screenshots/
│       ├── network/
│       ├── probes/
│       ├── perf/
│       └── manifest.json                      #     truncate-store/v1 — correlates store ↔ source stepIds
└── replays/
    ├── latest.txt                             # pointer file: one line, the `<runId>` of the most recent replay
    └── <runId>/                               # one directory per replay run
        ├── audit.json                         #   run-level metadata: runId, startedAt, finishedAt, exitCode, summary line
        ├── heal.jsonl                         #   per-step heal events: locator candidates tried, accept/reject, diffs vs contract
        ├── heal-requests/<stepId>.json        #   caller-driven heal request (replay exit 7)
        ├── heal-responses/<stepId>.json       #   correction loaded via `replay --heal-from-run`
        ├── diffs/<stepId>.patch.json          #   suggested-diff promoted via `heal-promote`
        ├── snapshots/<stepId>.txt
        ├── screenshots/<stepId>.png
        ├── network/<stepId>.json
        ├── probes/<stepId>.json
        └── perf/<stepId>.json
```

The two trees (`recording/` and `replays/<runId>/`) deliberately share the same internal shape. A consumer that knows how to read a replay's `snapshots/<stepId>.txt` already knows how to read the recording's. Comparison tools diff `recording/<kind>/<stepId>.<ext>` against `replays/<runId>/<kind>/<stepId>.<ext>` with no special-casing.

### `scenario.json`

The single contract file. Validates against [`../../cli/src/scenario-schema.json`](../../cli/src/scenario-schema.json). MUST NOT contain `evidence`, `effect`, `healHistory`, `candidates`, or any other observation-shaped field. The contract is the _only_ thing PR review is asked to read line-by-line; sidecar trees are referenced as wholes ("this run passed N/N") or via tooling (diff viewers), not diffed by reviewers.

### `recording/`

One per SID. Holds observations gathered at _authoring_ time — the page state when the recorder captured each step. Used by:

- the assert-quality gate at record time (read snapshot/probe to refuse claims that would be false on replay);
- post-recording inspection tools (`agent-qa list`, `agent-qa verify`);
- the `compare` verb when diffing record-time vs replay-time observations.

If a scenario is hand-authored (`producedBy.producer = "llm-author"` / `"human"`), `recording/` MAY be absent. Consumers MUST treat its absence as "no record-time observations available," not as an error.

### `replays/<runId>/`

One per replay run. The first action of a replay is to mint a `runId` and create `replays/<runId>/audit.json` with `startedAt`. The last action (success or failure) is to write `finishedAt`, `exitCode`, and the literal `summary` line the runner emitted to stderr (e.g. `SUMMARY: 9/9 (PASS)`).

`replays/latest.txt` is a one-line pointer file whose content is the `<runId>` of the most recent replay. Tools that want "show me the latest run" read this file; they MUST NOT sort directory entries by timestamp (the directory may be on a filesystem that does not preserve sort order, and a fresh run that has only written `audit.json` may legitimately be the latest while still mid-flight).

### `recording/failed/`

One directory per refused step, named `assert-refused-<stepId>-<isoTs>/`. Created by the assert-quality gate when it refuses a `check` step at record time — for example, when the author tries to assert a URL match that no signal on the page can support, or when the recorded claim would be false on replay.

Each refusal directory holds a complete evidence bundle — everything a triager (human or LLM) needs to understand what was refused and why, without having to re-run the recording:

- **`payload.json`** — the structured exit-4 diagnostic the gate emitted: the rejected claim, the signals that were actually available on the page, and a hint pointing at what to change. This is what an LLM or a human reads to fix the recording.
- **`snapshot.txt`** — the ARIA accessibility-tree snapshot at refusal time. The greppable "what a screen reader would announce" view of the page.
- **`screenshot.png`** — full-page screenshot at refusal time. The visual the reviewer's eye lands on first.
- **`probe.json`** — the DOM probe at refusal time (dialogs / listboxes / toasts / URL / body text). What the runner would have observed as the page state.
- **`network.json`** — recent GraphQL / REST calls observed around the refusal. Server errors here are often the "why" behind a refused assert (the create-user mutation failed, the page never updated, the assert tried to bind to state that doesn't exist).

The complete bundle means a triager can decide "is this a recording bug or an app bug" from the refusal directory alone, without re-running the failing record-step against a live tab.

On retry of the same step, the producer moves any pre-existing keyframe matching the refused step into a fresh `assert-refused-<stepId>-<retryTs>/` so the new attempt's capture lands cleanly. The original refusal directory stays put as the audit trail. If no retry happens, the refusal directory is the scenario's record of "we tried and refused, here is why."

`recording/failed/` MAY be absent for scenarios that never hit a refusal (the common case).

### `heal-requests/` + `heal-responses/`

Record-side caller-driven heal pipeline. When `smart-click` submits and the backend rejects with a structured payload (alert text + GraphQL `errors[]`), the recorder writes:

- **`recording/heal-requests/<stepId>.json`** — schema `scenario-heal-request/v1`. Carries the structured rejection evidence (`signal.networkErrors[]` with `postData` + `errors[].pointer`), the originally attempted value (when known), instructions for the caller, and a JSON-Schema description of the expected response shape. The producer is `cli/src/smart_click.rs`; consumers are the LLM driving the recording, `agent-qa heal-respond`, and `agent-qa heal-apply`.

- **`recording/heal-responses/<stepId>.json`** — schema `scenario-heal-response/v1`. Written by `agent-qa heal-respond` with the caller-proposed `correctedValue` + `rationale`. In record mode the response payload also carries `stepsHashBefore` (SHA-256 of `tmp/scenario.steps.jsonl` at respond time) — used by `agent-qa heal-apply` as a rebase guard.

- **`recording/heal-requests/<stepId>.applied.json`** — idempotency marker. On success, `agent-qa heal-apply` renames the request file from `<stepId>.json` to `<stepId>.applied.json`. Consumers that read `heal-requests/` MUST account for both extensions: bare `.json` means "pending heal, run heal-apply"; `.applied.json` means "heal already landed in `tmp/scenario.steps.jsonl`, the corresponding `caller-driven-resolved` row is in `heal.jsonl`."

Replay-side equivalents live under `replays/<runId>/heal-requests/` and `replays/<runId>/heal-responses/`. The replay-side request file is written by the runner on a caller-driven exit-7; the corresponding response is loaded back via `agent-qa replay --heal-from-run <runId>` to substitute the corrected value at step dispatch time. The replay side has no `.applied.json` marker — the response is applied transiently, not persisted into the contract.

Both directories MAY be absent (and usually are — most recordings never trigger a backend rejection).

### `failed/truncate-<isoTs>[-<archive-tag>]/`

Top-level `<SID>/failed/` (sibling of `recording/` and `replays/`, not under `recording/`) holds one directory per `agent-qa truncate <N>` call. The directory name is `truncate-<isoTs>[-<archive-tag>]/` where `<archive-tag>` is the optional `--archive-tag <slug>` argument (slug must match `[A-Za-z0-9._-]+`); without a tag, sub-second `isoTs` ties can collide on back-to-back truncations in one heal session.

Each store holds the per-step sidecar files for the dropped step ids, moved (not copied) from their `recording/<kind>/` source paths:

```
<SID>/failed/truncate-<isoTs>[-<tag>]/
├── snapshots/<stepId>.txt
├── screenshots/<stepId>.png
├── network/<stepId>.json
├── probes/<stepId>.json
├── perf/<stepId>.json
└── manifest.json             # schema: "truncate-store/v1" — see below
```

The `manifest.json` carries: the SID, the timestamp, the `toStepIndex` truncated to, the count and stepIds of dropped rows, the count of moved sidecar files, and the optional `archiveTag`. Triagers grepping `<SID>/failed/` for a stepId pinpoint when and why the truncate fired without rerunning the recording.

The verb itself is pure on-disk bookkeeping — it never drives the live tab. The agent owns the tab gesture via `agent-browser` primitives before truncating.

`<SID>/failed/` MAY be absent for scenarios that never hit a truncate (the common case).

### `network/`

Per-step network sidecars under `recording/network/<stepId>.json` and `replays/<runId>/network/<stepId>.json`. The file is a JSON object with the shape:

```json
{
  "stepIndex": 3,
  "capturedAt": "2026-05-19T12:54:21.471Z",
  "errors": [
    {
      "requestId": "…",
      "operationName": "createUser",
      "httpStatus": 200,
      "messages": ["…"],
      "postData": "…",
      "responseBody": "…"
    }
  ],
  "allRecent": [
    {
      "requestId": "…",
      "operationName": "…",
      "httpStatus": 200,
      "hasErrors": false,
      "postData": "…",
      "responseBody": "…"
    }
  ]
}
```

- `stepIndex` — the position the step held in `scenario.json` at write time (informational; readers key by `<stepId>` from the filename, not from this field).
- `capturedAt` — ISO timestamp.
- `errors[]` — derived subset of `allRecent[]` where `hasErrors === true`, with parsed GraphQL error messages lifted to the top level for greppability.
- `allRecent[]` — the raw response objects the producer's network collector returned (last N — typically 30 — by the time the step completed).

Producers MUST NOT emit HAR (W3C store format). Earlier drafts of this spec named `.har` as the default; that was aspirational and never implemented. The shape above is the actual contract. Consumers that want HAR can convert at read time, but the on-disk artifact is JSON.

Refusal-time network sidecars (`recording/failed/<…>/network.json`) use the same shape minus `stepIndex` (a refusal isn't bound to a step position).

## Path conventions

These rules are intentionally short and unambiguous. A producer that writes a sidecar without consulting this section is the bug.

- **Step-keyed files use `<stepId>.<ext>`.** The `<stepId>` is the literal `id` field on the step in `scenario.json` (regex `^[A-Za-z0-9._-]+$` per schema). One file per step per sidecar kind, no zero-padding, no synthetic index. Example: a step with `id: "openDialog"` writes `replays/<runId>/snapshots/openDialog.txt`.
  - This is the deliberate departure from v1's `000.txt` / `001.txt` zero-padded index naming. `stepId` is stable across reorderings; positional indices are not. v1 recordings are not converted (hard cut per `plan.md`'s validation strategy).
- **Run-keyed directories use `<runId>` of the form `<isoTimestamp>__<shortHash>`.** The timestamp matches the SID format (`YYYY-MM-DDTHH-mm-ss-sssZ`, colons replaced with dashes for filesystem safety) and the short hash is 8 lowercase hex chars. Example: `replays/2026-05-19T12-54-21-471Z__a1b2c3d4/`. Producers MAY append a profile label as a third segment for human readability (`__a1b2c3d4__default`), but tooling MUST identify a run by directory name as a whole, not by parsing segments out.
- **Extensions are fixed per sidecar kind.** `.json` for probes / perf / audit / network, `.txt` for ARIA snapshots, `.png` for screenshots, `.jsonl` for heal events. A consumer can decide how to parse a file from its kind directory + extension without sniffing content. (Earlier drafts of this spec named `.har` as the network extension; that was aspirational — no producer ever wrote HAR. See § `network/` for the actual shape.)
- **Sidecar paths are case-sensitive lowercase for directory names** (`snapshots/`, `screenshots/`, `network/`, `probes/`, `perf/`, `replays/`, `recording/`). File names inherit case from the step `id`, which is author-controlled — producers MUST preserve the author's casing verbatim.
- **Sidecar files are written atomically when possible** (write to `<path>.tmp`, then rename). Readers that find a `.tmp` file MUST treat it as in-flight, not as canonical data. This rule matters most for `heal.jsonl` and `audit.json`, which are written incrementally during a long-running replay.

## Stability guarantee

The directory tree IS the contract for sidecar discovery. Consumers can rely on the following without coordinating with producers:

1. **Path lookup is the discovery mechanism.** To find the screenshot for step `openDialog` from run `<runId>`, a tool reads `<SID>/replays/<runId>/screenshots/openDialog.png`. No index file, no manifest, no JSON-pointer chase. If the file is not present, the producer chose not to write it (e.g. screenshots disabled for this run); the absence is meaningful and is NOT an error.
2. **Adding a new sidecar kind is non-breaking.** Future work may introduce `<SID>/replays/<runId>/coverage/` or `<SID>/recording/a11y/<stepId>.json` without bumping the spec version. Existing consumers that did not know about the new kind simply do not look at it.
3. **Renaming an existing sidecar kind is breaking** and requires a spec version bump (`scenario-sidecar-tree/v2`). The same applies to changing the keying scheme (e.g. moving from `<stepId>` back to `<index>`), changing the run-id format, or removing the `recording/` vs `replays/<runId>/` split.
4. **The schema file's `description` field is allowed to reference this document by relative path.** A spec version bump that moves this file MUST update the schema reference in the same change.

Producers MUST NOT invent additional layout _within_ a kind directory. If a step needs to write multiple files (e.g. a screenshot AND a thumbnail), the right answer is two kind directories (`screenshots/`, `thumbnails/`), not `screenshots/<stepId>/full.png` + `screenshots/<stepId>/thumb.png`. The flat-per-kind shape keeps glob patterns and diff tools simple.

## What does NOT belong here

These fields are explicitly forbidden inside `scenario.json`. They live ONLY in sidecars under `recording/` or `replays/<runId>/`. This is the enforcement of P1 — if any of these creeps back into the contract, the design's strongest position has been compromised and a future agent will start making replay decisions from recorded observations.

- ❌ **`evidence`** on a step or on the root. The recorded snapshot/screenshot/network for step X lives at `recording/snapshots/<X>.txt` etc., not as `step.evidence.snapshot.path`.
- ❌ **`healHistory`** — every heal event ever proposed against the step. Lives in `recording/heal.jsonl` (if heal fired during authoring) and `replays/<runId>/heal.jsonl` (per-run heal trail). The contract carries one locator per step; if heal wants to suggest a rewrite, that suggestion is a diff a human reviews and accepts into a new `scenario.json`.
- ❌ **`candidates`** — the list of alternate locators tried before one resolved. Lives in `replays/<runId>/heal.jsonl` per step.
- ❌ **Click effects, ARIA hashes, snapshot hashes, screenshot hashes** referenced from steps. The path convention IS the reference; a hash field is an invitation to drift between the contract and the sidecar.
- ❌ **Replay verdicts** (`step.lastReplayStatus`, `step.lastReplayedAt`). Lives in `replays/<runId>/audit.json` and `replays/<runId>/heal.jsonl` per step.

If a producer finds itself wanting to add any of these to `scenario.json`, the answer is "write it to the appropriate sidecar and let consumers discover it by path." If the right sidecar kind does not yet exist, add a new kind directory (per the [Stability guarantee](#stability-guarantee) above) — that is non-breaking; adding a field to the contract is not.

## Relation to JSON Schema

The sidecar layout is not encoded in [`scenario-schema.json`](../../cli/src/scenario-schema.json). This is deliberate.

JSON Schema describes the _data contract_ — the shape of the bytes inside `scenario.json`. The sidecar tree is _convention_ — the shape of the directory next to it. Encoding the convention in the schema would require introducing JSON fields whose only purpose is to repeat what the directory layout already says (e.g. a `sidecars: { snapshots: "snapshots/<stepId>.txt" }` block). Those fields would be redundant, would invite drift between the declared and actual layout, and would re-introduce exactly the contract-and-audit coupling that P1 forbids.

The schema's top-level `description` field points readers at this document. That cross-reference is the only link between the two specs, and it is one-way: the schema knows about the convention; the convention does not depend on a particular schema version (a future `scenario/3` can keep `scenario-sidecar-tree/v1` if its sidecar shape is unchanged).

Validating that a scenario directory follows this convention is a separate concern from validating that `scenario.json` follows the schema. A `validate-sidecar-tree` tool (out of scope for this spec) would walk the directory and check the rules in [Path conventions](#path-conventions) and [What does NOT belong here](#what-does-not-belong-here); a JSON Schema validator only sees `scenario.json`.

## Cross-references

- [`../../cli/src/scenario-schema.json`](../../cli/src/scenario-schema.json) — the data contract whose `description` defers here.
