// web/src/features/cases/prompt.ts
// Builds the instruction handed to the Copilot (chat agent) via /chat?ask=…
// so it drives + records the case into a replayable scenario.json, then links
// the resulting sid back to the case.
import type { CaseRecord } from './types'

// The scenario `intent` we ask the agent to record under. Carries the case id
// so the link-back poller can match the resulting scenario unambiguously.
export function runIntent(c: Pick<CaseRecord, 'title' | 'id'>): string {
  return `${c.title} [${c.id}]`
}

export function buildRunPrompt(c: CaseRecord, apiBase: string): string {
  const steps = c.steps
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s, i) => `${i + 1}. ${s}`)
    .join('\n')

  const data = Object.entries(c.inputs || {})
    .map(([k, d]) => {
      const v = d.sensitive
        ? '<use a real test value; keep it secret / redacted in the recording>'
        : d.default != null && d.default !== ''
          ? JSON.stringify(d.default)
          : '<fill a realistic value>'
      return `  [${k}] = ${v}`
    })
    .join('\n')

  const intent = runIntent(c)
  const linkUrl = `${apiBase}/api/cases/${encodeURIComponent(c.id)}/link`

  const lines: string[] = [
    'Use the agent-qa skill to turn this QA test case into a replayable scenario, then link it back to the case.',
    '',
    'First load the skill: run `agent-qa skills get core`.',
    '',
    `Test case: "${c.title}"  (case id: ${c.id})`,
    `Start URL: ${c.startUrl || '(none — infer from the steps)'}`,
  ]
  if (c.preconditions) lines.push(`Preconditions: ${c.preconditions}`)
  lines.push(
    '',
    'Record these steps in a real browser, one at a time, substituting any [TOKEN] placeholders with the test data below:',
    steps || '(no steps authored)'
  )
  if (data) lines.push('', 'Test data (placeholders → values):', data)
  lines.push(
    '',
    'Then verify the EXPECTED RESULT and record it as assertion step(s):',
    `Expected: ${c.expected || '(none specified)'}`,
    '',
    'Workflow:',
    `1. Run \`agent-qa start "${intent}" --open "${c.startUrl}"\` to begin recording (this mints the scenario id).`,
    '2. Perform and record each step above. Map every [TOKEN] placeholder to the scenario `inputs` so the data stays parameterized.',
    '3. Record assertion step(s) for the expected result.',
    '4. Run `agent-qa flush` to write scenario.json.',
    '5. Report the resulting scenario id on its own line as: SCENARIO_SID=<sid>',
    '6. Link it to this case so the workbench shows the result:',
    `   curl -s -X POST "${linkUrl}" -H 'content-type: application/json' -d '{"scenarioSid":"<sid>"}'`
  )
  return lines.join('\n')
}
