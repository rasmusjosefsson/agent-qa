# AGENTS.md — agent-qa

Instructions for AI coding agents working in this repo.

## What this repo is

A Rust CLI for recording and replaying user scenarios against any web app via
CDP. Generic, plugin-based, vendor-neutral.

## Hard rules

1. **Generic and vendor-neutral.** No product-specific code or examples
   enter the core (`cli/src/`) or the embedded skill content (`skill-data/`).
   Vendor logic lives in plugins (separate repos); vendor-specific skill
   content (route catalogs, profile names, auth) arrives via `[skills]
   extra-dirs` in a downstream `agent-qa.toml`. This is currently a
   review-time convention, not an automated gate — before landing, grep
   `cli/src` + `skill-data` for vendor names (routes, product terms,
   in-house framework names) and replace them with neutral examples
   (`default-user`/`admin-user`/`viewer`, `example.com`, `/users`).
2. **Keep downstream details out of tracked history.** Do not put customer,
   product, repository, credential-provider, authentication, feature-flag,
   route, fixture, user, environment, or upstream-change details in tracked
   audit trails, handoffs, docs, commit messages, or PR text. Keep generic
   evidence in this repository. Keep downstream evidence and credentials in
   the downstream extension or local ignored artifacts.
3. **Plugins are the only extension point.** Don't bake in adapter
   implementations — only the protocol and the host live here. See
   `docs/plugins.md`.
4. **agent-browser is a hard dependency.** It is resolved by the Node
   launcher (`npm/agent-qa/bin/agent-qa.js`) via `require.resolve` and
   passed to the Rust binary as `AGENT_BROWSER_BIN`. The Rust binary
   never walks `node_modules`.

## What to do when starting work

1. Identify the modules your change touches.
2. Land code under the matching subdirectory of `cli/src/` (see
   `docs/architecture.md` for the layout).
3. Add focused unit tests + add/update the parity gate.
