# Settings

Settings shows which scenarios root this workbench is using and stores the headed-browser default in that root.

## Sub-features

- `settings-root` shows the disposable scenarios root for this run.
- `settings-headed` turns on headed browser by default and keeps it after a reload.

## How to get to it (user POV)

- Choose `Settings` in the sidebar.
- Open `/settings` directly.

## Driving it with verify-agent-qa

Preconditions:

- `doctor.sh` printed `ok` for this run.
- `$VERIFY_ROOT/_config/settings.json` does not exist yet, so the headed checkbox starts unchecked.

- **Sidebar entry.** From Runs, choose `Settings`. Run `scripts/browser.sh open "$VERIFY_URL"`, `scripts/browser.sh set viewport 1280 800`, and `scripts/browser.sh find role link click --name "Settings"`. The heading reads `Settings`.
- **Direct entry.** Run `scripts/browser.sh open "$VERIFY_URL/settings"`. The same heading is shown.
- **Root.** The Paths section's `Scenario root` text is exactly `$VERIFY_ROOT`. A different path means this is the wrong workbench; stop and run `doctor.sh`.
- **Turn headed on.** Choose the checkbox named `Headed browser by default`. Run `scripts/browser.sh find role checkbox click --name "Headed browser by default"`. The page shows `Saved`, and `scripts/browser.sh is checked "#headed-default"` prints a truthy result.
- **Confirm the file.** `$VERIFY_ROOT/_config/settings.json` contains `"headedDefault": true`. Copy it to `$VERIFY_EVIDENCE/settings/settings.json`.
- **Reload.** Run `scripts/browser.sh open "$VERIFY_URL/settings"` and `scripts/browser.sh wait "#headed-default"`. `scripts/browser.sh is checked "#headed-default"` is still truthy, and the scenario root line is still `$VERIFY_ROOT`.
- **Proof.** Save `scripts/browser.sh snapshot` and `scripts/browser.sh screenshot "$VERIFY_EVIDENCE/settings/page.png"` after the reload. `note.txt` says `settings-headed`.

## Gotchas

- The checkbox writes immediately. There is no separate save button. `Saved` is the confirmation text, not a status role.
- `AGENT_QA_CHAT_BACKEND` locks the agent-backend select. This recipe does not change that select. An env-locked backend is not a failed headed-default write.
- The headed default applies to the next replay started by this workbench. It does not retroactively change a replay that already finished.
- Reading the repo `tmp/agent-qa-scenarios/_config/settings.json` is the wrong file. Only `$VERIFY_ROOT/_config/settings.json` counts.
