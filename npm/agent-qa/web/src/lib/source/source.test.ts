import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  acceptAllDomshots,
  acceptAllShots,
  acceptDomshot,
  acceptShot,
  artifactUrl,
  baselineShotUrl,
  compareRuns,
  compareShotUrl,
  crawlScenario,
  deleteRun,
  deleteScenario,
  fetchArtifactText,
  getRunDetail,
  getScenarios,
  insertStep,
  promoteHeal,
  READ_ONLY_ERROR,
  replayStreamUrl,
  runFileUrl,
  startReplay,
} from '../runs-api'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { createFixtureSource, resolvePlanCaseIds, resolveSetCaseIds, runDetailFromFiles } from './fixture-source'
import { getPlans } from '../plans-api'
import { getSets } from '../sets-api'
import { httpSource } from './http-source'
import {
  canWrite,
  createScenarioSource,
  hasServer,
  LOCAL_ONLY_NOTE,
  resolveSourceKind,
  routeNeedsServer,
  setScenarioSourceForTests,
  sourceWrites,
} from './select'
import type { ScenarioSource, ScenarioWrites } from './types'

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

function textResponse(body: string, status = 200): Response {
  return new Response(body, { status, headers: { 'content-type': 'text/plain' } })
}

describe('source selection', () => {
  afterEach(() => {
    vi.unstubAllEnvs()
    setScenarioSourceForTests(null)
  })

  it('defaults to the http source, which can write', () => {
    expect(resolveSourceKind(undefined)).toBe('http')
    expect(resolveSourceKind('')).toBe('http')
    expect(resolveSourceKind('  http  ')).toBe('http')
    const source = createScenarioSource()
    expect(source.kind).toBe('http')
    expect(source).toBe(httpSource)
    expect(source.capabilities.writes).toBe(true)
    expect(source.capabilities.server).toBe(true)
    expect(canWrite(source)).toBe(true)
    expect(hasServer(source)).toBe(true)
    expect(sourceWrites(source)).toBe(httpSource.writes)
  })

  it('selects the fixture source from VITE_AQA_SOURCE', () => {
    vi.stubEnv('VITE_AQA_SOURCE', 'fixture')
    vi.stubEnv('VITE_AQA_FIXTURE_BASE', '/play/fixtures/')
    const source = createScenarioSource()
    expect(source.kind).toBe('fixture')
    expect(source.capabilities.writes).toBe(false)
    expect(source.capabilities.server).toBe(false)
    expect(canWrite(source)).toBe(false)
    expect(hasServer(source)).toBe(false)
    expect(LOCAL_ONLY_NOTE).toMatch(/run agent-qa locally/)
    expect(routeNeedsServer('/settings')).toBe(true)
    expect(routeNeedsServer('/chat')).toBe(true)
    expect(routeNeedsServer('/')).toBe(false)
    expect(routeNeedsServer('/cases')).toBe(false)
    expect(routeNeedsServer('/sets')).toBe(false)
    expect(routeNeedsServer('/plans?id=release-check')).toBe(false)
    expect(sourceWrites(source)).toBeNull()
    expect(source.screenshotUrl('s-1', 'r-1', 'open')).toBe('/play/fixtures/s-1/replays/r-1/screenshots/open.png')
  })

  it('lets an explicit kind override the env', () => {
    vi.stubEnv('VITE_AQA_SOURCE', 'fixture')
    expect(createScenarioSource({ kind: 'http' }).kind).toBe('http')
  })

  it('rejects an unknown VITE_AQA_SOURCE', () => {
    expect(() => resolveSourceKind('disk')).toThrow(/VITE_AQA_SOURCE/)
    expect(() => createScenarioSource({ kind: 'disk' })).toThrow(/"disk"/)
  })

  it('treats the writes flag as the gate, not the source kind', () => {
    const writes = {} as ScenarioWrites
    const blocked = { kind: 'http', capabilities: { writes: false, server: true }, writes } as unknown as ScenarioSource
    const opened = { kind: 'fixture', capabilities: { writes: true, server: false }, writes } as unknown as ScenarioSource
    expect(sourceWrites(blocked)).toBeNull()
    expect(canWrite(blocked)).toBe(false)
    expect(sourceWrites(opened)).toBe(writes)
    expect(canWrite(opened)).toBe(true)
  })
})

describe('fixture source', () => {
  afterEach(() => setScenarioSourceForTests(null))

  it('reads the replay tree from the static base and skips a partial jsonl line', async () => {
    const urls: string[] = []
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const path = String(input)
      urls.push(path)
      if (path.endsWith('/scenarios.json')) {
        return jsonResponse({ scenariosRoot: 'fixtures', scenarios: [{ sid: 's-1' }] })
      }
      if (path.endsWith('/health.json')) return new Response('no', { status: 404 })
      if (path.endsWith('/s-1/scenario.json')) return jsonResponse({ id: 's-1', intent: 'Open the page' })
      if (path.endsWith('/s-1/runs.json')) return jsonResponse({ sid: 's-1', replays: [{ runId: 'r-1' }] })
      if (path.endsWith('/s-1/audit-trend.json')) return new Response('no', { status: 404 })
      if (path.endsWith('/audit.json')) return jsonResponse({ summary: 'SUMMARY: 1/1 (PASS)', exitCode: 0 })
      if (path.endsWith('/events.jsonl')) {
        return textResponse('{"idx":1,"status":"running"}\n{"idx":1,"status":"pass"}\n{"idx":\n')
      }
      if (path.endsWith('/status.json')) return jsonResponse({ state: 'done', ok: true })
      if (path.endsWith('/latest.txt')) return textResponse('r-1\n')
      if (path.endsWith('/snapshots/open.txt')) return textResponse('button "Save"')
      return new Response('missing', { status: 404 })
    })

    const source = createFixtureSource('/demo/', fetchMock)
    expect(source.capabilities.writes).toBe(false)
    expect(await source.listScenarios()).toMatchObject({ scenariosRoot: 'fixtures' })
    expect(await source.getHealth()).toEqual({ health: [] })
    expect(await source.getScenario('s-1')).toMatchObject({ sid: 's-1', scenario: { intent: 'Open the page' } })
    expect(await source.getTrend('s-1')).toEqual({ sid: 's-1', trend: null })
    expect((await source.listRuns('s-1')).replays).toEqual([{ runId: 'r-1' }])

    const detail = await source.getRun('s-1', 'r-1')
    expect(detail.isLatest).toBe(true)
    expect(detail.audit).toMatchObject({ exitCode: 0 })
    expect(detail.status).toMatchObject({ state: 'done' })
    expect(detail.events).toEqual([
      { idx: 1, status: 'running' },
      { idx: 1, status: 'pass' },
    ])
    expect(detail.heals).toEqual([])
    expect(detail.shotDiffs).toEqual([])
    expect(detail.video).toBe(false)
    expect(detail.network).toBeNull()

    expect(urls).toContain('/demo/s-1/replays/r-1/audit.json')
    expect(urls).toContain('/demo/s-1/replays/r-1/events.jsonl')
    expect(urls).toContain('/demo/s-1/replays/r-1/status.json')
    expect(urls).toContain('/demo/s-1/replays/latest.txt')

    expect(source.screenshotUrl('s-1', 'r-1', 'open')).toBe('/demo/s-1/replays/r-1/screenshots/open.png')
    expect(source.snapshotUrl('s-1', 'r-1', 'open')).toBe('/demo/s-1/replays/r-1/snapshots/open.txt')
    expect(source.artifactUrl('s-1', 'r-1', 'snapshots', 'open')).toBe(source.snapshotUrl('s-1', 'r-1', 'open'))
    expect(source.replayStreamUrl('s-1')).toBeNull()
    expect(await source.fetchArtifactText('s-1', 'r-1', 'snapshots', 'open', false)).toBe('button "Save"')
  })

  it('fills heals, diffs, cues, and network when the files exist', async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const path = String(input)
      if (path.endsWith('/audit.json')) return jsonResponse({ summary: 'SUMMARY: 1/2 (FAIL)', exitCode: 1 })
      if (path.endsWith('/events.jsonl')) return textResponse('{"idx":1,"id":"open","status":"pass","ms":100}\n')
      if (path.endsWith('/status.json')) return jsonResponse({ state: 'done', ok: false })
      if (path.endsWith('/latest.txt')) return textResponse('r-1\n')
      if (path.endsWith('/artifacts.json')) {
        return jsonResponse({
          heals: [{ stepId: 'open', mode: 'locator-correction', strategy: 'name', patch: { rationale: 'renamed' } }],
          shotDiffs: ['open'],
          domshotDiffs: ['open'],
          video: true,
          videoFile: 'run.mp4',
          cues: [{ id: 'open', idx: 1, tMs: 400 }],
        })
      }
      if (path.endsWith('/network.json')) {
        return jsonResponse({ requestCount: 1, requests: [{ requestId: '1', url: 'https://example.com/', method: 'GET', status: 200 }] })
      }
      return new Response('missing', { status: 404 })
    })
    const source = createFixtureSource('/demo', fetchMock)
    const detail = await source.getRun('s-1', 'r-1')
    expect(detail.heals).toEqual([
      { stepId: 'open', mode: 'locator-correction', strategy: 'name', patch: { rationale: 'renamed' } },
    ])
    expect(detail.shotDiffs).toEqual(['open'])
    expect(detail.domshotDiffs).toEqual(['open'])
    expect(detail.video).toBe(true)
    expect(detail.videoFile).toBe('run.mp4')
    expect(detail.cues).toEqual([{ id: 'open', idx: 1, tMs: 400 }])
    expect(detail.network).toMatchObject({ requestCount: 1 })
  })

  it('joins heal.jsonl with patch files when artifacts.json is absent', async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const path = String(input)
      if (path.endsWith('/audit.json')) return jsonResponse(null)
      if (path.endsWith('/events.jsonl')) return new Response('missing', { status: 404 })
      if (path.endsWith('/status.json')) return new Response('missing', { status: 404 })
      if (path.endsWith('/latest.txt')) return new Response('missing', { status: 404 })
      if (path.endsWith('/artifacts.json')) return new Response('missing', { status: 404 })
      if (path.endsWith('/network.json')) return new Response('missing', { status: 404 })
      if (path.endsWith('/heal.jsonl')) {
        return textResponse(
          '{"stepId":"open","mode":"locator-correction","strategy":"name"}\n{"stepId":"save","mode":"value-rejection"}\n'
        )
      }
      if (path.endsWith('/diffs/open.patch.json')) return jsonResponse({ rationale: 'role matched' })
      return new Response('missing', { status: 404 })
    })
    const source = createFixtureSource('/demo', fetchMock)
    const detail = await source.getRun('s-1', 'r-1')
    expect(detail.heals).toEqual([
      { stepId: 'open', mode: 'locator-correction', strategy: 'name', patch: { rationale: 'role matched' } },
      { stepId: 'save', mode: 'value-rejection', patch: null },
    ])
    expect(detail.network).toBeNull()
  })

  it('refuses every write without fetching', async () => {
    const fetchMock = vi.fn()
    setScenarioSourceForTests(createFixtureSource('/demo', fetchMock))

    const results = await Promise.all([
      startReplay('s-1', { headed: true }),
      crawlScenario('https://example.com'),
      deleteScenario('s-1'),
      insertStep('s-1', 'check', { intent: 'saved' }),
      acceptShot('s-1', 'r-1', 'open'),
      promoteHeal('s-1', 'r-1', 'open'),
      acceptDomshot('s-1', 'r-1', 'open'),
      acceptAllDomshots('s-1', 'r-1'),
      acceptAllShots('s-1', 'r-1'),
      deleteRun('s-1', 'r-1'),
      compareRuns('s-1', 'r-0', 'r-1'),
    ])

    expect(results.every((result) => result.ok === false && result.error === READ_ONLY_ERROR)).toBe(true)
    expect(fetchMock).not.toHaveBeenCalled()
    expect(canWrite()).toBe(false)
  })

  it('resolves the example set and plan without calling /api', async () => {
    const root = resolve(__dirname, '../../../fixtures')
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const rel = String(input).replace(/^\/demo\//, '')
      try {
        return textResponse(readFileSync(resolve(root, rel), 'utf8'))
      } catch {
        return new Response('missing', { status: 404 })
      }
    })
    const source = createFixtureSource('/demo', fetchMock)
    const sets = await source.listSets()
    expect(sets.sets.map((set) => [set.id, set.name, set.caseCount])).toEqual([['smoke', 'Smoke', 2]])
    const plans = await source.listPlans()
    expect(plans.plans.map((plan) => [plan.id, plan.name, plan.caseCount])).toEqual([
      ['release-check', 'Release check', 3],
    ])
    const cases = await source.listCases()
    expect(cases.cases.map((row) => row.id)).toEqual(['accessibility-gate', 'device-emulation', 'open-homepage'])
    expect(cases.cases.every((row) => row.scenario === null)).toBe(true)
    const plan = await source.getPlan('release-check')
    expect(plan.plan.scope).toEqual({ setIds: ['smoke'], caseIds: ['device-emulation'] })
    await expect(source.getSet('missing')).rejects.toThrow(/404/)
    expect(fetchMock.mock.calls.every((call) => !String(call[0]).includes('/api/'))).toBe(true)
  })

  it('unions a tag set and a direct case in case-list order', () => {
    const cases = [
      { id: 'accessibility-gate', title: 'A', tags: ['smoke', 'a11y'], scenarioSid: 'a11y' },
      { id: 'device-emulation', title: 'D', tags: ['emulation'], scenarioSid: 'emulate' },
      { id: 'open-homepage', title: 'O', tags: ['smoke'], scenarioSid: 'smoke' },
    ]
    const smoke = { id: 'smoke', name: 'Smoke', mode: 'tag' as const, caseIds: [], tagQuery: ['smoke'] }
    expect(resolveSetCaseIds(smoke, cases)).toEqual(['accessibility-gate', 'open-homepage'])
    expect(
      resolvePlanCaseIds({ scope: { setIds: ['smoke'], caseIds: ['device-emulation', 'open-homepage'] } }, [smoke], cases),
    ).toEqual(['accessibility-gate', 'open-homepage', 'device-emulation'])
  })
})

describe('http source parity', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    setScenarioSourceForTests(null)
  })

  it('keeps the report-server paths and the replay body', async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input)
      if (init?.method === 'POST') return jsonResponse({ ok: true }, 202)
      if (path === '/api/scenarios') return jsonResponse({ scenariosRoot: '/tmp', scenarios: [] })
      if (path.endsWith('/runs/r-1')) {
        return jsonResponse({
          sid: 's-1',
          runId: 'r-1',
          isLatest: true,
          audit: { summary: 'PASS' },
          status: { state: 'done' },
          events: [{ idx: 0, status: 'pass' }],
        })
      }
      return textResponse('{"ok":true}')
    })
    vi.stubGlobal('fetch', fetchMock)
    setScenarioSourceForTests(httpSource)

    expect(await getScenarios()).toMatchObject({ scenariosRoot: '/tmp' })
    expect(artifactUrl('s-1', 'r-1', 'screenshots', 'open')).toBe('/api/scenarios/s-1/runs/r-1/artifact/screenshots/open')
    expect(baselineShotUrl('s-1', 'open.png')).toBe('/api/scenarios/s-1/baselines/open.png')
    expect(runFileUrl('s-1', 'r-1', 'run.mp4')).toBe('/api/scenarios/s-1/runs/r-1/file/run.mp4')
    expect(compareShotUrl('s-1', 'fold', 'open')).toBe('/api/scenarios/s-1/compare/fold/shots/open')
    expect(replayStreamUrl('s-1')).toBe('/api/scenarios/s-1/replay-stream')
    expect(httpSource.screenshotUrl('s-1', 'r-1', 'open')).toBe(artifactUrl('s-1', 'r-1', 'screenshots', 'open'))
    expect(httpSource.snapshotUrl('s-1', 'r-1', 'open')).toBe('/api/scenarios/s-1/runs/r-1/artifact/snapshots/open')

    const detail = await getRunDetail('s-1', 'r-1')
    expect(detail.events).toEqual([{ idx: 0, status: 'pass' }])
    expect(await httpSource.readEvents('s-1', 'r-1')).toBe('{"idx":0,"status":"pass"}')

    const started = await startReplay('s-1', { headed: true })
    expect(started).toEqual({ ok: true })
    const post = fetchMock.mock.calls.find((call) => (call[1] as RequestInit | undefined)?.method === 'POST')
    expect(String(post?.[0])).toBe('/api/scenarios/s-1/replay')
    expect(JSON.parse(String((post?.[1] as RequestInit).body))).toEqual({ headed: true })

    expect(await fetchArtifactText('s-1', 'r-1', 'probes', 'open', true)).toBe('{\n  "ok": true\n}')
    expect(canWrite()).toBe(true)

    expect(await getSets()).toEqual({ ok: true })
    expect(await getPlans()).toEqual({ ok: true })
    expect(fetchMock.mock.calls.map((call) => String(call[0]))).toEqual(
      expect.arrayContaining(['/api/sets', '/api/plans']),
    )
  })
})

describe('runDetailFromFiles', () => {
  it('marks the run latest only when latest.txt names it', () => {
    const detail = runDetailFromFiles('s', 'r', {
      audit: null,
      eventsJsonl: null,
      status: null,
      latestRunId: 'other',
    })
    expect(detail.isLatest).toBe(false)
    expect(detail.events).toEqual([])
  })
})
