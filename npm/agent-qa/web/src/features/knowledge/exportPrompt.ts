// web/src/features/knowledge/exportPrompt.ts
// Builds the instruction handed to the Copilot (chat agent) to push a plan's
// run results back to Xray as a Test Execution. The agent uses its own
// connector (Atlassian/Xray MCP or the atlassian-cli skill) to create the
// execution and set each test's result. The workbench just assembles the
// verdicts + evidence links from cases that carry an Xray external ref.

export type XrayResult = 'pass' | 'fail' | 'todo'

export interface XrayExportItem {
  xrayKey: string
  title: string
  result: XrayResult
  runUrl?: string | null
}

const RESULT_WORD: Record<XrayResult, string> = {
  pass: 'PASSED',
  fail: 'FAILED',
  todo: 'TODO (not run)',
}

export function buildXrayExportPrompt(
  planName: string,
  items: XrayExportItem[],
  _apiBase: string
): string {
  const lines = items.map((i) => {
    const ev = i.result === 'fail' && i.runUrl ? `  — evidence: ${i.runUrl}` : ''
    return `   - ${i.xrayKey}: ${RESULT_WORD[i.result]}${ev}`
  })
  const fails = items.filter((i) => i.result === 'fail').length
  return [
    `Push the agent-qa results for plan "${planName}" to Xray as a Test Execution.`,
    '',
    '1. Using your Atlassian/Xray connector (MCP or the atlassian-cli skill), create a new Xray Test Execution titled `agent-qa — ' +
      planName +
      ' — <today\'s date>`. If you have no Xray access, say so and stop.',
    '2. Add these tests to the execution and set each result:',
    ...lines,
    `3. For each FAILED test (${fails}), add a short comment; if your connector supports attachments, attach the end-of-run screenshot referenced in that run's events (see the evidence link).`,
    '4. Confirm with the new Test Execution key and the pass/fail tally.',
  ].join('\n')
}
