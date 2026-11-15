// The workbench's door to scenario and replay data.
//
// Two implementations: `httpSource` (the report server on 127.0.0.1:7878)
// and `fixtureSource` (a static tree). `VITE_AQA_SOURCE` selects one,
// default `http`. Every Runs read and write goes through this interface.
//
// `capabilities.writes` is the only switch for mutations. Fixture mode
// sets it false. Callers ask `sourceWrites` / `canWrite` — they do not
// branch on `kind`.
//
// `capabilities.server` is the local report server. The static demo sets
// it false. Callers ask `hasServer` — pages that need the CLI or `/api`
// stay disabled instead of calling the server.
//
// Later steps plug in here, not in the components:
//   capture   writes `scenarios.json`, `<sid>/runs.json`, and the replay tree
//   playback  reads heals, shot diffs, video cues, and network on `getRun`
//   play/     builds with `VITE_AQA_SOURCE=fixture`
//   banner    reads `canWrite()` and hides the write controls

import type {
  AuditTrend,
  CompareReport,
  RunDetail,
  RunEvent,
  RunSummary,
  ScenarioDef,
  ScenarioHealth,
  ScenarioSummary,
} from '@/features/runs/types'

export type SourceKind = 'http' | 'fixture'

/**
 * Positive capabilities.
 * `writes` false refuses every mutation.
 * `server` false means there is no local report server (the static demo).
 */
export interface SourceCapabilities {
  readonly writes: boolean
  readonly server: boolean
}

export interface ReplayOpts {
  headed?: boolean
  profile?: string
  params?: Record<string, string>
  personaId?: string
  environmentId?: string
}

export interface CrawlOpts {
  sid?: string
  max?: number
}

export interface StepDraftPos {
  after?: string
  at?: number
}

export interface OkResult {
  ok: boolean
  error?: string
}

export interface CrawlResult extends OkResult {
  stdout?: string
}

export interface MintResult extends OkResult {
  minted?: string[]
}

export interface CompareResult extends OkResult {
  report?: CompareReport
}

/** Mutations. Present on a source only when `capabilities.writes` is true. */
export interface ScenarioWrites {
  startReplay(sid: string, opts?: ReplayOpts): Promise<OkResult>
  crawlScenario(url: string, opts?: CrawlOpts): Promise<CrawlResult>
  deleteScenario(sid: string): Promise<OkResult>
  insertStep(sid: string, kind: 'do' | 'check', draft: Record<string, unknown>, pos?: StepDraftPos): Promise<OkResult>
  acceptShot(sid: string, runId: string, stepId: string): Promise<OkResult>
  promoteHeal(sid: string, runId: string, stepId: string): Promise<OkResult>
  acceptDomshot(sid: string, runId: string, stepId: string): Promise<MintResult>
  acceptAllDomshots(sid: string, runId: string): Promise<MintResult>
  acceptAllShots(sid: string, runId: string): Promise<MintResult>
  deleteRun(sid: string, runId: string): Promise<OkResult>
  compareRuns(sid: string, runA?: string, runB?: string): Promise<CompareResult>
}

export interface ScenarioSource {
  readonly kind: SourceKind
  readonly capabilities: SourceCapabilities
  /** Null when `capabilities.writes` is false. `sourceWrites` is the gate. */
  readonly writes: ScenarioWrites | null

  listScenarios(): Promise<{ scenariosRoot: string; scenarios: ScenarioSummary[] }>
  getHealth(): Promise<{ health: ScenarioHealth[] }>
  getScenario(sid: string): Promise<{ sid: string; scenario: ScenarioDef }>
  getTrend(sid: string, limit?: number): Promise<{ sid: string; trend: AuditTrend | null }>
  listRuns(sid: string): Promise<{ sid: string; replays: RunSummary[] }>
  getRun(sid: string, runId: string): Promise<RunDetail>

  /** `replays/<runId>/audit.json`. Null when the file is absent. */
  readAudit(sid: string, runId: string): Promise<RunDetail['audit']>
  /** Raw `replays/<runId>/events.jsonl`. Null when the file is absent. */
  readEvents(sid: string, runId: string): Promise<string | null>
  /** `replays/<runId>/status.json`. Null when the file is absent. */
  readStatus(sid: string, runId: string): Promise<RunDetail['status']>
  /** `replays/latest.txt`. Null when the pointer is absent. */
  readLatestRunId(sid: string): Promise<string | null>

  artifactUrl(sid: string, runId: string, kind: string, stepId: string): string
  screenshotUrl(sid: string, runId: string, stepId: string): string
  snapshotUrl(sid: string, runId: string, stepId: string): string
  baselineShotUrl(sid: string, name: string): string
  runFileUrl(sid: string, runId: string, name: string): string
  compareShotUrl(sid: string, folder: string, stepId: string): string
  /** Null when this source has no live browser (fixture mode). */
  replayStreamUrl(sid: string): string | null
  fetchArtifactText(sid: string, runId: string, kind: string, stepId: string, pretty: boolean): Promise<string | null>
}

/** Parsed JSONL. A trailing partial line is skipped. */
export function parseJsonl<T>(text: string | null): T[] {
  if (!text) return []
  const rows: T[] = []
  for (const line of text.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed) continue
    try {
      rows.push(JSON.parse(trimmed) as T)
    } catch {
      /* truncated line while a run is still writing */
    }
  }
  return rows
}

/** Parsed `events.jsonl`. A trailing partial line is skipped. */
export function parseEventsJsonl(text: string | null): RunEvent[] {
  return parseJsonl<RunEvent>(text)
}
