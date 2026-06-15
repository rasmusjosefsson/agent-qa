# Contributing to agent-qa

Glad you're here. agent-qa is a Rust CLI plus an npm distribution
shell; almost all interesting code lives under `cli/src/`.

## Quick start

```bash
git clone https://github.com/rasmusjosefsson/agent-qa
cd agent-qa/cli
cargo build
cargo test
```

The `cli/` crate is the whole binary — there is no multi-crate
workspace. Tests run against fake `agent-browser` shell scripts (see
`cli/src/test_util.rs` for the shared env-lock plumbing); no real
Chromium is spawned during `cargo test`.

## Adding a verb

Each verb is one module under `cli/src/<verb_name>.rs` exporting
`pub fn run(args: &[String]) -> anyhow::Result<u8>`. Registration is
two lines in `cli/src/main.rs`:

1. `mod <verb_name>;` near the other module declarations
2. one match arm in the dispatcher: `"<verb-name>" => verb_name::run(rest),`

Update the help text in `print_help()` and the README verb table.
Run `cargo test` — if the test surface is too thin, add focused unit
tests next to the verb (each existing verb file has an inline
`#[cfg(test)]` mod with at least the parse_args + happy-path cases).

## Conventions

- **Plain serde with explicit `rename_all = "camelCase"`** when on-disk
  shape matters (audit.json, profile.json, heal-response.json, etc).
  Matches the scenario/2 schema and JS-friendly tooling.
- **Atomic writes** for any sidecar that another tool might read live:
  `crate::sidecar::atomic_write_file`. Don't reach for `fs::write` for
  artefacts under `<sid>/`.
- **Safe path segments**. Anything that ends up in a path (sid,
  stepId, runId, archive-tag, etc.) goes through the safe-segment
  regex `[A-Za-z0-9._-]+` (and rejects `.` / `..`). See `paths.rs`
  and `sidecar.rs`.
- **Vendor-neutral**. Zero references to specific products in source,
  schema, skill-data, or fixtures. CI enforces this with a
  word-bounded grep against a configurable banned-words list.
- **Tests speak in real subprocesses** for browser-touching verbs:
  write a `#!/bin/sh` fake under a `tempfile::TempDir`, set
  `AGENT_BROWSER_BIN` to it, assert on the captured invocation log.

## Plugin protocol

Plugins are subprocesses that speak JSON over stdio. The protocol
contract lives in [`docs/plugins.md`](docs/plugins.md). The reference
plugin (10 lines of POSIX shell) is at
[`examples/plugins/noop-auth/`](examples/plugins/noop-auth/).

## Architecture and plan

- [`docs/architecture.md`](docs/architecture.md) — what fits where, on
  disk + in code.

## Releases

Tag `v0.0.x` on `main` to trigger
[`.github/workflows/release.yml`](.github/workflows/release.yml).
Cross-compile + per-platform npm publish + umbrella publish are all
in CI; you only need an `NPM_TOKEN` secret with publish access. Full
walkthrough in [`docs/releasing.md`](docs/releasing.md).

## Code of conduct

Be kind. Don't ship vendor-specific code into the core. The CI gate
will help with the second part.
