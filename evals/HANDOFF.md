# QA Playground Eval Handoff

## Branch

Current branch:

```bash
add-qaplayground-evals
```

Working goal: reach 100% reliable record + replay coverage for QA Playground eval cases, starting one page at a time.

## Key Decision

Use two layers:

1. **Golden deterministic runners** prove the framework can record/flush/verify/replay a case 100% with fixed commands.
2. **Model evals** prove a cheap model can discover and execute the workflow.

Do not let model stalls block framework hardening. If model eval flakes or times out, use golden runner to isolate framework capability.

## What Exists

Eval harness:

```bash
evals/lib/harness.ts
evals/run.ts
evals/cases.ts
evals/saucedemo.ts
```

QA Playground checklist and tickets:

```bash
evals/QA_PLAYGROUND_CHECKLIST.md
```

Repeatable runbook:

```bash
evals/QA_PLAYGROUND_RUNBOOK.md
```

Running log of fixes/discoveries:

```bash
evals/QA_PLAYGROUND_LOG.md
```

External practice-site backlog:

```bash
evals/PRACTICE_SITES.md
```

Supervised-loop skill:

```bash
.agents/skills/qaplayground-eval-loop/SKILL.md
```

Golden deterministic Forms runners:

```bash
evals/golden/forms-tc01.ts
evals/golden/forms-tc02.ts
evals/golden/forms-tc03.ts
evals/golden/forms-tc04.ts
evals/golden/forms-tc05.ts
```

Golden deterministic Bank Login runners:

```bash
evals/golden/bank-login-tc01.ts
evals/golden/bank-login-tc02.ts
evals/golden/bank-login-tc03.ts
evals/golden/bank-login-tc04.ts
evals/golden/bank-login-tc05.ts
```

## Current Catalog

QA Playground is split to documented test-case level.

```bash
cd evals
bun run run.ts --suite qaplayground --list | wc -l
# 163
```

Forms page list:

```bash
cd evals
bun run run.ts --suite qaplayground --page forms --list
```

Includes page-load plus TC01-TC15.

Bank page list:

```bash
cd evals
bun run run.ts --suite qaplayground --page bank --list
```

Includes page-load plus TC-LOGIN-01 through TC-LOGIN-05.

## Proven Passing Cases

Forms TC01 golden passed end-to-end.

Command:

```bash
cd evals
bun run golden:forms:tc01
```

Result:

```text
SUMMARY: 15/15 (PASS)
```

Artifact root:

```text
evals/results/golden-forms-tc01-2026-06-16T23-12-43-662Z
```

SID:

```text
s-2026-06-16T23-12-43-697Z__2ad27687
```

Important: this proves the framework can record/replay TC01. It does not prove cheap-model eval reliability.

Forms TC02 golden passed end-to-end.

Command:

```bash
cd evals
bun run golden:forms:tc02
```

Result:

```text
SUMMARY: 5/5 (PASS)
```

Artifact root:

```text
evals/results/golden-forms-tc02-2026-06-16T23-27-26-791Z
```

SID:

```text
s-2026-06-16T23-27-27-362Z__31b6dbdd
```

Notes:
- The live page renders TC02 validation messages as text, not the documented `error-*` test ids.
- Replay now uses native DOM click for raw CSS/test-id click locators before falling back to `agent-browser click`, because `agent-browser click '[data-testid="submit-form-btn"]'` reported success without dispatching form submit for this page.

Forms TC03 golden passed end-to-end.

Command:

```bash
cd evals
bun run golden:forms:tc03
```

Result:

```text
SUMMARY: 4/4 (PASS)
```

Artifact root:

```text
evals/results/golden-forms-tc03-2026-06-16T23-35-37-634Z
```

SID:

```text
s-2026-06-16T23-35-37-669Z__7b2a0fa3
```

Notes:
- TC03 uses `#email:invalid` as the replay assertion, not text, because the live page exposes invalid email through native HTML constraint validation.
- A broad text check for `Please enter a valid email address.` is unsafe on this page because replay can match tutorial/documentation text instead of the live form state.
- The TC03 runner includes a direct live browser selector check before flushing, so a golden pass proves the driven browser reached the intended state.

Forms TC04 golden passed end-to-end.

Command:

```bash
cd evals
bun run golden:forms:tc04
```

Result:

```text
SUMMARY: 15/15 (PASS)
```

Artifact root:

```text
evals/results/golden-forms-tc04-2026-06-16T23-40-13-946Z
```

SID:

```text
s-2026-06-16T23-40-13-977Z__a154e8c3
```

Notes:
- `#phone` is `type="tel"` with no native validation attributes, so TC04 cannot use a `:invalid` assertion.
- The live app emits `Enter a valid 10-digit phone number.` in `#phoneError` / `[data-testid="error-phone"]` after submit.
- The TC04 runner fills the other required fields with valid values, submits `#phone = abc`, and records `wait selector [data-testid="error-phone"]`.
- The runner includes a direct live text check before flushing so a golden pass proves the specific phone validation appeared in the live browser.

Forms TC05 golden passed end-to-end.

Command:

```bash
cd evals
bun run golden:forms:tc05
```

Result:

```text
SUMMARY: 15/15 (PASS)
```

Artifact root:

```text
evals/results/golden-forms-tc05-2026-06-16T23-52-01-968Z
```

SID:

```text
s-2026-06-16T23-52-02-003Z__8dc18cc5
```

Notes:
- `#password` is `type="password"` with no native `minlength`, `pattern`, or `required` attribute, so TC05 cannot use a native `:invalid` assertion.
- The live app emits `Password must be at least 6 characters.` in `#passwordError` / `[data-testid="error-password"]` after submit.
- The TC05 runner fills the other required fields with valid values, submits matching too-short password and confirm password values, and records `wait selector [data-testid="error-password"]`.
- Matching confirm password isolates TC05 from TC06's password mismatch validation.
- The runner includes a direct live text check before flushing so a golden pass proves the specific password validation appeared in the live browser.

Bank Login TC-LOGIN-01 through TC-LOGIN-05 golden runners passed end-to-end.

Commands:

```bash
cd evals
bun run golden:bank:login:tc01
bun run golden:bank:login:tc02
bun run golden:bank:login:tc03
bun run golden:bank:login:tc04
bun run golden:bank:login:tc05
```

Results:

```text
TC01 SUMMARY: 6/6 (PASS)
TC02 SUMMARY: 6/6 (PASS)
TC03 SUMMARY: 7/7 (PASS)
TC04 SUMMARY: 5/5 (PASS)
TC05 SUMMARY: 9/9 (PASS)
```

Artifacts/SIDs:

```text
TC01 root: evals/results/golden-bank-login-tc01-2026-06-17T08-34-35-877Z
TC01 sid:  s-2026-06-17T08-34-36-275Z__21cd8e4c
TC02 root: evals/results/golden-bank-login-tc02-2026-06-17T08-35-13-256Z
TC02 sid:  s-2026-06-17T08-35-13-289Z__27bb09c9
TC03 root: evals/results/golden-bank-login-tc03-2026-06-17T08-35-51-082Z
TC03 sid:  s-2026-06-17T08-35-51-115Z__68c1bb2d
TC04 root: evals/results/golden-bank-login-tc04-2026-06-17T08-36-34-293Z
TC04 sid:  s-2026-06-17T08-36-34-327Z__e4484ac3
TC05 root: evals/results/golden-bank-login-tc05-2026-06-17T08-40-57-740Z
TC05 sid:  s-2026-06-17T08-40-58-403Z__e2772964
```

Notes:
- Added Bank Login catalog entries under page slug `bank`: page-load plus TC-LOGIN-01 through TC-LOGIN-05.
- Added `evals/golden/bank-login-lib.ts` plus five thin Bank Login runners.
- TC-LOGIN-01 live `[data-testid="page-title"]` text is `SecureBank Dashboard — QA Automation Practice`, not exactly documented `Dashboard`.
- TC-LOGIN-02 uses `[data-testid="login-alert"]` with text containing `Invalid username or password` and URL still `/bank`.
- TC-LOGIN-03 uses selector waits for `#password[type="password"]` and `#password[type="text"]`.
- TC-LOGIN-04 records `pressKey` with `Enter` after filling the password field.
- TC-LOGIN-05 verifies viewer badge/role indicator, then navigates to `/bank/accounts` and asserts Add Account selectors are absent.
- Framework fix: added `record-step wait` condition kind `selectorAbsent`, mapped to raw CSS `isHidden`, because role/name absent fell back to broad text and matched the docs copy `Add New Account`.
- Updated embedded core skill and `record-step` help to document `selectorAbsent`.

## Model Eval Issues Observed

The cheap-model eval initially had several orchestration failures:

- It used a global `agent-qa` binary with stale `do/check` and `journey.env` semantics.
- It guessed `node_modules/.bin/agent-browser` paths.
- It generated too-long session names, causing agent-browser Unix socket failures.
- It manually edited generated `scenario.json` to make replay pass, which is now forbidden.
- It timed out with no useful artifact on one run.

Harness fixes already applied:

- Emits `[eval] START`, `[eval] HEARTBEAT`, `[eval] END`.
- Writes `status.json` per run.
- Generates short session names.
- Creates `/tmp/agent-qa-eval-bin/agent-qa` and `/tmp/agent-qa-eval-bin/agent-browser` wrappers.
- Pins `PATH` to wrapper + `cli/target/debug`.
- Fails if stdout/stderr indicates manual artifact edits.
- Looks for replay success in both `audit.json` and `replay.json`.

## Next Target

Continue with **Forms TC06**.

Catalog case id:

```text
qaplayground-forms-tc06-verify-password-mismatch-shows-confirm-password-error
```

Suggested golden runner to create:

```bash
evals/golden/forms-tc06.ts
```

Expected TC06 flow:

1. Start agent-qa recording with short session.
2. Open `https://qaplayground.com/practice/forms`.
3. Record navigation.
4. Fill required fields with otherwise-valid data.
5. Fill password and confirm password with otherwise-valid but different values after live inspection.
6. Click submit button.
7. Assert confirm-password mismatch validation appears, after live inspection identifies the real DOM/state signal.
8. Flush.
9. Verify.
10. Replay.

Known selectors/test ids from Forms page:

```text
phone input: #phone
password input: #password
confirm password input: #confirmPassword
submit button: [data-testid="submit-form-btn"] or #submitFormBtn
```

Current TC06 inspection note:

- TC05 proved app-level password validation errors can be asserted with `record-step wait` selector checks when stable error nodes exist.
- Inspect live `#confirmPassword` mismatch behavior before choosing the TC06 assertion; do not assume documented error ids are present.

Current recorder assertion note:

- `record-step assert` supports `present`, `absent`, `url`.
- `present` / `absent` map to role/name, not CSS selector.
- `record-step wait` supports `selector`, `selectorAbsent`, `text`, and `url`; TC02 uses text waits for validation copy.
- Prefer `selectorAbsent` over role/name `assert absent` when the page includes documentation text that can match the absent element name.

## Recommended Next-Agent Workflow

1. Run current checks:

```bash
cd evals
bun run run.ts --suite qaplayground --page forms --list
cd ../cli
cargo test --locked
```

2. Copy `evals/golden/forms-tc05.ts` to `evals/golden/forms-tc06.ts` and adjust steps.

3. Run:

```bash
cd evals
bun run golden/forms-tc06.ts
```

4. If it fails because selector/text assertion is not representable, fix agent-qa framework rather than hand-editing scenarios.

5. After golden TC06 passes, optionally run cheap-model eval:

```bash
cd evals
bun run run.ts --case qaplayground-forms-tc06-verify-password-mismatch-shows-confirm-password-error --model github-copilot/gpt-5-mini --json --timeout-ms 600000
```

6. Update `evals/QA_PLAYGROUND_CHECKLIST.md` with TC06 status.

## Rules

- Do not hand-edit generated `scenario.json` or `scenario.steps.jsonl` as a success path.
- If replay fails, fix framework/docs/eval prompt/golden runner, then re-record.
- Commit only source/docs/harness changes, not ignored `evals/results/` artifacts.
