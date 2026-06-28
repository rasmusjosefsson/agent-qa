// web/src/features/cases/types.ts
// Mirrors the case/1 contract written by lib/report-server.js (handleCases).
import type { ScenarioSummary } from '@/features/runs/types'

export interface InputDecl {
  type?: string
  default?: unknown
  sensitive?: boolean
  description?: string
}

export interface CaseRecord {
  schema: 'case/1'
  id: string
  title: string
  startUrl: string
  preconditions: string
  steps: string[]
  expected: string
  inputs: Record<string, InputDecl>
  tags: string[]
  scenarioSid: string | null
  source: string
  sourceRef: string | null
  createdAt: number
  updatedAt: number
}

// A case joined to its linked scenario's last-run summary (null until recorded).
export interface CaseWithScenario extends CaseRecord {
  scenario: ScenarioSummary | null
}
