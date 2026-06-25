# PHASE 1 — Chat tab (task checklist)

> **STATUS: ✅ Phases 1–3 COMPLETE (2026-06-25).** All three `/v2` tabs (Chat,
> Editor, Runs) reach functional parity with their classic vanilla twins. Web
> build green; **34** Vitest unit tests green (12 chat reducer + 10 editor
> compose + 12 runs rows); **95** backend tests green. Each tab verified live
> end-to-end. **Next: Phase 4 (cutover & cleanup)** — make `/v2` the default
> and retire the vanilla `{app,editor,chat}.js` + `styles.css`. See
> `tmp/react-rewrite-PHASE0-handoff.md` for the full per-phase summary and the
> agent-browser `viewer`-session test-harness note.
>
> ---
>
> **Phase 1 (Chat) details below.**

> **STATUS: ✅ COMPLETE (2026-06-25).** `/v2/chat` reaches functional parity with
> classic `/chat`, plus markdown rendering via streamdown. Verified: web build
> green, 12 reducer unit tests green (`npm --prefix web test`), 95 backend tests
> green, and live end-to-end (selected Haiku model, streamed a prompt, confirmed
> markdown `<ul>` render + collapsible thinking + rehydration + live browser
> pane). What landed vs. the original plan:
> - Reducer rewritten to an **ordered `items[]` model** (`chatReducer.ts`, pure +
>   unit-tested) that mirrors classic `onAgentEvent` DOM order, so live streaming
>   and rehydration interleave user/assistant/thinking/**tool cards** identically.
>   (The earlier scaffold kept tool cards in a side map that was never rendered —
>   fixed.)
> - Components: `components/{Markdown,Message,ToolCard,PromptInput}.tsx`,
>   `BrowserPane.tsx` (own EventSource for precise live-status parity), `ChatPage.tsx`.
> - streamdown for progressive markdown; shadcn `select/scroll-area/tooltip` added;
>   `@source` for streamdown registered in `index.css`.
> - Meta changes (model/thinking) use `patch_meta` (no thread reload → no clobber).
> - Vitest added (dev-only) + `test` script; test files excluded from `tsc -b`.
> - Dead code removed (`lib/sse.ts`, unused `createBrowserEventSource`).
>
> **Next: Phase 2 (Editor — canvas + input forwarding), then Phase 3 (Runs),
> Phase 4 (cutover).** See `tmp/react-frontend-rewrite-plan.md` §5.
> Remaining nice-to-haves (non-blocking): code-split the 548 kB chat chunk
> (streamdown/mermaid), add a React-component smoke test, wire `web test` into CI.

---

This file is the drop-in, actionable checklist you can hand to the next agent or use as a local TODO. It spells out the steps to complete Phase 1 (Chat tab) with explicit checkboxes and verification commands.

Prerequisites
- Phase 0 scaffold is already in place (web/ source + v2 build output committed).
- Backend/server unchanged; canonical JS behavior to mirror: `npm/agent-qa/lib/public/chat.js`.

Quick dev loop
- Start dev server (proxies /api to the real server):
  - cd npm/agent-qa/web
  - npm install
  - npm run dev
  - open http://localhost:5173/chat.html

One-time dependency installs
- [ ] cd npm/agent-qa/web
- [ ] npm install
- [ ] npm install streamdown
- [ ] npx shadcn@latest add select scroll-area dialog tooltip
- [ ] npx ai-elements@latest add  # add conversation, message, reasoning, prompt-input, suggestion, tool

Verify starter scaffolding (should already exist)
- [ ] web/src/lib/types.ts
- [ ] web/src/lib/api.ts
- [ ] web/src/lib/sse.ts
- [ ] web/src/features/chat/useChat.ts
- [ ] web/src/features/chat/ChatPage.tsx (basic renderer)
- [ ] web/src/features/chat/BrowserPane.tsx
- [ ] web/src/main-chat.tsx mounts <ChatPage /> into AppShell

Implementation checklist (do in order)

1) Components (replace minimal renderer with full components)
- [ ] Create directory: web/src/features/chat/components/
- [ ] Add Conversation.tsx (wrap ai-elements Conversation; container + scroll area)
- [ ] Add Message.tsx
  - [ ] Renders user, assistant, thinking (details), toolCall parts
  - [ ] Integrate streamdown for progressive markdown rendering of assistant text (streaming deltas)
- [ ] Add ToolCard.tsx (arguments collapsed, partial output while running, final result/error)
- [ ] Add PromptInput.tsx
  - [ ] Textarea input with auto-grow
  - [ ] Send (Enter) & newline (Shift+Enter)
  - [ ] Stop (abort) & New
  - [ ] Model Select (shadcn Select) wired to useChat.setModel
  - [ ] Thinking Select wired to useChat.setThinking
  - [ ] Suggestion chips (SUGGESTIONS from classic chat.js)
- [ ] Add Suggestions.tsx (reusable chips)

2) Use existing hook + reducer
- [ ] Review web/src/features/chat/useChat.ts reducer and state shape; confirm parity with lib/public/chat.js:onAgentEvent
- [ ] Ensure streamingMessage and toolCards lifecycle works: partial deltas update in place; finalize on message_end/agent_end/turn_end
- [ ] Reconcile state on EventSource reconnect and session events (session_reset/session_idle) by re-fetching GET /api/chat/state
- [ ] Add small adjustments if components require slightly different shapes (keep types.ts updated)

3) Wire UI
- [ ] Replace ChatPage renderer with Conversation + Message components + PromptInput
- [ ] Ensure scroll container id/class matches logic in useChat for auto-scroll (or copy nearBottom heuristics)
- [ ] Ensure streamdown renderer supports progressive append for streaming deltas
- [ ] Hook BrowserPane into the right-hand column; add liveStatus and liveHint similar to classic

4) Tool card behavior
- [ ] Ensure tool_execution_start creates a running card keyed by toolCallId
- [ ] tool_execution_update appends partial results (append, not replace)
- [ ] tool_execution_end finalizes status (ok or err) and renders final result or error

5) UX details
- [ ] Auto-scroll only when near bottom (same heuristic as classic)
- [ ] When streaming, show Stop button (Abort); disable/enable inputs to match classic availability state
- [ ] Enter sends when not shift; Shift+Enter inserts newline
- [ ] Model & thinking selectors reflect backend state and are disabled when agent unavailable

6) Tests
- [ ] Add unit tests for reducer fold logic (web/test/useChat.reducer.test.ts or with Vitest in web/)
  - Test scenarios to cover:
    - text_start -> text_delta* -> message_end => final assistant message in messages[]
    - thinking_delta then text stream => thinking block preserved + assistant text
    - tool_execution_start -> update -> end => toolCard status & out
- [ ] Run frontend test runner (if added): npm --prefix web run test
- [ ] Run repository tests: cd npm/agent-qa && node --test test/*.test.js (existing tests should remain green)

7) Build & commit
- [ ] Build web: cd npm/agent-qa && npm run build:web  (produces lib/public/v2/)
- [ ] Verify built pages: /v2/chat, /v2/editor, /v2/
- [ ] Commit built output (Decision 3A — distribution keeps built assets)
- [ ] Push branch and open PR

PR checklist (add to PR description)
- [ ] Types added/updated: web/src/lib/types.ts
- [ ] API and SSE helpers implemented: web/src/lib/api.ts, web/src/lib/sse.ts
- [ ] useChat hook + reducer + reducer tests
- [ ] Chat UI components (Conversation, Message, ToolCard, PromptInput, Suggestions)
- [ ] BrowserPane polish and nav wired
- [ ] streamdown integrated for progressive markdown rendering
- [ ] Built output committed: npm/agent-qa/lib/public/v2/
- [ ] node --test test/*.test.js passes locally (repo tests)
- [ ] Attach two screenshots to PR: classic /chat vs /v2/chat side-by-side and note any behavioral differences

Acceptance criteria (DoD)
- [ ] /v2/chat rehydrates history and renders user/assistant text, thinking blocks, tool cards and results identical to classic for representative cases
- [ ] SSE streaming deltas append in-place with no duplicates
- [ ] Send/Abort/New behave like classic
- [ ] Model selector & thinking selector wired and functional
- [ ] Suggestions send the prompt
- [ ] Browser pane displays frames and supports nav/url commands
- [ ] Assistant markdown rendered via streamdown progressively
- [ ] Unit tests for reducer pass; repo tests remain green

Notes & tips
- EventSource auto-reconnects. Re-fetch GET /api/chat/state on reconnect or on session_reset/session_idle events to avoid drift.
- streamdown supports incremental markdown streaming; feed deltas into the renderer and flush when the message finalizes.
- For parity, copy folding logic from npm/agent-qa/lib/public/chat.js:onAgentEvent into your reducer rather than inventing a different flow.
- Keep runtime deps in web/package.json (shadcn, @fontsource-variable/geist, tw-animate-css, ai-elements, streamdown).

Where to put the finished checklist
- If you want this checklist included in the repo for the next agent, I can move it to `npm/agent-qa/web/TASKS.md` (this is where I wrote it). If you prefer the root repo or tmp/, tell me and I will move it.

When done
- Update the checkboxes in this file and push. In the PR description paste a short summary and the final checklist state.

---

## Chat ↔ browser session binding ("chat + browser = session")  (2026-06-25)

Goal: each chat conversation OWNS a dedicated agent-browser session, so the
agent's browsing AND recording land in one browser the live pane mirrors —
instead of borrowing the shared global `default` session.

### What changed
- **Rust CLI** (`cli/src/start.rs`, `cli/src/run_step.rs`): session resolution
  now honors `AGENT_BROWSER_SESSION` (explicit `--session` > active record-env
  SESSION > `AGENT_BROWSER_SESSION` > `"default"`). So bare `agent-browser`
  AND `agent-qa start` land in the same session with no `--session` flag.
  Rebuild: `cd cli && cargo build --release`.
- **report-server.js**:
  - `makeChatBrowserBinding()` mints `chat-<hex>` and exports it on
    `process.env.AGENT_BROWSER_SESSION` (the in-process chat agent's bash
    inherits it). Editor/replay `childEnv` explicitly **strips** the var so they
    keep their own sessions (verified: editor `start` → `default`).
  - Pins `AGENT_QA_SCENARIOS_DIR`/`AGENT_QA_RECORD_DIR` on `process.env` so the
    chat agent records into the same roots the viewer/editor read → chat
    recordings show in Runs + the pane can follow the recorder session.
  - `GET /api/chat/active-session` → `{ session, recording }` (recorder SESSION
    if recording, else the bound chat session). `GET /api/chat/sessions` lists
    live sessions (reads `*.sock` in `AGENT_BROWSER_HOME`, drops dead pids).
  - `POST /api/chat/new` rotates to a fresh `chat-<hex>` and best-effort closes
    the old daemon (fresh browser per conversation).
- **BrowserPane.tsx**: follows `active-session` (auto), plus a session picker to
  pin any live session; badges `rec · live` while recording.

### Local binary note (IMPORTANT for production)
The Rust change must be built into the PUBLISHED platform binaries
(`@rasmusjosefsson/agent-qa-<os>-<arch>`). For local validation the rebuilt
`cli/target/release/agent-qa` was copied over the resolved platform binary
(backup at `…/bin/agent-qa-darwin-arm64.orig`). A normal `npm` reinstall would
revert it until the platform packages are rebuilt+republished.

### Verified
- `agent-qa start` (no flag) with `AGENT_BROWSER_SESSION` set → records into that
  session; active-session → `{recording:true, session:chat-…}`; browse+record
  share one browser. Editor still uses `default`. Backend 95/95, web 34/34.

### Known follow-ups (polish, non-blocking)
- Streaming a not-yet-navigated bound session spawns an idle about:blank daemon
  via `cdp-url` (same trait as the replay viewer). Consider deferring the stream
  until the session has a real URL, and a sweep to close idle `chat-*` daemons.
- Multi-chat: the hub is single-per-server; concurrent chats need a hub map
  keyed by chat id (each with its own bound session).

---

## Phase 4 — Cutover & cleanup  ✅ DONE (2026-06-25)

The React app is now the canonical UI. `/v2` is retired.

- **vite.config.ts**: `base:'/'`, `outDir:'../lib/public'`, `emptyOutDir:false`
  (vanilla files removed manually so artifacts aren't wiped).
- **Deleted vanilla files**: `lib/public/{app,editor,chat}.js`, `chat.html`,
  `styles.css`, and `lib/public/v2/`. (Full backup tarball at
  `tmp/phase4-backup/` — note `chat.js`/`chat.html` were untracked.)
- **Built** React into `lib/public/{index,editor,chat}.html` + `lib/public/assets/*`.
- **report-server.js**: `STATIC_FILES` trimmed to page routes only; added
  `serveAssets()` for `/assets/*` (segment-clamped); removed `V2_PAGES`/`serveV2`
  and the `/v2` dispatch. Titles dropped the `(v2)` suffix.
- **AppShell.tsx**: tab links → `/`, `/editor`, `/chat`; removed the "Classic UI"
  link; branding is just "agent-qa".
- **Tests**: updated `report-server.test.js` (`/v2`→`/assets`, index title) and
  `editor.test.js` (`editor.js`→hashed `/assets/*.js`). Backend **95/95**,
  web **34/34**, build green. Live verified: `/`,`/editor`,`/chat` → 200;
  `/v2/*` + vanilla `*.js` → 404; hashed assets resolve.

### Follow-up (optional, §6 distribution — not done)
- Consider a `prepack` script (`npm run build:web`) + a CI check that
  `git diff --exit-code lib/public` is clean after a fresh build, so shipped
  bundles can't go stale. Deferred to avoid release-path surprises now.

---

## Multi-chat hub  ✅ DONE (2026-06-25)

Multiple concurrent chats, each with its OWN conversation AND its own
agent-browser session (browsing + recording isolated per chat).

### Mechanism (the key insight)
The in-process chat agent's bash inherits `process.env`, so a single global
`AGENT_BROWSER_SESSION` could not isolate concurrent chats. Instead each chat's
hub builds a custom bash tool via the pi SDK's `createBashToolDefinition(cwd,
{ spawnHook })` (with `noTools:'builtin'` + `customTools` = read/bash/edit/write)
and the `spawnHook` injects `{ AGENT_BROWSER_SESSION: chat-<id>,
AGENT_QA_RECORD_DIR: <recordRoot>/chat-<id> }` into **every** bash spawn. The
Rust CLI honoring `AGENT_BROWSER_SESSION` (see start.rs/run_step.rs) then lands
both browsing and recording in that chat's session. Verified: two chats prompted
concurrently each echoed their own `chat-<id>`.

### Backend (report-server.js + chat-agent.mjs)
- `chat-agent.mjs makeSessionFactory`: honors `config.bashEnv` (a getter) to
  build the session-injecting bash tool. Falls back to default tools otherwise.
- `createChatManager` is now a **map** of chat entries. Each entry: id, own
  `makeChatBrowserBinding()`, lazy per-chat hub (built with a per-chat `bashEnv`),
  `recordDir()`. `primary()` auto-creates for the legacy flat routes.
- Routes: `GET /api/chat/list`, `POST /api/chat/create`,
  `POST /api/chat/c/<id>/delete`, and per-chat `c/<id>/{state,stream,prompt,
  abort,new,model,thinking,active-session}`. Legacy flat `/api/chat/<sub>`
  routes still work against the primary chat (tests + single-chat clients).
- `active-session` is per-chat: reports `recording:true` only when the active
  recording's session matches this chat's own session.

### Frontend (web/src/features/chat + lib/api.ts)
- `lib/api.ts`: `listChats`/`createChat`/`deleteChat` + all per-chat calls take a
  `cid` and hit `/api/chat/c/<cid>/*`.
- `useChat(cid)`: keyed on cid; re-hydrates state + reopens the SSE stream when
  cid changes.
- `ChatPage`: owns the chats list + active id, renders the switcher (pills with
  ×, "+ New chat"), and mounts `<ChatConversation key={activeId} cid=… />` so a
  switch remounts and re-hydrates. `BrowserPane` takes `chatId` and follows that
  chat's `active-session`.

### Tests / verification
- Backend `chat-agent.test.js`: added multi-chat (list/create/per-chat/delete) +
  primary-chat tests. Backend **97/97**, web **34/34**, build green.
- Live: created Chat 2 in the UI (distinct session); concurrent prompts to two
  chats each injected their own session; per-chat pane follows its own session.

### Known follow-ups
- Chat titles are positional ("Chat N"); could derive from the first prompt.
- Streaming a not-yet-navigated session still spawns an idle about:blank daemon
  (pre-existing live-bridge trait).

---

## Local CLI build / "survives reinstalls"  ✅ DONE (2026-06-25)

The Rust session-precedence change (start.rs/run_step.rs) is built into the
**published** platform binaries automatically: `.github/workflows/release.yml`
runs `cargo build --release` from `cli/` per target and stages the binary into
`@rasmusjosefsson/agent-qa-<os>-<arch>`. Committed Rust source ships on the next
release — no extra wiring.

For **local dev** (the platform packages are versioned 0.0.0 / unpublished, so
`npm install` can't fetch a real binary and a clean `npm ci` drops the dep):
- `npm --prefix npm/agent-qa run build:cli` → `scripts/build-local-cli.js`.
  Rebuilds the release binary and **self-heals** the local platform package:
  recreates `node_modules/@rasmusjosefsson/agent-qa-<key>/{package.json,bin/}`
  with the bin symlinked → `cli/target/release/agent-qa`, so `cargo build
  --release` keeps it fresh and `agent-qa report view` resolves it with no env
  var. Verified: deleting the whole package dir then re-running build:cli
  restores resolution + the binary honors `AGENT_BROWSER_SESSION`.
- node_modules-independent fallback (survives ANY reinstall):
  `export AGENT_QA_BINARY_PATH="$PWD/cli/target/release/agent-qa"` (checked first
  by the launcher).

---

## Chat view: resizable split + live Recording tab  ✅ DONE (2026-06-25)

Two always-on chat enhancements (the conversation|browser layout now has a
draggable split and the recording steps stack under the live browser):

### Update (2026-06-25, later): tabs → stacked, and saved recordings persist
- **Steps under the browser, no tabs.** The live browser canvas is
  `object-contain` (letterboxed), so it never fills the column. Dropped the
  Live|Recording tabs; the right column is now a vertical stack — browser on top
  (`flex-[3]`), the recording steps panel underneath (`flex-[2]`), shown only
  once a recording exists. Removed `rightTab`/`RTAB_KEY`/`TabButton`.
- **Recording no longer vanishes when the chat finishes.** On finish the CLI
  truncates the per-chat `scenario.env` but leaves `scenario.last` → the saved
  SID. `chatRecordingState` + `serveRecordingArtifact` now resolve the SID via
  `resolveChatRecordingSid()` (live `scenario.env.SID`, else `scenario.last`),
  and hydrate finished recordings from the saved `scenario.json`
  (`normalizeFinalizedStep()` maps `do/goto`→navigation, `check`→assert, etc.
  onto the live RecordingStep shape). Shows as **✓ saved** with working
  keyframes. Test extended to cover the finished-recording fallback.
- **Resizable split** — a draggable divider between the conversation and the
  right pane (desktop only; pointer-events based, clamped 25–80%, persisted to
  `aqa-chat-split`). Stacks vertically on narrow screens.
- **Right pane tabs: Live | Recording** — "Live" is the existing browser pane;
  "Recording" shows the chat's current recording as it's captured: header
  (intent · sid · session) + a ● recording / ✓ saved badge, a step list (kind
  badge, summary, timestamp), each step expandable to its keyframe screenshot.
  The tab shows a live badge with the step count.

### Backend (report-server.js)
- `GET /api/chat/c/<id>/recording` → `{recording, sid, intent, session,
  startedAt, baseline, flushed, steps[]}`, read straight off the per-chat record
  scratch (`scenario.env` + `scenario.steps.jsonl`); `flushed` = the scenario dir
  has a `scenario.json`. `recording = !flushed`. No CLI spawn (pollable).
- `GET /api/chat/c/<id>/recording/step/<stepId>/{screenshot,snapshot}` → serves
  the per-step keyframe from `<scenariosRoot>/<sid>/recording/{screenshots,
  snapshots}/`, path-clamped. `handleChat` now takes `scenariosRoot`.
- Helpers `chatRecordingState()` + `serveRecordingArtifact()`. Test added
  (`chat-agent.test.js`): empty → populated → artifact serve → 404 → traversal
  guard → flushed. Backend **98/98**.

### Frontend (web/src)
- `lib/useMediaQuery.ts`,
  `lib/api.ts` (`getRecording`, `recordingArtifactUrl`, types),
  `features/chat/components/RecordingView.tsx`, and `ChatPage.tsx`
  (shared `conversationColumn` + `liveBrowserPane`).
  web **34/34**, build green. Live-verified: flag off = original layout; flag on
  draggable resizer (persisted); Recording tab shows steps + screenshot.
