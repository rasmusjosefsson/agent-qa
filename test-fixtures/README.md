# test-fixtures

## CI browser fixture

`scripts/ci-browser-contract.sh` serves `ci-browser/index.html` on local HTTP.
The fixture has one `Complete` button. The button reveals the accessible
`Completed` status. The CI contract records the interaction, replays it twice,
and compares the replay screenshots with no pixel tolerance.
