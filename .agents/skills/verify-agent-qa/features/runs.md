# Runs

Runs is the workbench home. It lists scenarios in the left sidebar and, once one is chosen, shows that scenario's intent and steps. An empty root tells the user to record a scenario.

## Sub-features

- `runs-empty` shows the empty state when the scenarios root has no scenario.
- `runs-open` opens one scenario from the sidebar or from `/?sid=<sid>` and shows its intent.

## How to get to it (user POV)

- Open the workbench URL. The first page is Runs.
- Choose `Runs` in the sidebar.
- Open `/?sid=<sid>` after a scenario has been flushed. The center pane selects that scenario without an extra click.

## Driving it with verify-agent-qa

Preconditions:

- `doctor.sh` printed `ok` for this run.
- For `runs-empty`, `$VERIFY_ROOT` contains no scenario directory.
- For `runs-open`, [Record a scenario](./record-scenario.md) has flushed one scenario. `$VERIFY_RECORD/scenario.last` is that sid. Do not invent a scenario directory by hand.

- **Empty home.** Open the workbench. Run `scripts/browser.sh open "$VERIFY_URL"` and `scripts/browser.sh set viewport 1280 800`. The sidebar shows `agent-qa` and `Scenarios`, and the center shows `Record your first scenario`. The sidebar list shows `No scenarios yet`.
- **Sidebar entry.** From another page, choose `Runs`. Run `scripts/browser.sh open "$VERIFY_URL/cases"` then `scripts/browser.sh find role link click --name "Runs"`. The center shows the empty state or the previously opened scenario, and the address is `/`.
- **Open a flushed scenario.** After record, open the deep link. Run `scripts/browser.sh open "$VERIFY_URL/?sid=$(tr -d '[:space:]' <"$VERIFY_RECORD/scenario.last")"`. The center heading is `open the contract fixture and complete it` and the step list includes `open the page` and `complete the page`.
- **Proof.** Run `scripts/browser.sh snapshot >"$VERIFY_EVIDENCE/runs/page.aria.txt"` and `scripts/browser.sh screenshot "$VERIFY_EVIDENCE/runs/page.png"`. The artifacts show `agent-qa` and either `Record your first scenario` or the flushed intent. `snapshot -i` omits that empty-state text. Write `runs-empty` or `runs-open` into `$VERIFY_EVIDENCE/runs/note.txt`.

## Gotchas

- `/?sid=` selects a scenario only after the sidebar list has loaded that sid. A missing directory leaves the empty state; that is a failed open, not an empty-root proof.
- The scenario button's accessible name is the intent, not the sid. The sid is the smaller mono line under the intent.
- `Refresh` (the sidebar button named `Refresh`) reloads the list. A flush from another terminal does not appear until that refresh or a full `open`.
- The center empty state links to Chat. Chat is not how this map proves Runs.
