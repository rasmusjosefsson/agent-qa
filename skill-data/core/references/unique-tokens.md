## Unique fields & template tokens

When the page asks you to **create** something the server enforces uniqueness on (account name, email, domain, username, …), recording the literal value would make the second replay collide on the server. Instead, use `fill-unique` which records a scenario/v1 template; the replayer generates a fresh value each run.

```bash
# Replaces the manual `agent-browser fill … + record-step verb action fillByLabel` pair.
agent-qa fill-unique "Account name" --template '{{vars._unique}}'
agent-qa fill-unique "Email" --template 'qa-{{vars._unique}}@scenario-test.example.com'
agent-qa fill-unique "Domain" --template '{{vars._unique}}.example.com'
agent-qa fill-unique "Account name" --template 'Acme {{vars._unique}}' --save-as accountName
agent-qa fill-unique "Username" --template 'user_{{vars._unique}}-test'
```

The template is a literal string with **one or more `{{vars._unique}}` slots**. At record time, every slot in one field is replaced by the same `rec-<sid8>-<step>-<nonce>` value. At replay time, the replayer mints its own fresh value. Legacy unique-token grammar and bare `{{name}}` recording are not supported.

Use `--save-as <name>` when a later assert, fill, or URL pattern needs the same minted value. The name must be a safe key and cannot be empty, `profiles`, or an `Object.prototype` property name.

Token mapping (record-time → replay-time):

| Template example                     | Record-time generated value                        | Replay-time generated value                 |
| ------------------------------------ | -------------------------------------------------- | ------------------------------------------- |
| `qa-{{vars._unique}}@example.com`    | `qa-rec-<sid8>-<step>-<nonce>@example.com` | `qa-rep-<replayId8>-<step>@example.com` |
| `{{vars._unique}}.example.com`       | `rec-<sid8>-<step>-<nonce>.example.com`    | `rep-<replayId8>-<step>.example.com`    |
| `Mock {{vars._unique}}`              | `Mock rec-<sid8>-<step>-<nonce>`           | `Mock rep-<replayId8>-<step>`           |
| `{{vars._unique}}--{{vars._unique}}` | same record-time slot in both positions            | same replay-time slot in both positions     |

`<sid8>` = last 8 chars of the scenario id; `<replayId8>` = 8 chars from the replay id; `<step>` = current step index. Together they guarantee per-run uniqueness without collisions across multiple unique fields in the same scenario.

**When NOT to use `fill-unique`:** any field that must MATCH an existing record (selecting an existing user from an "Assigned" dropdown, picking an existing tag, choosing a preset option). Those stay as literals via plain `agent-browser fill` + `record-step action`. Rule of thumb: **CREATE → unique; SELECT → literal**.

Every substitution made during replay is logged to `replays/<replayId>/replay.json` under `mutations[]`, so you can see exactly which value was used in any given run.

After recording + verify, replay the scenario deterministically to confirm a non-LLM script can reproduce the same end-state. Useful as proof-of-replayability before handing the scenario to the user.

```bash
agent-browser --session default-user-session open 'https://app.example.com/?deployment=Staging'   # reset tab
agent-browser --session default-user-session wait 2000

agent-qa replay --profile default-user    # replays the most-recently-started scenario
agent-qa diff      # confirms replay reproduced the recorded state
agent-qa list      # pretty step→keyframe table + replay history + summary.md
```

`replay` re-applies the network blocks, walks `steps[]`, dispatches each step to `agent-browser` via `snapshot+ref` clicks, and writes a final sentinel keyframe + `replay.json` manifest under a fresh **`replays/<replayId>/`** subfolder (where `<replayId>` is the run's ISO timestamp). The original recording in is never touched. `diff` then structurally compares the original final-step keyframe to the latest replay sentinel — `✓ structurally identical` confirms replay reproduced the recorded state. Exit code is 0 on identical, 1 on differences. Re-run `diff <sid> <replayId>` to pin a specific historical replay.

> **Agents: NEVER replay an auth-gated scenario without `--profile`.** `scenario.json` does not store the recording profile (recording is profile-agnostic), but the scenario's `replays/<isoTs>__<profile>/` folder names DO. When the user says "replay <sid>" without naming a profile, in this order:
>
> 1. **Conversation context** — if a profile was just used (record/bootstrap/replay/compare in this session), reuse it.
> 2. **Most-recent replay on disk** — `ls -t tmp/agent-qa-scenarios/<sid>/replays/` and parse the profile suffix from the newest folder name (`<isoTs>__<profile>/`). Use that profile.
> 3. **Ask the user** — only if neither (1) nor (2) yields a profile.
>
> Don't fall back to "no profile" silently — auth-gated scenarios (anything on `app.example.com`, `app.example.com`) will redirect to login on step 1 and halt with a misleading "element not found" error.
>
> **When you summarise a finished recording for the user, ALWAYS print the replay command with the resolved profile baked in** — never the literal `--profile <p>` placeholder. The recording profile is whatever you passed to `record-step` / `smart-click` / `fill-unique` (defaults to `default` since `start` defaults that way too). Copy-paste-ready commands prevent the user from re-running it without `--profile` and re-discovering the login-redirect failure mode.

### Folder layout

```
tmp/agent-qa-scenarios/<SID>/
├── scenario.json                                     ← canonical recording (immutable)
├── summary.md                                       ← written by list verb
├── probes/                                          ← per-step DOM probe sidecars
│   ├── 000.json                                     ← {alerts, dialogs, toasts, network, url}
│   └── 00N.json                                     ← `network` slot pre-populated with GQL errors[]
├── snapshots/                                       ← per-step ARIA snapshot sidecars
│   ├── 000.txt                                      ← `agent-browser snapshot -i` output, post-settle
│   └── 00N.txt                                      ← greppable; `diff snapshots/004.txt 005.txt` shows diff
├── screenshots/                                     ← per-step full-page screenshots
│   ├── 000.png                                      ← `agent-browser screenshot --full`, post-settle
│   └── 00N.png                                      ← sibling of snapshots/; pair them when triaging
└── replays/
    ├── 2026-04-29T14-30-00-123Z/                    ← one folder per replay run
    │   ├── replay.json                              ← manifest (started/ended, finalUrl, diff result)
    │   └── 00N__<slug>.json                         ← sentinel keyframe (plain NNN — folder isolates it)
    └── 2026-04-29T14-45-12-456Z/
        ├── replay.json
        └── 00N__<slug>.json
```

**Triage tip:** the `probes/NNN.json` `network` slot is populated AT RECORD TIME with any GraphQL response carrying `errors[]` (capped at 10 per step, gathered from the last 30 GQL POSTs). Reading the probe sidecar of a quarantined scenario gives you the same signals as `cli inspect` on a live tab — no re-running needed. The `snapshots/NNN.txt` sidecars are the cheapest possible diff input: they're the raw a11y tree, so `diff snapshots/<step-1>.txt snapshots/<step>.txt` shows exactly what the gesture changed. The `screenshots/NNN.png` sidecars sit next to the snapshots so you can pair them when triaging — read the snapshot text first, open the picture for visual confirmation. Skip with `SKIP_PROBE_NETWORK=1` / `SKIP_SNAPSHOT=1` / `SKIP_STEP_SCREENSHOT=1` if the extra round-trip per step matters.

Note: `screenshots/NNN.png` is the routine post-settle frame for every step. The separate `probes/NNN.png` (only present when the heal loop's failure detectors fire) is failure-time evidence — distinct artifact, distinct meaning, do not conflate.

Sentinel keyframes use the **same step index** as the original (no 900 offset) since they live in their own folder. That makes the original ↔ replay mapping trivial: `snapshots/006.txt` ⇄ `replays/<id>/snapshots/006.txt`.
