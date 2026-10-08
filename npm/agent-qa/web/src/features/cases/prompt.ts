// web/src/features/cases/prompt.ts
// Thin typed wrappers over the shared prompt text in lib/case-prompts.js —
// the same strings the server uses when it seeds repair chats itself.
import type { ScenarioSummary } from '@/features/runs/types'
import type { CaseRecord } from './types'
// @ts-expect-error — plain-JS module in lib/ shared with the report server
import { runIntent as _runIntent, buildRunPromptText, buildRepairPromptText } from '../../../../lib/case-prompts.js'

// The scenario `intent` we ask the agent to record under. Carries the case id
// so the link-back poller can match the resulting scenario unambiguously.
export function runIntent(c: Pick<CaseRecord, 'title' | 'id'>): string {
  return _runIntent(c)
}

export function buildRunPrompt(c: CaseRecord, apiBase: string): string {
  return buildRunPromptText(c, apiBase)
}

export function buildRepairPrompt(
  c: CaseRecord,
  scenario: ScenarioSummary,
  apiBase: string
): string {
  return buildRepairPromptText(c, { sid: scenario.sid }, scenario.latestRun, apiBase)
}
