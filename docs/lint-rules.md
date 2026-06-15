# Scenario lint rules

13 rules ship out of the box. Run `agent-qa scenario lint --list-rules`
for the live table (this doc may lag the binary).

| Code | Severity | What it catches |
| --- | --- | --- |
| `duplicate-step-id` | error | Two or more steps share the same id. The runner uses the id to address steps; duplicates would cause silent skips. |
| `empty-intent` | warning | A step's intent is whitespace-only. Intent is the human-readable "why does this exist" for a step; an empty one makes triage harder. |
| `bare-do` | warning | A do step is not followed by a check claim (trailing or pre-do). The replay can succeed without verifying the post-condition. |
| `undeclared-input` | error | A value references `{ from: 'input', input: '<name>' }` where `<name>` isn't in `scenario.inputs`. The runner would fail at resolve time. |
| `unused-input` | warning | A declared input is never referenced anywhere in the scenario. |
| `undeclared-step-ref` | error | A value references `{ from: 'step', stepId: '<id>' }` where `<id>` doesn't exist in this scenario. |
| `goto-without-url` | error | A do/goto step has no `params.url`. The runner would fail at dispatch. |
| `missing-locator` | error | A do step that needs a locator (`click`, `type`, `clear`, `hover`, `focus`, `blur`, `check`, `uncheck`) has no `params.locator`. |
| `params-on-noop` | warning | A do step whose verb ignores params (`reload`, `back`, `forward`) has a non-empty params object. |
| `no-env-open` | warning | The scenario has no `env.open[]` entries; replay will start on a blank tab. |
| `no-checks` | warning | The scenario has steps but zero check claims; replay can only fail on browser errors, not assertions. |
| `empty-steps` | warning | The scenario has zero steps; replay will only open env then close it. |
| `wait-without-condition` | warning | A do/wait step has neither `params.timeoutMs` nor `params.locator`; will hang the replay until the global timeout fires. |

## Severity → exit code

- Default: any **error** finding makes the verb exit 1. Warnings
  inform but don't gate.
- `--strict`: treat warnings as errors too. Exit code reflects
  errors + warnings.

## Filtering

- `--rule <code>` (repeatable) narrows to specific codes.
- `--exclude-rule <code>` (repeatable) subtracts codes from the
  active set.
- `--rule` and `--exclude-rule` combine as `(only.contains || only.none) && !excluded.contains`.

## Output formats

`--format text|json|github`:

- `text` (default): human-readable findings list + count summary.
- `json`: `{id, errors, warnings, findings: [{severity, code, message}]}`.
- `github`: one `::error file=...,title=lint/<code>::<message>` or
  `::warning ...` workflow command per finding. Drop-in for GitHub
  Actions PR annotations. See
  [`examples/github-actions-scenarios.yml`](../examples/github-actions-scenarios.yml).

## Live source of truth

Every rule listed here is also in `cli/src/scenario_cli.rs`'s
`list_lint_rules()` table. If the two ever diverge, the code wins —
file an issue against this doc.
