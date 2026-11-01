# Replay

Replay runs a flushed scenario again from the Runs pane and shows the new run's pass state. The run writes a replay directory next to the scenario.

## Sub-features

- `replay-start` starts a replay from the `Replay` button on the scenario preview.
- `replay-pass` finishes with a passing run and a new directory under `replays/`.
- `replay-cli` runs the same scenario from the terminal with `aq.sh replay`.

## How to get to it (user POV)

- Open Runs, choose the scenario, and choose `Replay` on the scenario preview.
- In a terminal, run `agent-qa replay <sid>`. This map uses `scripts/aq.sh replay`.

A case detail page also shows `Replay` after a scenario is linked. That entry is not mapped; do not count it as this recipe.

## Driving it with verify-agent-qa

Preconditions:

- `doctor.sh` printed `ok` for this run.
- [Record a scenario](./record-scenario.md) has flushed a scenario and the fixture server from `fixture.sh start` is still serving `fixture.sh url`.
- The Runs center pane is the scenario preview (no run selected yet), so the button says `Replay` and not `Replaying…`.

- **Open the preview.** Run `scripts/browser.sh open "$VERIFY_URL/?sid=$(tr -d '[:space:]' <"$VERIFY_RECORD/scenario.last")"` and `scripts/browser.sh set viewport 1280 800`. The heading is `open the contract fixture and complete it` and a button named `Replay` is visible. The mono sid under the heading matches `scenario.last`.
- **Replay.** Choose `Replay`. Run `scripts/browser.sh find role button click --name "Replay"`. The button label becomes `Replaying…`.
- **Wait for the run.** Poll until `$VERIFY_ROOT/<sid>/replays/latest.txt` exists and `scripts/browser.sh snapshot` no longer shows `Replaying…`. The snapshot shows `pass`. Do not start a second replay while the button says `Replaying…`.
- **Confirm the directory.** Read `latest.txt`. `$VERIFY_ROOT/<sid>/replays/<run-id>/screenshots/s0.png`, `s1.png`, and `s2.png` are non-empty.
- **CLI entry.** Run `scripts/aq.sh replay "$(tr -d '[:space:]' <"$VERIFY_RECORD/scenario.last")" --session "$VERIFY_RECORD_SESSION"`. Exit code is 0 and `latest.txt` changes to a new run id.
- **Proof.** Save the snapshot and `scripts/browser.sh screenshot "$VERIFY_EVIDENCE/replay/run.png"` while the run is visible. Copy `latest.txt` into `$VERIFY_EVIDENCE/replay/`. `note.txt` says `replay-pass` and names the button entry, then the CLI entry.

## Gotchas

- Replay opens the URL stored in the scenario. If `fixture.sh` was stopped, the run fails. A failed run is not a pass proof.
- The workbench uses one browser session per scenario (`replay-<sid>`). A second click while the first replay is in flight collides with that session.
- The button stays `Replaying…` until the run is observed or about 45 seconds pass. Wait on the label and on `latest.txt`, not on a fixed sleep.
- `aq.sh replay` without the record from this run replays a different root. The sid has to be the one in `$VERIFY_RECORD/scenario.last`.
