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
  // Steps the auto-heal loop recovered in this run (from audit.autoHealed).
  healed?: number | null
}

// One row of `audit health --json` — step ids flagged by each silent-
// degradation detector (flaky = outcome churn, slow = duration regression,
// chronic = self-healing locator debt).
export interface ScenarioHealth {
  scenarioId: string
  flaky: string[]
  slow: string[]
  chronic: string[]
}

export interface AuditTrendRun {
  runId: string
  exitCode: number | null
  durationSecs: number | null
  startedAt: string | null
}

// `audit trend` rollup: the scenario's last N runs as outcome glyphs +
// a duration sparkline, for the "is this degrading" glance in the header.
export interface AuditTrend {
  scenarioId: string
  runs: AuditTrendRun[]
  passed: number
  failed: number
  medianSecs: number
  outcomes: string
  sparkline: string
}

export interface ScenarioSummary {
  sid: string
  dir: string
  scenarioId: string | null
  hasScenario: boolean
  intent: string | null
  steps: number | null
  // do→check coverage (same heuristic as `scenario coverage`): null when no
  // scenario.json exists yet.
  coverage: {
    doSteps: number
    checked: number
    bare: number
    shotCovered: number
    ratio: number
    shotRatio: number
  } | null
  // scenario.json's tags[] — `replay --tags` selects on these.
  tags: string[]

  latestRunId: string | null
  activeRunId: string | null
  latestRun: RunSummary | null
}

export interface ScenarioStep {
  id?: string
  verb?: string
  intent?: string
  on?: { role?: string; name?: string; raw?: { kind?: string; value?: string }; reason?: string }
  value?: { literal?: unknown }
  [k: string]: unknown
}

export interface ScenarioDef {
  id?: string
  intent?: string
  steps?: ScenarioStep[]
  // The recorded setup baseline. A `useProfile` op names the login the run must
  // sign in as — used to default the "Replay as" persona.
  env?: { open?: Array<{ kind?: string; name?: string }>; [k: string]: unknown }
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

// One row of a run's heal.jsonl — a locator correction the auto-heal loop
// applied, or a classified value rejection it refused to retry. `patch` is the
// parsed diffs/<stepId>.patch.json for corrections (null otherwise).
export interface HealRow {
  stepId?: string
  mode?: string // 'locator-correction' | 'value-rejection'
  strategy?: string
  from?: string
  to?: string
  rationale?: string
  ts?: string
  patch?: {
    newLocator?: unknown
    rationale?: string
    [k: string]: unknown
  } | null
  [k: string]: unknown
}

export interface RunDetail {
  sid: string
  runId: string
  isLatest: boolean
  audit: { summary?: string; exitCode?: number; autoHealed?: string[]; [k: string]: unknown } | null
  status: { state?: string; currentIdx?: number; total?: number; ok?: boolean; [k: string]: unknown } | null
  events: RunEvent[]
  heals?: HealRow[]
  // StepIds whose {"shot"} claim missed the baseline this run — each has a
  // shots-diff/<stepId>.diff.png delta map servable via artifactUrl.
  shotDiffs?: string[]
  // run.webm exists in the run dir (replay ran with --record-video) —
  // servable via runFileUrl(sid, runId, 'run.webm').
  video?: boolean
  // The run's captured request log (network.json): every fetch/XHR/etc the
  // browser made during replay. Absent on runs that predate the artifact.
  network?: { requestCount?: number; requests?: RunNetworkRequest[] } | null
}
export interface RunNetworkRequest {
  requestId: string
  url: string
  method: string
  status?: number
  resourceType?: string
  mimeType?: string
}

export type DetailTab = 'step' | 'scenario' | 'context' | 'network' | 'html' | 'console'

// Compare report returned by POST /api/scenarios/:sid/compare — mirrors the
// `agent-qa compare` CLI output (compare.md + per-step diff files).
export interface CompareEntry {
  stepId: string
  outcome: 'SAME' | 'CHANGED' | 'ONLY-A' | 'ONLY-B' | string
  diff?: string | null
}

export interface CompareShot {
  stepId: string
  outcome: string
  differingPixels: number | null
  hasDiffPng: boolean
}

export interface CompareNetEntry {
  request: string
  outcome: 'SAME' | 'CHANGED' | 'ONLY-A' | 'ONLY-B' | string
  statusA: string
  statusB: string
}

export interface CompareReport {
  sid: string
  folder: string
  runA: string | null
  runB: string | null
  snapshots: CompareEntry[]
  screenshots: CompareShot[]
  network: CompareNetEntry[]
}

export interface Selection {
  sid: string | null
  runId: string | null
  stepIdx: number | null
  tab: DetailTab
}
