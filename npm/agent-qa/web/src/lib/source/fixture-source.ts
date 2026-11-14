// Static replay tree. The capture step writes these files; playback fills
// the run fields this stub leaves empty (heals, diffs, video, network).
//
//   <base>/scenarios.json                         GET /api/scenarios body
//   <base>/health.json                            GET /api/health body (optional)
//   <base>/<sid>/scenario.json                    raw scenario.json
//   <base>/<sid>/runs.json                        GET /api/scenarios/:sid/runs body
//   <base>/<sid>/audit-trend.json                 { sid, trend } (optional)
//   <base>/<sid>/replays/latest.txt
//   <base>/<sid>/replays/<runId>/audit.json
//   <base>/<sid>/replays/<runId>/events.jsonl
//   <base>/<sid>/replays/<runId>/status.json
//   <base>/<sid>/replays/<runId>/screenshots/<stepId>.png
//   <base>/<sid>/replays/<runId>/snapshots/<stepId>.txt
//   <base>/<sid>/baselines/<name>
//   <base>/<sid>/compare/<folder>/screenshots/<stepId>.diff.png
//
// Extensions match the report server's artifact kinds. A static host has
// no live browser, so replayStreamUrl is null and writes is null.

import type { AuditTrend, RunDetail, RunSummary, ScenarioDef, ScenarioHealth, ScenarioSummary } from '@/features/runs/types'
import { fetchJson, fetchOptional, readArtifactText } from './fetch-json'
import { parseEventsJsonl, type ScenarioSource } from './types'

export const DEFAULT_FIXTURE_BASE = '/fixtures'

/** Sidecar extensions. Same map as ARTIFACT_KINDS in report-server.js. */
export const ARTIFACT_EXT: Record<string, string> = {
  screenshots: '.png',
  snapshots: '.txt',
  'shots-diff': '.diff.png',
  'domshots-diff': '.diff.txt',
  network: '.json',
  probes: '.json',
  perf: '.json',
}

export function normalizeFixtureBase(raw: string | undefined): string {
  const trimmed = (raw ?? '').trim().replace(/\/+$/, '')
  return trimmed === '' ? DEFAULT_FIXTURE_BASE : trimmed
}

export function fixtureUrl(base: string, ...segments: string[]): string {
  const root = normalizeFixtureBase(base)
  return `${root}/${segments.map((segment) => encodeURIComponent(segment)).join('/')}`
}

export function runDetailFromFiles(
  sid: string,
  runId: string,
  files: {
    audit: RunDetail['audit']
    eventsJsonl: string | null
    status: RunDetail['status']
    latestRunId: string | null
  },
): RunDetail {
  return {
    sid,
    runId,
    isLatest: files.latestRunId === runId,
    audit: files.audit,
    status: files.status,
    events: parseEventsJsonl(files.eventsJsonl),
    // Playback (a later step) fills these from heal.jsonl, diff dirs, and video.
    heals: [],
    shotDiffs: [],
    domshotDiffs: [],
    video: false,
    videoFile: null,
    cues: null,
    network: null,
  }
}

type SourceFetch = typeof fetch

export function createFixtureSource(base?: string, doFetch?: SourceFetch): ScenarioSource {
  const root = normalizeFixtureBase(base)
  const request = doFetch ?? fetch
  const url = (...segments: string[]) => fixtureUrl(root, ...segments)

  async function readText(path: string): Promise<string | null> {
    const res = await fetchOptional(path, request)
    if (!res) return null
    return res.text()
  }

  async function readJson<T>(path: string): Promise<T | null> {
    const res = await fetchOptional(path, request)
    if (!res) return null
    return (await res.json()) as T
  }

  const source: ScenarioSource = {
    kind: 'fixture',
    capabilities: { writes: false },
    writes: null,

    listScenarios() {
      return fetchJson<{ scenariosRoot: string; scenarios: ScenarioSummary[] }>(url('scenarios.json'), request)
    },

    async getHealth() {
      const body = await readJson<{ health: ScenarioHealth[] }>(url('health.json'))
      return { health: body?.health ?? [] }
    },

    async getScenario(sid) {
      const scenario = await fetchJson<ScenarioDef>(url(sid, 'scenario.json'), request)
      return { sid, scenario }
    },

    async getTrend(sid, limit = 20) {
      // The captured file is already the window the demo shows. `limit` is
      // accepted so the signature matches http; playback may slice later.
      void limit
      const body = await readJson<{ sid: string; trend: AuditTrend | null }>(url(sid, 'audit-trend.json'))
      if (!body) return { sid, trend: null }
      return { sid: body.sid || sid, trend: body.trend ?? null }
    },

    listRuns(sid) {
      return fetchJson<{ sid: string; replays: RunSummary[] }>(url(sid, 'runs.json'), request)
    },

    async getRun(sid, runId) {
      const [audit, eventsJsonl, status, latestRunId] = await Promise.all([
        source.readAudit(sid, runId),
        source.readEvents(sid, runId),
        source.readStatus(sid, runId),
        source.readLatestRunId(sid),
      ])
      return runDetailFromFiles(sid, runId, { audit, eventsJsonl, status, latestRunId })
    },

    readAudit(sid, runId) {
      return readJson<RunDetail['audit']>(url(sid, 'replays', runId, 'audit.json'))
    },

    readEvents(sid, runId) {
      return readText(url(sid, 'replays', runId, 'events.jsonl'))
    },

    readStatus(sid, runId) {
      return readJson<RunDetail['status']>(url(sid, 'replays', runId, 'status.json'))
    },

    async readLatestRunId(sid) {
      const text = await readText(url(sid, 'replays', 'latest.txt'))
      const id = text?.trim()
      return id ? id : null
    },

    artifactUrl(sid, runId, kind, stepId) {
      const ext = ARTIFACT_EXT[kind] ?? ''
      return url(sid, 'replays', runId, kind, `${stepId}${ext}`)
    },

    screenshotUrl(sid, runId, stepId) {
      return url(sid, 'replays', runId, 'screenshots', `${stepId}.png`)
    },

    snapshotUrl(sid, runId, stepId) {
      return url(sid, 'replays', runId, 'snapshots', `${stepId}.txt`)
    },

    baselineShotUrl(sid, name) {
      return url(sid, 'baselines', name)
    },

    runFileUrl(sid, runId, name) {
      return url(sid, 'replays', runId, name)
    },

    compareShotUrl(sid, folder, stepId) {
      return url(sid, 'compare', folder, 'screenshots', `${stepId}.diff.png`)
    },

    replayStreamUrl() {
      return null
    },

    fetchArtifactText(sid, runId, kind, stepId, pretty) {
      return readArtifactText(source.artifactUrl(sid, runId, kind, stepId), pretty, request)
    },
  }

  return source
}
