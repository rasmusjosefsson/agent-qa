# AGENTS.md — agent-qa

Instructions for AI coding agents working in this repo.

## What this repo is

A Rust CLI for recording and replaying user scenarios. The web surface drives
a web app through agent-browser and CDP. Desktop, iOS, and Android replay
through a surface provider and do not start the browser. Generic,
plugin-based, vendor-neutral.

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
3. **Two extension points, both out of process.** App-specific behavior
   (auth, session policy, hooks) is a plugin. A non-web surface (desktop,
   iOS, Android) is a surface provider. The host and the protocol live
   here; the driver binary does not. Do not bake either adapter into the
   browser runner. See `docs/plugins.md` and `docs/surfaces.md`.
4. **agent-browser is a hard dependency.** It is resolved by the Node
   launcher (`npm/agent-qa/bin/agent-qa.js`) via `require.resolve` and
   passed to the Rust binary as `AGENT_BROWSER_BIN`. The Rust binary
   never walks `node_modules`.

## What to do when starting work

1. Identify the modules your change touches.
2. Land code under the matching subdirectory of `cli/src/` (see
   `docs/architecture.md` for the layout).
3. Add focused unit tests + add/update the parity gate.
4. If a user or an agent would notice the change, update the docs in the
   same change. See below.

## Docs move with the change

`docs/` is the source the site publishes. A new markdown file stays off
the site until the site lists it. Do this before landing, not as a
follow-up.

- Edit the page that already documents the area (`docs/configuration.md`,
  `docs/architecture.md`, `docs/verbs.md`, and the rest). Link it from
  the page a reader would open instead, so the two are not confused.
- A new page also needs a row in `site/scripts/sync-docs.mjs`, a sidebar
  entry in `site/astro.config.mjs`, and a line in `site/public/llms.txt`.
- Update `site/src/pages/index.astro` when the product boundary changes,
  and `site/public/.well-known/agent.json` when the capability list
  changes. The homepage still describes the browser path as the default.
- When an agent must record or replay differently, update `skill-data/`
  in the same change. `skills/agent-qa/SKILL.md` stays a loader. Command
  syntax lives in `skill-data`, served by `agent-qa skills get`.
- Adding or removing a published page, or editing `site/`, rewrites the
  docs golden scenario. From `evals/`, run `bun evals/docsite/gen.ts` and
  commit `evals/docsite/scenarios/docs-pages/scenario.json`. Screenshot
  baselines are minted in CI with `/docs-goldens accept`, not on a dev
  machine. Say that in the PR.
- Confirm with `npm --prefix site run build`. Do not commit
  `site/src/content/docs/docs/` or `site/public/docs/`. Those are
  generated.

## Cursor Cloud specific instructions

- `node` on `PATH` is Node 24 (`agent-browser` requires `>=24`; CI uses Node 24). `/usr/local/cargo/bin` is ahead of `/exec-daemon/node`, which is still Node 22. Rust is the stable toolchain via `rustup` (edition 2024). Bun is `/usr/local/bin/bun`.
- Checks match `.github/workflows/ci.yml`. In `cli/`: `cargo test --locked`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo fmt --check`. In `npm/agent-qa`: `node --test test/*.test.js`. In `npm/agent-qa/web`: `npm run typecheck` and `npx vitest run` (run them from that directory). In `evals/`: `bun test golden/`.
- `cargo test` does not launch Chrome. Record/replay proof is `scripts/ci-browser-contract.sh` with `AQ` set to a launcher whose sibling is `agent-browser`. `agent-browser install` caches Chrome for Testing under `~/.agent-browser`.
- The workbench is `agent-qa web` at `http://127.0.0.1:7878`. Cloud Agent boot already starts it (`--no-open`). `npm --prefix npm/agent-qa run dev` rebuilds the debug CLI and binds the same port, so stop the boot server first if 7878 is taken. The UI bundle is gitignored; `npm --prefix npm/agent-qa run build:web` writes `npm/agent-qa/lib/public/`. Default scenarios root is `tmp/agent-qa-scenarios`.
- Workbench and record/replay checks use `.agents/skills/verify-agent-qa`. Its scripts launch an isolated `agent-qa web` on the first free port from 7879, drive that instance, and clean up only what that run started.
