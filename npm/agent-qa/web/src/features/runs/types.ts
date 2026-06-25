// web/src/features/runs/types.ts
// Mirrors the read-only /api/scenarios/* contract from lib/report-server.js.

export interface RunSummary {
  runId: string
  summary: string | null
  exitCode: number | null
  startedAt: string | null
  finishedAt: string | null
  state: string | null // 'running' | 'done' | ...
  currentIdx?: number | null
  total?: number | null
  ok: boolean | null
  profile?: string | null
  tag?: string | null
}

export interface ScenarioSummary {
  sid: string
  dir: string
  scenarioId: string | null
  hasScenario: boolean
  intent: string | null
  steps: number | null
  latestRunId: string | null
  activeRunId: string | null
  latestRun: RunSummary | null
}

export interface ScenarioStep {
  id?: string
  verb?: string
  intent?: string
  on?: { role?: string; name?: string }
  value?: { literal?: unknown }
  [k: string]: unknown
}

export interface ScenarioDef {
  id?: string
  intent?: string
  steps?: ScenarioStep[]
  [k: string]: unknown
}

// A row in events.jsonl (collapsed by idx).
export interface RunEvent {
  idx: number
  id?: string
  intent?: string
  kind?: string
  status?: string // pass | fail | running | pending | ...
  ms?: number
  total?: number
  error?: string
  screenshot?: string
  snapshot?: string
  pending?: boolean
}

export interface RunDetail {
  sid: string
  runId: string
  isLatest: boolean
  audit: { summary?: string; exitCode?: number; [k: string]: unknown } | null
  status: { state?: string; currentIdx?: number; total?: number; ok?: boolean; [k: string]: unknown } | null
  events: RunEvent[]
}

export type DetailTab = 'step' | 'context' | 'network' | 'html' | 'console'

export interface Selection {
  sid: string | null
  runId: string | null
  stepIdx: number | null
  tab: DetailTab
}
