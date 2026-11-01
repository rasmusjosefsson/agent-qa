# Record a scenario

Record a scenario drives the local browser-contract fixture with the CLI, flushes `scenario.json`, and shows that scenario in the workbench Runs pane.

## Sub-features

- `record-fixture` serves `test-fixtures/ci-browser` and opens it in the record session.
- `record-steps` records goto, the `Complete` click, and the `Completed` status check.
- `record-flush` writes `scenario.json` and `scenario.last`.
- `record-visible` shows the new intent in Runs.

## How to get to it (user POV)

- In a terminal, run `agent-qa start`, drive the page, `agent-qa record-step`, then `agent-qa flush`. The workbench is not required to record. This map runs those commands through `scripts/aq.sh` so they hit the isolated root.
- After flush, open Runs in the workbench. The new scenario is in the sidebar.

The Editor tab can also start a recording. That entry is not mapped; do not substitute it for this recipe.

## Driving it with verify-agent-qa

Preconditions:

- `doctor.sh` printed `ok` for this run.
- `$VERIFY_ROOT` has no scenario yet, so the new sidebar row is unambiguous.
- `scripts/browser.sh` is not aimed at the record session. Gestures use `VERIFY_RECORD_SESSION`.

- **Serve the fixture.** Run `scripts/fixture.sh start`. Stdout is `http://127.0.0.1:<port>/`. `scripts/fixture.sh url` prints the same URL. The page title is `Browser contract fixture` and the button is `Complete`.
- **Start.** Run `scripts/aq.sh start "open the contract fixture and complete it" --session "$VERIFY_RECORD_SESSION"`. Stdout contains `started sid=` and exit code is 0.
- **Open the page.** Run `scripts/aq.sh browser --session "$VERIFY_RECORD_SESSION" open "$(scripts/fixture.sh url)"`. Then record the navigation: `scripts/aq.sh record-step do "{\"intent\":\"open the page\",\"verb\":\"goto\",\"value\":{\"from\":\"literal\",\"literal\":\"$(scripts/fixture.sh url)\"}}"`. Exit code is 0.
- **Complete.** Click the button, then record it. Run `scripts/aq.sh browser --session "$VERIFY_RECORD_SESSION" find role button click --name Complete` and `scripts/aq.sh record-step do '{"intent":"complete the page","verb":"click","on":{"role":"button","name":"Complete"}}'`.
- **Check.** Run `scripts/aq.sh record-step check '{"intent":"completion status is visible","claim":{"subject":{"element":{"role":"status","name":"Completed"}},"predicate":"isVisible"}}'`. Exit code is 0.
- **Flush.** Run `scripts/aq.sh flush`. Stdout contains `flushed sid=` and `wrote`. `$VERIFY_RECORD/scenario.last` is the sid. `$VERIFY_ROOT/<sid>/scenario.json` exists, and `$VERIFY_ROOT/<sid>/recording/screenshots/s0.png`, `s1.png`, and `s2.png` are non-empty.
- **See it in Runs.** Run `scripts/browser.sh open "$VERIFY_URL/?sid=$(tr -d '[:space:]' <"$VERIFY_RECORD/scenario.last")"` and `scripts/browser.sh set viewport 1280 800`. The center heading reads `open the contract fixture and complete it`.
- **Proof.** Save flush stdout, copy `scenario.json` and one recording screenshot into `$VERIFY_EVIDENCE/record-scenario/`, and save `scripts/browser.sh snapshot` plus `scripts/browser.sh screenshot` there. `note.txt` says `record-visible`.

## Gotchas

- Record the step after the gesture. Recording the click before the click stores the wrong page.
- `browser.sh` always uses the workbench session. A click issued through it does not land on the fixture.
- The fixture port is chosen by the OS. Read it from `fixture.sh url`. Do not hard-code a port.
- `fixture.sh start` records its pid in `$VERIFY_STATE/fixture.pid`. `cleanup.sh` kills that pid. Stopping it before replay makes the replay fail.
- Flush without `AGENT_QA_SCENARIOS_DIR` from `aq.sh` writes the repo `tmp/agent-qa-scenarios` directory. That run is not this workbench.
