// web/src/features/sources/importPrompt.ts
// Builds the instruction handed to the Copilot (chat agent) to import an
// existing test case from an issue tracker into the workbench. The agent uses
// its own connector (e.g. an Atlassian MCP/skill) to fetch the issue, then
// persists a case via the local /api/cases write surface.

// A safe case id derived from an issue key (e.g. "PROJ-123" → "proj-123").
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
