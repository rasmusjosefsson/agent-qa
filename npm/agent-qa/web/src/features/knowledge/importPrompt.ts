// web/src/features/knowledge/importPrompt.ts
// Builds the instruction handed to the chat agent to import existing test
// cases into the workbench. Fetch path that works today:
//   • acli (Jira REST via OAuth) — issue fields + Xray JQL membership.
//     Xray STEP CONTENT IS NOT IN JIRA FIELDS — it only renders inside the
//     issue page's Xray panel, so steps come from the chat's own browser.
//   • agent-browser — open the issue page and read the Xray panel DOM.
//     Needs a signed-in Atlassian session in the session jar; if the page
//     bounces to id.atlassian.com/login the user must do the one-time
//     browser sign-in (the auth plugin's cookie store persists it after).
// Persists cases (and a set) via the local /api/cases and /api/sets writes.

// A safe id derived from an external key (e.g. "PROJ-123" → "proj-123").
export function caseIdFromKey(key: string): string {
  return (
    key
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'imported-case'
  )
}

const FETCH_STEPS = [
  'How to read an issue:',
  '  • Fields: `acli jira workitem view <KEY> --fields summary,description` (acli is on PATH and already authed).',
  '  • Jira host: `acli jira auth status` prints "Site: <host>" — build URLs as https://<host>/browse/<KEY>.',
  '  • Xray test steps are NOT in the API fields. Open https://<host>/browse/<KEY> in the chat browser (`agent-browser --session "$AGENT_BROWSER_SESSION" open <url>`), snapshot, and read the Xray "Test details" panel — steps, preconditions, expected results all render in the DOM.',
  '  • If the page bounces to id.atlassian.com/login, STOP and tell the user: the workbench browser needs a one-time Atlassian sign-in (headed window, SSO button). Do not retry in a loop.',
].join('\n')

export function buildJiraImportPrompt(issueKey: string, apiBase: string): string {
  const key = issueKey.trim()
  const id = caseIdFromKey(key)
  const url = `${apiBase}/api/cases/${id}`
  return [
    `Import Jira issue ${key} into the agent-qa workbench as a test case.`,
    '',
    FETCH_STEPS,
    '',
    `1. Fetch issue ${key} — read it as a QA test case and extract:`,
    '   - title   = the issue summary',
    '   - steps   = the numbered test/repro steps (Xray panel if present, else description — one plain-English line each)',
    '   - expected = the "Expected result" / acceptance text',
    '   - startUrl = a URL mentioned in the description, else leave blank',
    '   - keep any test-data values as [TOKEN] placeholders where appropriate (e.g. [EMAIL]).',
    '2. Save it to the local workbench (do NOT invent fields):',
    `   curl -s -X POST "${url}" -H 'content-type: application/json' \\`,
    `     -d '{"title":"…","startUrl":"…","steps":["…","…"],"expected":"…","source":"jira","sourceRef":"${key}","tags":["jira"]}'`,
    `3. Confirm with the new case id (${id}) and a one-line summary. It will then appear under Test Cases, ready to run with the agent.`,
  ].join('\n')
}

// What an Xray import is scoped to. A plan/set imports its member tests; a
// story/epic imports the tests covering it.
export type XrayContainer = 'plan' | 'set' | 'story' | 'epic'

const CONTAINER_LABEL: Record<XrayContainer, string> = {
  plan: 'test plan',
  set: 'test set',
  story: 'story',
  epic: 'epic',
}

// Xray JQL membership — verified live: testPlanTests + testSetTests exist;
// story/epic have no coverage JQL on this Jira, so those read the Xray
// coverage panel on the issue page via the browser instead.
const CONTAINER_JQL: Partial<Record<XrayContainer, string>> = {
  plan: 'issue in testPlanTests(%s)',
  set: 'issue in testSetTests(%s)',
}

// A safe set id derived from a container key (e.g. "PROJ-123" → "set-proj-123").
export function setIdFromKey(key: string): string {
  return `set-${caseIdFromKey(key)}`
}

// Import every test under an Xray container as a case, then group them into a
// Test Set. Mirrors the Jira single-issue flow but fan-out + a set, so the
// imported cases land already organized.
export function buildXrayImportPrompt(
  containerKey: string,
  container: XrayContainer,
  apiBase: string
): string {
  const key = containerKey.trim()
  const setId = setIdFromKey(key)
  const label = CONTAINER_LABEL[container]
  const jqlFn = CONTAINER_JQL[container]
  const listStep = jqlFn
    ? [
        '1. List the member tests:',
        `   acli jira workitem search --jql "${jqlFn.replace('%s', key)}" --fields summary,issuetype --json`,
        '   (if the JQL function errors, report it and stop — do not fan out by guessing keys)',
      ]
    : [
        `1. List the member tests: open https://<host>/browse/${key}` +
          " in the chat browser, snapshot, and collect the test keys from the issue's Xray coverage/tests panel (do not guess keys)",
      ]
  return [
    `Import the Xray ${label} ${key} into the agent-qa workbench as test cases grouped in a set.`,
    '',
    FETCH_STEPS,
    '',
    ...listStep,
    '2. For EACH test key, open the issue in the chat browser and read the Xray panel for: title (summary), steps (numbered repro steps, one plain line each), expected (acceptance text), startUrl (a URL if mentioned, else blank). Keep test-data values as [TOKEN] placeholders (e.g. [EMAIL]).',
    '3. Save EACH test as a case (id = lowercased test key, e.g. "proj-123"); do NOT invent fields:',
    `   curl -s -X POST "${apiBase}/api/cases/<case-id>" -H 'content-type: application/json' \\`,
    `     -d '{"title":"…","startUrl":"…","steps":["…"],"expected":"…","source":"xray","sourceRef":"<TEST-KEY>","tags":["xray"],"externalRefs":[{"provider":"xray","key":"<TEST-KEY>","url":"<test url>"}]}'`,
    `4. Group the imported cases into a Test Set named after ${key} (manual membership):`,
    `   curl -s -X POST "${apiBase}/api/sets/${setId}" -H 'content-type: application/json' \\`,
    `     -d '{"name":"<${label} summary>","mode":"manual","caseIds":["<case-id>","…"],"source":"xray","sourceRef":"${key}"}'`,
    `5. Confirm with how many cases were imported and the set id (${setId}). They'll appear under Test Cases and Test Sets, ready to add to a plan and run.`,
  ].join('\n')
}
