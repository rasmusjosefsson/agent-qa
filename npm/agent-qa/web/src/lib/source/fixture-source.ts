// Static replay tree. The capture step writes these files. getRun fills
// heals, shot diffs, video cues, and network when the files are present.
//
//   <base>/scenarios.json                         GET /api/scenarios body
//   <base>/catalog.json                           ids for cases, sets, and plans
//   <base>/_cases/<id>/case.json                  case/1
//   <base>/_sets/<id>/set.json                    set/1
//   <base>/_plans/<id>/plan.json                  plan/1
//   <base>/health.json                            GET /api/health body (optional)
//   <base>/<sid>/scenario.json                    raw scenario.json
//   <base>/<sid>/runs.json                        GET /api/scenarios/:sid/runs body
//   <base>/<sid>/audit-trend.json                 { sid, trend } (optional)
//   <base>/<sid>/replays/latest.txt
//   <base>/<sid>/replays/<runId>/audit.json
//   <base>/<sid>/replays/<runId>/events.jsonl
//   <base>/<sid>/replays/<runId>/status.json
//   <base>/<sid>/replays/<runId>/artifacts.json           heals, diffs, video cues
//   <base>/<sid>/replays/<runId>/network.json             optional
//   <base>/<sid>/replays/<runId>/heal.jsonl               used when artifacts.json is absent
//   <base>/<sid>/replays/<runId>/screenshots/<stepId>.png
//   <base>/<sid>/replays/<runId>/snapshots/<stepId>.txt
//   <base>/<sid>/baselines/<name>
//   <base>/<sid>/compare/<folder>/screenshots/<stepId>.diff.png
//
// Extensions match the report server's artifact kinds. A static host has
// no live browser, so replayStreamUrl is null and writes is null.

import type { CaseRecord, CaseWithScenario } from '@/features/cases/types'
import type { PlanRecord, PlanWithCount } from '@/features/plans/types'
import type { AuditTrend, HealRow, RunDetail, RunSummary, ScenarioDef, ScenarioHealth, ScenarioSummary } from '@/features/runs/types'
import type { SetRecord, SetWithCount } from '@/features/sets/types'
import { fetchJson, fetchOptional, readArtifactText } from './fetch-json'
import { parseEventsJsonl, parseJsonl, type ScenarioSource } from './types'

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

/** Directory listing a static host cannot produce. Capture writes this. */
export interface FixtureArtifacts {
  heals?: HealRow[]
  shotDiffs?: string[]
  domshotDiffs?: string[]
  video?: boolean
  videoFile?: 'run.mp4' | 'run.webm' | null
  cues?: RunDetail['cues']
}

export function runDetailFromFiles(
  sid: string,
  runId: string,
  files: {
    audit: RunDetail['audit']
    eventsJsonl: string | null
    status: RunDetail['status']
    latestRunId: string | null
    artifacts?: FixtureArtifacts | null
    network?: RunDetail['network']
  },
): RunDetail {
  const artifacts = files.artifacts
  return {
    sid,
    runId,
    isLatest: files.latestRunId === runId,
    audit: files.audit,
    status: files.status,
    events: parseEventsJsonl(files.eventsJsonl),
    heals: artifacts?.heals ?? [],
    shotDiffs: artifacts?.shotDiffs ?? [],
    domshotDiffs: artifacts?.domshotDiffs ?? [],
    video: artifacts?.video ?? false,
    videoFile: artifacts?.videoFile ?? null,
    cues: artifacts?.cues ?? null,
    network: files.network ?? null,
  }
}

interface FixtureCatalog {
  cases?: string[]
  sets?: string[]
  plans?: string[]
}

function isSafeId(id: string): boolean {
  return id !== '.' && id !== '..' && /^[A-Za-z0-9._-]+$/.test(id)
}

function sortedIds(ids: string[] | undefined): string[] {
  return [...(ids ?? [])].filter(isSafeId).sort()
}

/** Same membership as report-server.js `resolveSetCaseIds`. */
export function resolveSetCaseIds(
  set: Pick<SetRecord, 'mode' | 'caseIds' | 'tagQuery'>,
  cases: Pick<CaseRecord, 'id' | 'tags'>[],
): string[] {
  if (set.mode === 'tag') {
    const want = new Set(set.tagQuery)
    if (want.size === 0) return []
    return cases.filter((c) => c.tags.some((tag) => want.has(tag))).map((c) => c.id)
  }
  const byId = new Set(cases.map((c) => c.id))
  return set.caseIds.filter((id) => byId.has(id))
}

/** Same membership as report-server.js `resolvePlanCaseIds`. */
export function resolvePlanCaseIds(
  plan: Pick<PlanRecord, 'scope'>,
  sets: Array<Pick<SetRecord, 'id' | 'mode' | 'caseIds' | 'tagQuery'>>,
  cases: Array<Pick<CaseRecord, 'id' | 'tags'>>,
): string[] {
  const order: string[] = []
  const seen = new Set<string>()
  const add = (id: string) => {
    if (seen.has(id)) return
    seen.add(id)
    order.push(id)
  }
  const setById = new Map(sets.map((set) => [set.id, set]))
  for (const id of plan.scope.setIds) {
    const set = setById.get(id)
    if (set) for (const caseId of resolveSetCaseIds(set, cases)) add(caseId)
  }
  const byId = new Set(cases.map((c) => c.id))
  for (const id of plan.scope.caseIds) if (byId.has(id)) add(id)
  return order
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

  interface Library {
    cases: CaseWithScenario[]
    sets: SetWithCount[]
    plans: PlanWithCount[]
  }

  let library: Promise<Library> | null = null

  function loadLibrary(): Promise<Library> {
    if (!library) {
      library = readLibrary().catch((err) => {
        library = null
        throw err
      })
    }
    return library
  }

  async function readLibrary(): Promise<Library> {
    const [catalog, listed] = await Promise.all([
      readJson<FixtureCatalog>(url('catalog.json')),
      readJson<{ scenarios: ScenarioSummary[] }>(url('scenarios.json')),
    ])
    const scenarios = listed?.scenarios ?? []
    const scenarioById = new Map(scenarios.map((row) => [row.sid, row]))
    const cases: CaseWithScenario[] = []
    for (const id of sortedIds(catalog?.cases)) {
      const rec = await readJson<CaseRecord>(url('_cases', id, 'case.json'))
      if (!rec) continue
      const scenario = rec.scenarioSid ? scenarioById.get(rec.scenarioSid) ?? null : null
      cases.push({ ...rec, scenario })
    }
    const sets: SetWithCount[] = []
    for (const id of sortedIds(catalog?.sets)) {
      const rec = await readJson<SetRecord>(url('_sets', id, 'set.json'))
      if (!rec) continue
      sets.push({ ...rec, caseCount: resolveSetCaseIds(rec, cases).length })
    }
    const plans: PlanWithCount[] = []
    for (const id of sortedIds(catalog?.plans)) {
      const rec = await readJson<PlanRecord>(url('_plans', id, 'plan.json'))
      if (!rec) continue
      plans.push({ ...rec, caseCount: resolvePlanCaseIds(rec, sets, cases).length })
    }
    return { cases, sets, plans }
  }

  function missing(kind: string, id: string): Error {
    return new Error(`${url(kind, id)} → 404`)
  }

  const source: ScenarioSource = {
    kind: 'fixture',
    capabilities: { writes: false, server: false },
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
      const [audit, eventsJsonl, status, latestRunId, artifacts, network, healText] = await Promise.all([
        source.readAudit(sid, runId),
        source.readEvents(sid, runId),
        source.readStatus(sid, runId),
        source.readLatestRunId(sid),
        readJson<FixtureArtifacts>(url(sid, 'replays', runId, 'artifacts.json')),
        readJson<RunDetail['network']>(url(sid, 'replays', runId, 'network.json')),
        readText(url(sid, 'replays', runId, 'heal.jsonl')),
      ])
      // artifacts.json is the joined form (patches included). Without it,
      // heal.jsonl is joined one patch file at a time — a static host cannot
      // list diffs/.
      let heals = artifacts?.heals
      if (!heals) {
        const rows = parseJsonl<HealRow>(healText)
        heals = await Promise.all(
          rows.map(async (row) => {
            const stepId = typeof row.stepId === 'string' ? row.stepId : ''
            if (row.mode !== 'locator-correction' || !stepId) return { ...row, patch: row.patch ?? null }
            const patch = await readJson<NonNullable<HealRow['patch']>>(url(sid, 'replays', runId, 'diffs', `${stepId}.patch.json`))
            return { ...row, patch }
          })
        )
      }
      return runDetailFromFiles(sid, runId, {
        audit,
        eventsJsonl,
        status,
        latestRunId,
        artifacts: { ...artifacts, heals },
        network,
      })
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

    listCases() {
      return loadLibrary().then((lib) => ({ cases: lib.cases }))
    },

    async getCase(id) {
      const lib = await loadLibrary()
      const found = lib.cases.find((row) => row.id === id)
      if (!found) throw missing('_cases', id)
      return { case: found }
    },

    listSets() {
      return loadLibrary().then((lib) => ({ sets: lib.sets }))
    },

    async getSet(id) {
      const lib = await loadLibrary()
      const found = lib.sets.find((row) => row.id === id)
      if (!found) throw missing('_sets', id)
      return { set: found }
    },

    listPlans() {
      return loadLibrary().then((lib) => ({ plans: lib.plans }))
    },

    async getPlan(id) {
      const lib = await loadLibrary()
      const found = lib.plans.find((row) => row.id === id)
      if (!found) throw missing('_plans', id)
      return { plan: found }
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
