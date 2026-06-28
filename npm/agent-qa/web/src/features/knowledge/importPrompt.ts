// web/src/features/knowledge/importPrompt.ts
// Builds the instruction handed to the Copilot (chat agent) to import existing
// test cases from a connected source into the workbench. The agent uses its own
// connector (e.g. an Atlassian MCP/skill) to fetch, then persists cases (and a
// set) via the local /api/cases and /api/sets write surfaces.

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

export function buildJiraImportPrompt(issueKey: string, apiBase: string): string {
  const key = issueKey.trim()
  const id = caseIdFromKey(key)
  const url = `${apiBase}/api/cases/${id}`
  return [
    `Import Jira issue ${key} into the agent-qa workbench as a test case.`,
    '',
    `1. Fetch Jira issue ${key} using your Atlassian connector (MCP or the atlassian-cli skill). If you have no Atlassian access, say so and stop.`,
    '2. Read it as a QA test case and extract:',
    '   - title   = the issue summary',
    '   - steps   = the numbered test/repro steps from the description (one plain-English line each)',
    '   - expected = the "Expected result" / acceptance text',
    '   - startUrl = a URL mentioned in the description, else leave blank',
    '   - keep any test-data values as [TOKEN] placeholders where appropriate (e.g. [EMAIL]).',
    '3. Save it to the local workbench (do NOT invent fields):',
    `   curl -s -X POST "${url}" -H 'content-type: application/json' \\`,
    `     -d '{"title":"…","startUrl":"…","steps":["…","…"],"expected":"…","source":"jira","sourceRef":"${key}","tags":["jira"]}'`,
    `4. Confirm with the new case id (${id}) and a one-line summary. It will then appear under Test Cases, ready to run with the agent.`,
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
  const member = container === 'plan' || container === 'set' ? 'member tests' : 'tests covering it'
  return [
    `Import the Xray ${label} ${key} into the agent-qa workbench as test cases grouped in a set.`,
    '',
    `1. Using your Atlassian/Xray connector (MCP or the atlassian-cli skill), fetch the Xray ${label} ${key} and its ${member}. If you have no access, say so and stop.`,
    '2. For EACH test, extract: title (summary), steps (numbered repro steps, one plain line each), expected (acceptance text), startUrl (a URL if mentioned, else blank). Keep test-data values as [TOKEN] placeholders (e.g. [EMAIL]).',
    '3. Save EACH test as a case (id = lowercased test key, e.g. "proj-123"); do NOT invent fields:',
    `   curl -s -X POST "${apiBase}/api/cases/<case-id>" -H 'content-type: application/json' \\`,
    `     -d '{"title":"…","startUrl":"…","steps":["…"],"expected":"…","source":"xray","sourceRef":"<TEST-KEY>","tags":["xray"],"externalRefs":[{"provider":"xray","key":"<TEST-KEY>","url":"<test url>"}]}'`,
    `4. Group the imported cases into a Test Set named after ${key} (manual membership):`,
    `   curl -s -X POST "${apiBase}/api/sets/${setId}" -H 'content-type: application/json' \\`,
    `     -d '{"name":"<${label} summary>","mode":"manual","caseIds":["<case-id>","…"],"source":"xray","sourceRef":"${key}"}'`,
    `5. Confirm with how many cases were imported and the set id (${setId}). They'll appear under Test Cases and Test Sets, ready to add to a plan and run.`,
  ].join('\n')
}
