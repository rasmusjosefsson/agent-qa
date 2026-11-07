# GitHub Action

This repository doubles as a composite GitHub Action. In an app repo that
keeps agent-qa scenarios under `scenarios/`:

```yaml
# .github/workflows/qa.yml
name: qa
on: pull_request
permissions:
  contents: read
jobs:
  replay:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rasmusjosefsson/agent-qa@main
        with:
          scenarios: scenarios        # dir containing one subdir per scenario
          args: --retry 2 --junit     # anything `replay --all` accepts
```

Each PR then replays the whole scenario suite headlessly, posts a sticky
comment with the verdict and a timing table, and uploads
`<scenario>/replays/` as the `agent-qa-evidence` artifact — events, audit,
screenshots, snapshots, heal trail, network/console logs, and a
per-run `report.html` you can open straight from the artifact. The comment
needs `pull-requests: write` on the job.

## Inputs

| input | default | effect |
| --- | --- | --- |
| `scenarios` | `scenarios` | Becomes `AGENT_QA_SCENARIOS_DIR`. |
| `args` | `""` | Appended to `agent-qa replay --all` — `--shard 1/3`, `--filter`, `--tags`, `--retry`, `--junit`, `--offline`, `--mock-from`, etc. |
| `reports` | `"true"` | Renders `run-report` per scenario into each run dir before upload. Set `"false"` on a pinned ref that predates `run-report`. |
| `comment` | `"true"` | Post or update the sticky `<!-- agent-qa -->` comment. `"false"` skips it. No-op outside a pull request. |
| `timings` | `"report"` | Append a duration table to that comment. `"strict"` also fails the job when a row is slow. `"off"` skips the check. |
| `timings-baseline` | `""` | Workspace-relative `agent-qa-timings/v1` file. Empty uses `<scenarios>/_timings/baseline.json`. |
| `artifact-name` | `agent-qa-evidence` | Uploaded artifact name. |
| `retention-days` | `"30"` | Artifact retention. |

## Timing

`timings check` compares each scenario's latest passing run, and each step
that took at least 500ms, to the baseline. A row is slow when it is more
than twice the baseline and at least 5s slower, or when it crosses that
entry's `budgetMs`. A 10s case that starts taking 2min is slow. A case that
moves by a couple of seconds is not. Failed runs are left out.

With no baseline file the comment says so and the job stays green. Record
one from a passing suite and commit it:

```bash
agent-qa timings accept
```

`timings: strict` is the same table, and the job fails when anything is
slow. Details and the file shape are in [verbs](verbs.md).

## Growing coverage from the same loop

The point of the gate isn't just regression — it's *adoption*:

1. A feature PR lands → the gate replays existing scenarios (catch drift).
2. `agent-qa crawl <preview-url>` drafts a shot-claim skeleton for the new
   routes; commit it under `scenarios/` and the next PR replays it.
3. Locator churn is self-healed inline; `heal-chronic --all` surfaces the
   chronic debt when it's time to promote permanently.

## Previews / non-localhost targets

The action replays against wherever your scenarios point. To retarget a
deploy preview instead of the recorded origin, pass the flag that rewrites
navs/gotos (requires `--base-url` support):

```yaml
args: --base-url ${{ needs.deploy.outputs.preview_url }}
```
