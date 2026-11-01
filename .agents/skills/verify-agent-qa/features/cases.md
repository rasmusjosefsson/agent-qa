# Cases

Cases lets a user create a titled test case from the workbench. The case is stored as `case.json` and remains after a reload.

## Sub-features

- `cases-open` opens the empty case list from the sidebar and from `/cases`.
- `cases-create` creates a case from the `New case` dialog.
- `cases-persist` shows the same title after a fresh load and in `case.json`.

## How to get to it (user POV)

- Choose `Cases` in the sidebar.
- Open `/cases` directly.
- On Sets, Plans, or Cases, choose `Cases` in the `How tests flow` strip. Runs does not show that strip.

## Driving it with verify-agent-qa

Preconditions:

- `doctor.sh` printed `ok` for this run.
- `$VERIFY_ROOT/_cases` does not exist yet.
- No case is titled `Verify checkout`.

- **Sidebar entry.** From Runs, choose `Cases`. Run `scripts/browser.sh open "$VERIFY_URL"`, `scripts/browser.sh set viewport 1280 800`, and `scripts/browser.sh find role link click --name "Cases"`. The heading reads `Test Cases` and the empty state reads `No test cases yet`.
- **Direct entry.** Run `scripts/browser.sh open "$VERIFY_URL/cases"`. The same heading and empty state are shown.
- **Open the dialog.** Choose `New case`. Run `scripts/browser.sh find role button click --name "New case"` and `scripts/browser.sh wait "#case-title"`. A dialog named `New test case` is visible and the `Title` textbox is empty.
- **Name the case.** Run `scripts/browser.sh find label "Title" fill "Verify checkout"`. The textbox value is `Verify checkout` and the dialog shows `id: verify-checkout`.
- **Capture the filled dialog.** Run `scripts/browser.sh snapshot >"$VERIFY_EVIDENCE/cases/dialog.aria.txt"` and `scripts/browser.sh screenshot "$VERIFY_EVIDENCE/cases/dialog.png"`. Both show `New test case` and `Verify checkout`.
- **Create.** Choose `Create`. Run `scripts/browser.sh find role button click --name "Create"` and `scripts/browser.sh wait "[aria-label='Case title']"`. `scripts/browser.sh get value "[aria-label='Case title']"` prints `Verify checkout`. The snapshot line is `textbox "Case title": Verify checkout`.
- **Confirm the file.** `$VERIFY_ROOT/_cases/verify-checkout/case.json` exists and its `title` is `Verify checkout`. Copy it to `$VERIFY_EVIDENCE/cases/case.json`.
- **Reload.** Run `scripts/browser.sh open "$VERIFY_URL/cases?id=verify-checkout"` and `scripts/browser.sh wait "[aria-label='Case title']"`. `scripts/browser.sh get value "[aria-label='Case title']"` prints `Verify checkout`.
- **Proof.** Run `scripts/browser.sh snapshot >"$VERIFY_EVIDENCE/cases/detail.aria.txt"` and `scripts/browser.sh screenshot "$VERIFY_EVIDENCE/cases/detail.png"`. `note.txt` says `cases-persist` and lists the sidebar entry and the `/cases` entry.

## Gotchas

- The header and the empty state each have a `New case` button. The first match opens the same dialog.
- The id is the slug of the title. `Verify checkout` becomes `verify-checkout`. A second create with that title writes the same directory.
- `Save` on the detail page stays disabled until a field changes. Create already wrote `case.json`; a disabled `Save` is not a failed create.
- `find role textbox text --name "Case title"` prints an empty line. The value is on the control, not its text content. Read it with `get value "[aria-label='Case title']"` or the snapshot line `textbox "Case title": Verify checkout`.
- The `Steps` and `Expected result` textareas are not associated with their visible labels. The title field is the named control. Do not claim those textareas were filled because the title saved.
- `Run with agent` sends the case to Chat. Chat is not part of this proof.
