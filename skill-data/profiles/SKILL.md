---
name: profiles
description: Multi-profile scenario replay & comparison. Read this when running the same recorded scenario under different profiles (default-user, admin-user, ...), comparing what each profile sees, or adding a new profile. Companion to `core.md` — that one covers single-profile recording; this one is the deep-dive on profile management and N-way comparison.
---

# Profiles — multi-profile scenario replay & comparison

How to record once, replay under different profiles, and compare what each profile sees. Companion to [`SKILL.md`](./SKILL.md) — the main skill stays focused on single-profile recording; this file is the deep-dive for multi-profile work.

## Mental model

- **Recording is profile-agnostic.** `scenario.json` does not store which profile recorded it. A scenario is a spec; replays are executions.
- **Replay binds a profile at command time.** Pass `--profile default-user` (or `admin-user`); the scenario runs against that profile's authenticated Chrome session.
- **Comparison is replay-vs-replay only.** Recording-time keyframes (`<sid>/recording/`) are NEVER used as a diff side. They stay as the immutable canonical-recording artifact.
- **Replay-only across profiles.** If a step can't continue under one profile (missing affordance, 404 page, 403, "permission denied" alert), replay halts at that step, records a sentinel keyframe, and marks the step `gated`. The keyframe IS the comparison signal.

## Profiles

| profile        | session name           | user-data-dir                     | env-var prefix              |
| -------------- | ---------------------- | --------------------------------- | --------------------------- |
| `default-user` | `default-user-session` | `~/.agent-qa-chrome/default-user` | `AGENT_QA_PROFILE_DEFAULT_*` |
| `admin-user`   | `admin-user-session`   | `~/.agent-qa-chrome/admin-user`   | `AGENT_QA_PROFILE_ADMIN_*`   |

Adding a new profile is `agent-qa profile-add <id> --plugin <auth-plugin> [--session <name>] [--leaf <dir>]`. The plugin owns the actual sign-in (OAuth, cookie injection, whatever); agent-qa records the registration in `~/.agent-qa/profiles/<id>/profile.json`.

## Required `.env`

```
AGENT_QA_PROFILE_DEFAULT_EMAIL=<default-user-email>
AGENT_QA_PROFILE_DEFAULT_PASSWORD=<default-user-password>
AGENT_QA_PROFILE_ADMIN_EMAIL=<admin-user-email>
AGENT_QA_PROFILE_ADMIN_PASSWORD=<admin-user-password>
AGENT_QA_CLIENT_ID=<staging oauth client id>
```

Missing creds for a profile: `profile-bootstrap <profile>` fails loud with the missing variable name.

## Three command shapes

All three accept N >= 2 profiles / replays. N=2 is the common case; N=3+ matrix comparisons surface "what does profile X have that no other profile has" patterns. There is no special-case code path for any value of N — the output structure and markdown rendering scale uniformly.

### Shape A — one-shot compare

```bash
agent-qa compare <sid> --profiles default-user,admin-user [--force] [--serial]
agent-qa compare <sid> --profiles default-user,admin-user,viewer
agent-qa compare <sid> --profiles all   # every registered profile
```

1. Bootstraps each profile (probe-then-auth; near-no-op if already authenticated).
2. Runs/reuses one replay per profile. Current scenario/v1 profile execution is serial; `--serial` is accepted for explicitness.
3. Reuse rule: per profile, if a recent replay exists with `meta.scenarioContentHash` equal to the current `scenario.steps[]` hash AND `status === 'ok'`, reuse it. `--force` always re-replays. Cosmetic edits to `scenario.json` (instruction text, comments, anything outside `steps[]`) do NOT invalidate replays.
4. Diffs the N most-recent per-profile replays via the engine.

### Shape B — single-profile replay

```bash
agent-qa replay <sid> --profile <p>
```

Calls `profile-bootstrap <p>` first, then runs the deterministic replayer. `replay` is single-profile; use Shape A when you want multiple profiles.

Use Shape B when you want the replay artifacts on disk but want to think before comparing (or to add a third profile's replay later for a Shape C diff).

### Shape C — diff existing replays

```bash
# Helper: list every replay for a scenario (newest first, copy-paste CSV at bottom)
agent-qa compare <sid> --list-replays

# Diff any N >= 2 replays (alphabetised + deduped automatically)
agent-qa compare <sid> --replays <id1>,<id2>[,<id3>,...]
```

No bootstrap, no replay — just diffs the named replay folders under `<sid>/replays/`. Accepts ANY N >= 2 replay ids. Mode is auto-detected from each replay's metadata. Examples:

- `default-user@today` vs `admin-user@today` → cross-profile, tight time, same sha
- `default-user@today` vs `default-user@last-week` → cross-time same-profile (regression detection)
- `default-user@today` vs `admin-user@today` vs `viewer@today` vs `readonly@today` → 4-way cross-profile matrix
- mixed combinations → mode descriptor expresses both axes (`profileSet`, `timeSpread`, `commitShaSpread`)

`--profiles` and `--replays` are mutually exclusive. Both flags dedupe + alphabetically sort their values, so re-running the same N produces deterministic folder names.

## Comparison mode descriptor

There is no single-enum mode field. The mode is a **structured descriptor** stored on `meta.mode` of each comparison, with three orthogonal axes:

| Axis              | Values            | Meaning                                                               |
| ----------------- | ----------------- | --------------------------------------------------------------------- |
| `profileSet`      | `string[]` (N>=2) | Alphabetically-sorted set of profiles in this comparison              |
| `timeSpread`      | `tight` \| `wide` | `tight` if `max(startedAt) - min(startedAt) < 10min`, else `wide`     |
| `commitShaSpread` | `same` \| `mixed` | `same` if all replays share the same `commitSha`, else `mixed`        |

Common patterns derive from the descriptor:

- `profileSet.length >= 2`, `timeSpread = tight`, `commitShaSpread = same` → cross-profile (default-user vs admin-user, run together)
- `profileSet.length === 1` (or duplicate-profile labels), `timeSpread = wide` → cross-time (regression on same profile)
- `commitShaSpread = mixed` → some deltas may be build changes, not real product changes (the report surfaces a prominent warning)

## Output contract per comparison

Every `compare` run produces two top-level files plus per-pair artefacts in `<sid>/compare/<isoTs>__<p1>-vs-<p2>[-vs-<p3>...]/`:

| File                                            | Purpose                                                                                          |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `compare.md`                                    | Human report. Counts → Per-pair verdict overview → Per-step deltas.                              |
| `compare.json`                                  | Machine source-of-truth. `meta` (mode descriptor + per-profile replays{} + per-step verdicts).  |
| `pairs/<base>-vs-<cand>/summary.md`             | Per-step verdict table for one pair (snapshot / screenshot columns).                            |
| `pairs/<base>-vs-<cand>/<NNN>.snapshot.diff`    | Unified ARIA text diff for one step (only when changed).                                         |
| `pairs/<base>-vs-<cand>/<NNN>.screenshot.diff.png` + `.baseline.png` + `.candidate.png` + `.compare.png` | Pixelmatch overlay + side-by-side triptych for one step (only when changed). |

Per-pair verdicts are N-way native: each step carries a per-profile presence/outcome, so a future reader can see "present in baseline, absent in candidate" without A/B fields.

## Gating vs error

Replay distinguishes two failure modes per step:

- **`gated`** — page legitimately denied. Sniffed by:
  - URL contains `/forbidden`, `/404`, `/login`
  - Body text matches "couldn't find this page", "you may not have permission"
  - `[role="alert"]` contains permission/authorization language
  - The step's error itself is "no element found" / "role not found" (missing-affordance — most common gated case)
- **`error`** — mechanical failure (adapter crash, network failure, broken state-change check)

Both halt the replay loop (subsequent steps depend on this one) and record a sentinel keyframe. The difference matters because:

- `gated` runs feed the cross-profile diff section as valid signal — the diff at the gating step IS the answer to "what does default-user lack."
- `error` runs feed the adapter / runner gap sections as defects to fix — they should NOT inform conclusions about profile differences.

`replay` exits 5 on `error`, 0 on `gated` or `ok` — so callers (including compare) can distinguish broken replays from useful gated ones.

## Bootstrap + auth

`profile-bootstrap <profile>` is idempotent. Per profile:

1. `mkdir -p ~/.agent-qa-chrome/<leaf>` (silent on first creation; one log line; gated by hard-coded profile-name allowlist so typos can't litter the filesystem).
2. Probe: is `<profile>-session` session alive AND on a non-login URL? If yes → no-op. Otherwise →
3. Open the session at `https://app.example.com/`.
4. Invoke the configured `auth` plugin's `login` op for `<profile>` — the plugin returns a JS injection string (cookies + localStorage) or performs its own OAuth dance. agent-qa never bakes auth logic in-tree.
5. Pipe the plugin's injection script into `agent-browser eval --stdin` to seed cookies + localStorage on the staging domain.
6. Wait 5s for OIDC settle.
7. Apply the shared `networkPolicy` (in-app-messaging block).

`profile-status <profile>` is the side-effect-free preflight: prints `authenticated` / `needs-bootstrap`, exits 0 / 1 accordingly. Use in scripts that want to check before invoking bootstrap.

## Replay reuse policy in Shape A

Per profile, skip a fresh replay iff:

- the existing `replay.json.meta.scenarioContentHash` equals `sha256(scenario.steps[])` of the current `scenario.json` (the spec — specifically the steps — hasn't changed since the replay), AND
- replay status is `ok` (gated/error replays are not valid executions of the spec — re-replay anyway), AND
- `--force` was NOT passed.

Otherwise: re-replay. The hash-based check means cosmetic edits to `scenario.json` (changing `meta.instruction`, adding comments, anything outside `steps[]`) do NOT invalidate existing replays — only changes that affect what gets replayed do.

`--force` exists for **product-drift testing** — when the scenario hasn't changed but staging has, and you want fresh data to surface what changed.

## When recording-time keyframes are NOT used

Comparison is replay-vs-replay only. Recording-time keyframes:

- have stale values from `{{vars._unique}}` templates (literal record-time values, not replay-time fresh ones).
- carry no record-time profile label (we don't know which profile recorded the scenario).
- may have been recorded against a different build than today.

Mixing them with replay keyframes would produce bogus deltas. The `compare.md` provenance footer mentions when the scenario was recorded, so the reader can trace back to original intent — but never as a diff side.

## Single-profile recording (unchanged)

If you're not doing multi-profile work, ignore this file and use the single-profile flow in `SKILL.md`. The default profile is `default-user`; pass `--profile <p>` on recording/replay verbs to resolve the matching `<p>-session` session, or `--session <name>` when you deliberately want to bypass profile resolution.

## Adding a third profile (e.g. `viewer`)

Two files, both must be edited together:

1. `agent-qa profile-add viewer --plugin <auth-plugin> --leaf viewer` (registers the profile + ties it to an auth plugin).
2. Configure credentials per your auth plugin's contract (typically env vars or a credentials file the plugin reads).

## Troubleshooting

| Problem                                                                  | Cause                                                                          | Fix                                                                                                                                                                 |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `[profile] unknown profile "X" — known: default-user, admin-user`        | Typo in `--profile` arg                                                        | Use one of the listed profiles, or add a new one (see above)                                                                                                        |
| `Missing credentials for profile "default-user"`                         | Missing `.env` variables for that prefix                                       | Add `AGENT_QA_PROFILE_DEFAULT_EMAIL` + `AGENT_QA_PROFILE_DEFAULT_PASSWORD`                                                                                            |
| `replay --profile a,b` exits non-zero but reports both replays completed | `error`-classified step in one of the runs                                     | Inspect `replay.json.steps[]` for the failing step; fix the runner / page / adapter; re-run                                                                         |
| Comparison folder name has weird chars                                   | Replay id contains the profile suffix; engine hashes it for the folder name    | Cosmetic only — the `compare.json.meta` carries clean profile labels                                                                                                |
| `Auto-launch failed: All CDP discovery methods failed for 127.0.0.1`     | Orphan agent-browser daemon (child Chrome died, daemon PID alive).             | Auto-recovered by agent-qa once per invocation; if that fails, `npx agent-browser close --session <profile>-session` then retry. See `core/references/gotchas.md`. |

## Cross-references

- Comparison engine + per-step verdict schema: `cli/src/compare/mod.rs`

## See also

- [`byo` skill](../byo/SKILL.md) — drive the user's own browser (Chrome, Brave, Brave Nightly, Edge, Arc, Vivaldi) instead of a registered profile. Explicit per-turn opt-in only; never a default. Load when the user explicitly asks to use their own browser.
