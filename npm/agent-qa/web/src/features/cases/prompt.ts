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
  // Verbatim — imported steps may already carry their own numbering.
  const steps = c.steps
    .map((s) => s.trim())
    .filter(Boolean)
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
    '## Target + sign-in — resolve BEFORE recording',
    '',
    'Check the chat sign-in state first:',
    '  curl -s "$AGENT_QA_BASE/api/chat/c/$AGENT_QA_CHAT_ID/connection"',
    '',
    `- Start URL: ${c.startUrl || '(none given — pick the target yourself: list environments via `curl -s "$AGENT_QA_BASE/api/environments"` and pick the one whose routes match the steps, then derive the URL from its baseUrl)'}`,
    '',
    'If `state` is "disconnected" and the target needs auth: list personas/environments via /api/personas + /api/environments, pick the matching environment, and connect BEFORE `agent-qa start` —',
    '  curl -s -X POST "$AGENT_QA_BASE/api/chat/c/$AGENT_QA_CHAT_ID/connect" -H \'content-type: application/json\' -d \'{"personaId":"<id>","environmentId":"<env>"}\'',
    'Connecting first stamps a useProfile baseline into the scenario so replays re-authenticate. If the user must choose the persona, ask instead of guessing.',
    '',
    `Test case: "${c.title}"  (case id: ${c.id})`,
  ]
  if (c.preconditions) lines.push(`Preconditions: ${c.preconditions}`)
  lines.push(
    '',
    'Record these steps in a real browser, one at a time, substituting any [TOKEN] placeholders with the test data below (steps may carry their own numbering — keep it):',
    steps || '(no steps authored — derive a sensible flow from the title)'
  )
  if (data) lines.push('', 'Test data (placeholders → values):', data)
  lines.push(
    '',
    'Then verify the EXPECTED RESULT and record it as assertion step(s):',
    `Expected: ${c.expected || '(none specified — assert the outcome the steps imply)'}`,
    '',
    'Recording rules — read carefully:',
    '- `smart-click` / `smart-fill` / `smart-assert` / `smart-select` ALREADY record the step — never call `record-step` for the same action. One action = one recorded step.',
    '- Failed attempts and retries land in the buffer as real steps too. Before flushing, run `agent-qa buffer list` and `agent-qa buffer delete <id>` the dead/duplicate steps — roughly one step per test step.',
    '',
    'Workflow:',
    `1. ${c.startUrl ? `Run \`agent-qa start "${intent}" --open "${c.startUrl}"\`` : `Run \`agent-qa start "${intent}"\``} to begin recording (this mints the scenario id).`,
    '2. Drive each step with a smart verb or a single `record-step`. Map every [TOKEN] placeholder to the scenario `inputs` so the data stays parameterized.',
    '3. Record assertion step(s) for the expected result.',
    '4. Prune the buffer (`buffer list`/`buffer delete`), then run `agent-qa flush` to write scenario.json.',
    '5. Report the resulting scenario id on its own line as: SCENARIO_SID=<sid>',
    '6. Link it to this case so the workbench shows the result:',
    `   curl -s -X POST "${linkUrl}" -H 'content-type: application/json' -d '{"scenarioSid":"<sid>"}'`
  )
  return lines.join('\n')
}
