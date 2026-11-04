# Workbench

`agent-qa web` opens a localhost UI for the same scenarios root the CLI uses. You describe a test in Chat, hand-edit one in the Editor, or write a case in plain English and let the agent record it. Sets group cases, plans run a scope, and Runs shows each replay with its evidence.

Every record, flush, and replay shells the `agent-qa` CLI, so a scenario written here is the same `scenario.json` a terminal run would produce.

```bash
agent-qa web                         # http://127.0.0.1:7878, opens a browser
agent-qa web --port 7890 --no-open   # bind another port, stay in the terminal
agent-qa web --root ./scenarios      # view a scenarios root other than the default
```

| Flag | Effect |
| --- | --- |
| `--port <N>` | Bind `127.0.0.1` (default `7878`). |
| `--root <dir>` | Scenarios root. Otherwise `AGENT_QA_SCENARIOS_DIR`, then `[paths].scenarios_root`, then `<cwd>/tmp/agent-qa-scenarios`. |
| `--no-open` | Do not open a browser tab. |

`agent-qa report view` still starts the same server and prints a deprecation notice.

The shell has a light/dark theme toggle. A fresh checkout has no UI bundle until `npm --prefix npm/agent-qa run build:web` (publishing rebuilds it). Without the platform CLI binary, Runs can still show evidence already on disk; Chat and the Editor need that binary (`AGENT_QA_BINARY_PATH` overrides discovery).

## The sidebar

| Group | Page | What you do there |
| --- | --- | --- |
| Create | [Chat](#chat) | Talk to an agent that drives a browser and records a scenario. |
| Create | [Editor](#editor) | Build or extend a scenario against a live page. |
| Test pipeline | [Cases](#cases) | Write a test in plain English. |
| Test pipeline | [Sets](#sets) | Group cases you run together. |
| Test pipeline | [Plans](#plans) | Pick a scope and run it. |
| Test pipeline | [Runs](#runs) | Replay, inspect, and compare. The home page. |
| Test pipeline | [Goldens](#goldens) | Review screenshot and snapshot baselines. |
| Setup | [Personas](#personas-and-environments) | A login: profile plus credentials. |
| Setup | [Environments](#personas-and-environments) | A target: base URL, params, auth. |
| Setup | [Knowledge](#knowledge) | Import tests that already live somewhere else. |
| Setup | [Extensions](#extensions) | Install plugin packages and the browser recorder. |
|  | [Settings](#settings) | Chat backend, headed replay, golden storage. |

Cases, Sets, Plans, and Runs show the same four-step strip, and each step links to its page. Leaving a case, set, or plan with unsaved edits asks before the form is dropped.

## Chat

Chat is an agent with its own browser. Describe a flow ("sign in and check the dashboard") and the agent drives the page, records steps, and flushes a scenario you can open from Runs.

Each chat is a separate conversation and a separate agent-browser session (`chat-<rand>`). Chats run side by side without sharing a tab. The list is stored under the scenarios root, so restarting `agent-qa web` restores it and reopening a chat resumes the conversation. See [Chats](configuration.md#chats) for the on-disk record.

The page is split: the transcript on one side, the chat's live browser on the other. While the agent is recording, a recording view shows the steps as they land. The header names the backend, model, and cumulative usage. **Copy** puts the whole conversation on the clipboard as markdown (tool calls included, thinking dropped).

An empty chat offers starter prompts: record a login, fill and submit a form, replay the latest scenario, list scenarios, inspect a page, explain a failure. **Cases → Run with agent** and a failed step's **Ask agent** open a new chat with the prompt already filled in.

**Sign in as** picks a persona for this chat's browser. The empty choice is **No sign-in** (guest). **Connect** resolves that persona's credentials and bootstraps its profile. If that sign-in needs a person in the loop, **Retry in a visible browser** opens a window for it. **On** picks an environment when you have defined one.

The model provider is separate from the persona. When a provider is still signed out, Chat shows its own **Connect**: Cursor runs its login flow, and another provider opens that provider's login page or takes an API key in the banner. The key is handed to that provider once and is not written into the repo. `CURSOR_API_KEY` and a provider variable such as `OPENAI_API_KEY` in the workbench process are the other way in. Which agent runtime answers is [Settings → Agent backend](#settings): `auto` (default), `cursor`, `pi`, or `opencode`. `AGENT_QA_NO_CHAT=1` removes the Chat tab. Backend details and the pi `/login` hint are in [Configuration](configuration.md#environment-variables).

## Editor

The Editor is the hand-authoring surface. It shells `run-step`, `aria-snapshot`, and `buffer`; the CLI still owns every mutation. See the note under Recording in [CLI verbs](verbs.md).

Three columns:

- **Session and steps.** Start a recording with an intent and a URL, or open a scenario that already exists. Pause freezes capture (the same `record pause` the CLI uses). Reorder, edit, delete, or re-run a single step. A camera on a do-step appends a shot claim. **Check** runs `verify` against the live session. **Flush** writes `scenario.json`.
- **Live page.** A screencast of the session. The click mode decides what a pointer does: **Browse** drives the page, **Record** turns clicks, typing, selects, checks, key presses, uploads, dialogs, viewport changes, and downloads into steps, **Pick element** fills the composer from the control under the cursor, and **Gesture** records a touch gesture (a swipe or a hold) as a step. In Record mode, the storage button writes a `do/state` step that seeds the page's storage and cookies.
- **Element picker.** An ARIA snapshot of the page. Choosing a row fills role and accessible name on the step you are composing.

The composer can add a step without touching the canvas: navigate, click (role + name), type, press a key, wait, assert present, assert absent, or assert the URL. **Run step** dispatches it once for feedback and does not record. **Record step** appends it.

## Cases

A case is the test in plain English, before or beside a recording. **New case** asks for a title and stores `case.json`.

| Field | What it is |
| --- | --- |
| Start URL | Where the browser opens before step 1. |
| Preconditions | State that has to be true first. Optional. |
| Steps | One action per line. `[EMAIL]` style tokens become test-data rows. |
| Expected result | What proves the case passed. Tokens work here too. |
| Tags | Labels a set can match. Comma-separated. |
| Test data | One row per `[TOKEN]`. Mark a value sensitive to keep it out of the recording. |
| Recording | Attach a scenario that already exists, or leave it empty until **Run with agent**. |

The list shows step count, check coverage (share of do-steps followed by a check), last-run status, and source. A linked recording adds **Open in Runs**, **Replay**, and, after a failure, **Repair with agent**. **Run with agent** saves the case and opens Chat with a prompt that records it and links the new scenario back.

## Sets

A set is a named group of cases. Membership is one of:

- **Manual** — the cases you check.
- **By label** — any case that carries one of the labels you add. The member list updates as cases gain or lose those tags.

A set has a name and a description. A plan is what runs it.

## Plans

A plan is the scope you run: any number of sets, plus extra cases that are not in those sets. Members are the union, de-duplicated, in set order then direct-case order. A case with no recording is skipped.

**Run as** picks a persona, **on** picks an environment. **Connect** signs that persona in for the environment before the run. The headed toggle follows Settings unless you flip it for this run. **Auto-repair** opens a repair chat for each member that fails after the run settles. The page polls while members are in flight and shows step progress; open a row to land on that scenario in Runs.

A plan forwards only inputs the member scenario declares. The same scope from a terminal or CI is `agent-qa plan run <planId>` — flags and exit code are in [CLI verbs](verbs.md#plans).

When a member was imported from Xray, **Export to Xray** pushes the latest verdict back as a test execution.

## Runs

Runs is the home page. The sidebar lists scenarios. An empty root tells you to record one. Each row can show an `audit health` badge (flaky, slow, or chronic heal) and a trend chip of recent pass/fail.

Open a scenario to see its intent and steps. **Replay** runs it. Next to the button:

- **Replay as** — a persona. An auth-walled scenario preselects the persona whose profile matches the recording.
- **Environment** — base URL and params for this run.
- **Inputs** — one field per declared scenario input. A plan run uses the same rule: undeclared keys are not forwarded.
- **Headed** — visible browser for this run. The default comes from Settings.

A second Replay click stays disabled while that scenario's run is in flight. They share one browser session (`replay-<sid>`).

During the run, a live pane follows the browser. Afterward the step list shows pass, fail, skip, and heal (a locator correction or a value the page refused). Screenshots open in a lightbox. A recorded video gets a scrubber. **Compare** diffs this run's snapshots and screenshots against another run of the same scenario. **Insert check** adds a claim after a step in the saved scenario. A failed shot claim shows the red diff map with **Re-mint baseline** (one step) or **Accept shots** (every capture in the run). The shot-diff card and the structural diff are described in [Golden screenshots](visual-testing.md#in-the-workbench). **Ask agent** opens Chat with the failing step.

The list auto-refreshes. The pause control in the header stops that polling.

## Goldens

Goldens lists every scenario that has a screenshot (`shot`) or ARIA-snapshot (`domshot`) baseline, and names the store those files sync to. Each scenario section shows the intent. A baseline opens the same comparison Runs uses (before/after, blink, diff map, side-by-side). Minting happens from Runs (**Re-mint baseline**, **Accept shots**) or from `shot-accept` / `domshot-accept`. Geometry baselines (`layout-accept`) stay on the CLI. Where the files live — in the repo, a GitHub repo, or a Turso database — is [Settings](#settings) and the `[baselines]` table in [Configuration](configuration.md).

## Personas and environments

These are the same records `replay --persona` and `replay --environment` read.

A **persona** is a login: a display name, a profile (the session id, defaulting to a slug of the name), and credential rows. A value is a literal or a `scheme:ref` that a credentials plugin resolves (`vault:…`). You can clone a persona that an extension installed into an editable copy.

An **environment** is a target: a name, a base URL, key/value params, and optional auth (plugin name, login URL, auth config, credential refs). The same case can run against staging and production by changing the environment on the plan or the run.

File shapes are in [Personas and environments](configuration.md#personas-and-environments).

## Knowledge

Knowledge imports tests you already have. The import runs through the agent, which holds the credentials.

- **Jira** — one issue key becomes one case.
- **Xray** — a test plan, test set, story, or epic becomes cases plus a set.

The job lands each case as it finishes, so cancel keeps what already imported, then opens Sets or Cases. Other tools on the page (issue trackers, docs, designs) are marked coming soon.

## Extensions

An extension is a package from npm or git. Installing it wires its plugins, skills, personas, and environments into this workbench with no config editing. **Check for updates** covers installed packages and the agent-qa CLI itself.

The page also has:

- **Browser extension** — download the Chrome recorder zip. Unpacked install is in [Browser extension](extension.md).
- **Element resolution** — enable a bundled resolve plugin (Jev) with an API key. Chat, the Editor, and replay then use it. Details are in [Plugins](plugins.md#enable-in-the-workbench).
- **Plugin paths** — the discovered plugin list, plus a manual binary path when you are not installing a package.

## Settings

Preferences live in `<scenarios>/_config/settings.json`. Environment variables override the file.

| Setting | What it does |
| --- | --- |
| Agent backend | `auto`, `cursor`, `pi`, or `opencode`. Resolved once at startup, so a change applies the next time you start `agent-qa web`. `AGENT_QA_CHAT_BACKEND` locks the control. |
| Headed browser by default | New replays and persona connects open a visible window. The toggle on a plan or a run still wins. |
| Golden storage | `local` (in the scenario tree), `github`, or `turso`. Writes the `[baselines]` table in `agent-qa.toml` and can pull or push. |
| Paths | The scenarios root and the settings file, read-only. |

## Where the files live

Everything is under the scenarios root, next to `<sid>/scenario.json`.

| Record | Path |
| --- | --- |
| Case | `_cases/<id>/case.json` |
| Set | `_sets/<id>/set.json` |
| Plan | `_plans/<id>/plan.json` |
| Chat | `_chats/<id>.json` |
| Persona | `_personas/<id>/persona.json` |
| Environment | `_environments/<id>/environment.json` |
| Settings | `_config/settings.json` |

`_cases`, `_sets`, and `_plans` are metadata. Runs lists scenario directories; these records sit beside them. The recording a case links to is an ordinary scenario directory.
