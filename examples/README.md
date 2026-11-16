# agent-qa examples

Copy-paste templates and reference setups.

| Path | What it is |
| --- | --- |
| [`agent-qa.toml`](./agent-qa.toml) | Reference config showing every supported `[plugins]` + `[paths]` key. Drop into a repo root and tweak. |
| [`scenarios/`](./scenarios/) | A scenarios root: `smoke`, `a11y`, and `emulate`, plus `_cases`, `_sets/smoke`, and `_plans/release-check`. `AGENT_QA_SCENARIOS_DIR=examples/scenarios agent-qa plan list` reads the plan. `agent-qa web --root examples/scenarios` opens it in the workbench. |
| [`scenarios/smoke/`](./scenarios/smoke/) | Minimal valid `scenario.json` with one do step + one check claim, plus its README. The canonical "does the toolchain work?" smoke scenario. |
| [`plugins/noop-auth/`](./plugins/noop-auth/) | Reference plugin binary (POSIX shell) that implements the `auth` kind via the JSON-over-stdio protocol. Useful as a starting point for vendor adapters. |
| [`github-actions-scenarios.yml`](./github-actions-scenarios.yml) | CI workflow: validate + lint every scenario on every PR, surface findings as inline annotations via `--format github`. |
| [`pre-commit.sh`](./pre-commit.sh) | Git pre-commit hook that runs `scenario check` against staged `scenario.json` files. |

## Conventions

- Examples are runnable. If something here doesn't work, that's a
  bug — file an issue.
- Replace the placeholder `https://example.com/` URLs with real
  targets before running anything against a live site.
- The CI workflow assumes `npm install -g @agent-qa/cli` works in
  the runner. Adjust if you're vendoring the binary or installing
  via cargo.
