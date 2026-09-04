# Test fixtures

The scenario corpus is empty. CI records a fresh scenario against a neutral
local fixture. Each fixture has this shape.

    test-fixtures/scenario/<name>/
    ├── scenario.json
    ├── snapshots/   screenshots/   network/   probes/
    └── replays/<runId>/...

See `docs/specs/scenario-sidecar-tree.md` for the on-disk contract.

## CI browser fixture

`scripts/ci-browser-contract.sh` serves `ci-browser/index.html` on local HTTP.
The fixture has one `Complete` button. The button reveals the accessible
`Completed` status. The CI contract records the interaction, replays it twice,
and compares the replay screenshots with no pixel tolerance.
