# QA Playground Running Log

Append to this after each TC. Keep `HANDOFF.md` focused on the next agent's current state; keep this file as the historical record of what changed and why.

## Log Template

```markdown
## YYYY-MM-DD - <Page> <TCxx>

Result: golden pass | blocked | framework fixed | model-only failure

Commands:
- `bun run golden:<page>:<tc>`

Artifact:
- root: `evals/results/<runId>`
- sid: `<sid>`

What changed:
- ...

What we learned:
- ...

Follow-ups:
- ...
```

## 2026-06-16 - Forms TC01

Result: golden pass.

Commands:
- `bun run golden:forms:tc01`

Artifact:
- root: `evals/results/golden-forms-tc01-2026-06-16T23-12-43-662Z`
- sid: `s-2026-06-16T23-12-43-697Z__2ad27687`

What changed:
- Added deterministic Forms TC01 golden runner.
- Proved full valid form submit can record, flush, verify, and replay.

What we learned:
- Golden runners are the right first layer; they separate framework capability from cheap-model behavior.
- TC01 success does not prove model eval reliability.

Follow-ups:
- Continue Forms page linearly.

## 2026-06-16 - Forms TC02

Result: golden pass with framework fixes.

Commands:
- `bun run golden:forms:tc02`
- `cargo test --locked`

Artifact:
- root: `evals/results/golden-forms-tc02-2026-06-16T23-29-11-453Z`
- sid: `s-2026-06-16T23-29-12-028Z__0fe7e4fe`

What changed:
- Added deterministic Forms TC02 golden runner.
- Added `golden:forms:tc02` script.
- Fixed raw CSS/test-id presence checks in `cli/src/claims.rs` to use `document.querySelector` through `agent-browser eval` instead of unsupported `agent-browser find css`.
- Fixed raw CSS/test-id click replay in `cli/src/verbs.rs` for native controls so submit buttons dispatch real DOM click/form behavior before falling back to `agent-browser click`.
- Added focused Rust coverage for the raw CSS native-control click path.

What we learned:
- Current `agent-browser` does not support `find css`; CSS checks need DOM eval or top-level selector verbs depending on intent.
- `agent-browser click '[data-testid="submit-form-btn"]'` reported success on QA Playground Forms but did not dispatch submit validation in replay.
- Native DOM `mousedown`/`mouseup`/`click` on the submit button triggers the live page validation messages.
- The live page renders TC02 validation messages as visible text, not the documented `error-*` test ids.
- `record-step wait` with `kind: "text"` is currently the most stable way to assert TC02 errors.

Follow-ups:
- Continue with Forms TC03: invalid email validation.
- Prefer live DOM inspection before trusting documented selectors/test ids.
- If a TC needs value assertions, add framework support instead of hand-editing generated artifacts.

## 2026-06-16 - Forms TC03

Result: golden pass.

Commands:
- `bun run golden:forms:tc03`
- `cargo test --locked`

Artifact:
- root: `evals/results/golden-forms-tc03-2026-06-16T23-35-37-634Z`
- sid: `s-2026-06-16T23-35-37-669Z__7b2a0fa3`

What changed:
- Added deterministic Forms TC03 golden runner.
- Added `golden:forms:tc03` script.
- TC03 records invalid email fill, submit, and a selector wait for `#email:invalid`.
- Added a live browser check before flush so the runner cannot pass by matching tutorial text outside the form.

What we learned:
- The live Forms page does not render an in-page invalid-email error element for TC03.
- Invalid email is exposed as native HTML constraint validation on `#email` (`type="email"`, `:invalid`, browser `validationMessage`).
- A plain text wait for `Please enter a valid email address.` is unsafe here because replay can match documentation/tutorial text on the same page.
- Raw CSS selector waits already cover pseudo-class checks such as `#email:invalid`; no framework schema change was needed.
- TC04 is not equivalent to TC03: `#phone` is `type="tel"` with no `pattern`, `required`, `minlength`, or `maxlength` attributes, so an invalid-phone case needs live behavior inspection before choosing an assertion.

Follow-ups:
- Continue with Forms TC04: invalid phone number format.
- Prefer selector/state checks over broad text checks when QA Playground documentation includes the same validation copy on-page.

## 2026-06-16 - Forms TC04

Result: golden pass.

Commands:
- `bun run golden:forms:tc04`
- `cargo test --locked`

Artifact:
- root: `evals/results/golden-forms-tc04-2026-06-16T23-40-13-946Z`
- sid: `s-2026-06-16T23-40-13-977Z__a154e8c3`

What changed:
- Added deterministic Forms TC04 golden runner.
- Added `golden:forms:tc04` script.
- TC04 records an otherwise-valid form submission with `#phone = abc`, then waits for `[data-testid="error-phone"]`.
- Added a live browser check before flush to prove the specific phone error text is present in the form.

What we learned:
- `#phone` is `type="tel"` with no native HTML validation attributes, so `:invalid` is not useful for TC04.
- Phone validation is app-level submit validation and renders `Enter a valid 10-digit phone number.` in `#phoneError` / `[data-testid="error-phone"]`.
- Filling the other required fields prevents unrelated validation messages from obscuring the TC04 signal.

Follow-ups:
- Continue with Forms TC05: password minimum length validation.
- Inspect live password validation before choosing selector/text; the docs mention password minimum length but the exact live error node needs confirmation.

## 2026-06-16 - Forms TC05

Result: golden pass.

Commands:
- `bun run golden:forms:tc05`
- `cargo test --locked`

Artifact:
- root: `evals/results/golden-forms-tc05-2026-06-16T23-52-01-968Z`
- sid: `s-2026-06-16T23-52-02-003Z__8dc18cc5`

What changed:
- Added deterministic Forms TC05 golden runner.
- Added `golden:forms:tc05` script.
- TC05 records an otherwise-valid form submission with matching too-short password and confirm password values, then waits for `[data-testid="error-password"]`.
- Added a live browser check before flush to prove the password minimum-length error text is present in the form.

What we learned:
- `#password` is `type="password"` with no native `minlength`, `pattern`, or `required` attribute, so TC05 is app-level validation, not a native `:invalid` case.
- The live app renders `Password must be at least 6 characters.` in `#passwordError` / `[data-testid="error-password"]` after submit.
- Matching the short confirm password isolates TC05 from TC06's password mismatch behavior.

Follow-ups:
- Continue with Forms TC06: password mismatch shows confirm password error.
- Inspect live confirm-password mismatch behavior before choosing the selector/text assertion.

## 2026-06-17 - Bank Login TC-LOGIN-01 through TC-LOGIN-05

Result: golden pass with framework fix.

Commands:
- `bun run run.ts --suite qaplayground --page bank --list`
- `bun run golden:bank:login:tc01`
- `bun run golden:bank:login:tc02`
- `bun run golden:bank:login:tc03`
- `bun run golden:bank:login:tc04`
- `bun run golden:bank:login:tc05`
- `cargo build --locked`
- `cargo test --locked`

Artifact:
- TC01 root: `evals/results/golden-bank-login-tc01-2026-06-17T08-34-35-877Z`
- TC01 sid: `s-2026-06-17T08-34-36-275Z__21cd8e4c`
- TC02 root: `evals/results/golden-bank-login-tc02-2026-06-17T08-35-13-256Z`
- TC02 sid: `s-2026-06-17T08-35-13-289Z__27bb09c9`
- TC03 root: `evals/results/golden-bank-login-tc03-2026-06-17T08-35-51-082Z`
- TC03 sid: `s-2026-06-17T08-35-51-115Z__68c1bb2d`
- TC04 root: `evals/results/golden-bank-login-tc04-2026-06-17T08-36-34-293Z`
- TC04 sid: `s-2026-06-17T08-36-34-327Z__e4484ac3`
- TC05 root: `evals/results/golden-bank-login-tc05-2026-06-17T08-40-57-740Z`
- TC05 sid: `s-2026-06-17T08-40-58-403Z__e2772964`

What changed:
- Added QA Playground Bank Login catalog entries for page-load plus TC-LOGIN-01 through TC-LOGIN-05.
- Added `evals/golden/bank-login-lib.ts` and five deterministic Bank Login golden runners.
- Added `golden:bank:login:tc01` through `golden:bank:login:tc05` scripts.
- Added recorder support for `wait` condition kind `selectorAbsent`, mapped to raw CSS `isHidden`.
- Updated embedded core skill and `record-step` help to document `selectorAbsent`.
- Added focused Rust coverage for selector-absence wait mapping.

What we learned:
- TC-LOGIN-01 redirects to `/bank/dashboard`, but `[data-testid="page-title"]` text is `SecureBank Dashboard — QA Automation Practice`, not the documented exact text `Dashboard`.
- TC-LOGIN-02 renders `⚠️ Invalid username or password. Please try again.` in `[data-testid="login-alert"]` / `role="alert"` and remains on `/bank`.
- TC-LOGIN-03 can assert password visibility with CSS selectors `#password[type="password"]` and `#password[type="text"]`.
- TC-LOGIN-04 works with recorder action method `pressKey` and value `Enter` after filling the password field.
- TC-LOGIN-05 viewer login shows `[data-testid="viewer-badge"] = Read-only` and `[data-testid="role-indicator"]` containing `Read-only Viewer`.
- TC-LOGIN-05 exposed a false positive in role/name absence: broad text fallback matched the page's test-case documentation text `Add New Account`. Selector absence avoids this by checking the real button selectors.

Follow-ups:
- Continue Forms TC06 if returning to the prior linear Forms track.
- For future absence checks on pages that include documentation text, prefer `record-step wait` with `selectorAbsent` over role/name `assert absent` unless the accessible target is isolated from docs copy.
