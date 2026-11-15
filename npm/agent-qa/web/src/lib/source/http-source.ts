// Report-server client. Request paths and bodies match the previous
// runs-api wrappers so http mode is unchanged.

import type {
  AuditTrend,
  CompareReport,
  RunDetail,
  RunSummary,
  ScenarioDef,
  ScenarioHealth,
  ScenarioSummary,
} from '@/features/runs/types'
import { fetchJson, readArtifactText } from './fetch-json'
import type { ScenarioSource, ScenarioWrites } from './types'

function sidPath(sid: string): string {
  return `/api/scenarios/${encodeURIComponent(sid)}`
}

function runPath(sid: string, runId: string): string {
  return `${sidPath(sid)}/runs/${encodeURIComponent(runId)}`
}

async function readDetail(sid: string, runId: string): Promise<RunDetail> {
  return fetchJson<RunDetail>(runPath(sid, runId))
}

const writes: ScenarioWrites = {
  async startReplay(sid, opts) {
    const res = await fetch(`${sidPath(sid)}/replay`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(opts ?? {}),
    })
    // A named persona whose credential refs can't be resolved comes back 200 { ok:false }.
    if (res.ok) {
      const j = (await res.json().catch(() => ({}))) as { ok?: boolean; error?: string }
      if (j.ok === false) return { ok: false, error: j.error || 'replay refused' }
      return { ok: true }
    }
    const j = (await res.json().catch(() => ({}))) as { error?: string }
    return { ok: false, error: j.error || String(res.status) }
  },

  async crawlScenario(url, opts) {
    const res = await fetch('/api/scenarios/crawl', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ url, ...(opts?.sid ? { sid: opts.sid } : {}), ...(opts?.max ? { max: opts.max } : {}) }),
    })
    const j = (await res.json().catch(() => ({}))) as { stdout?: string; error?: string }
    if (res.ok) return { ok: true, stdout: j.stdout }
    return { ok: false, error: j.error || String(res.status) }
  },

  async deleteScenario(sid) {
    const res = await fetch(`${sidPath(sid)}/delete`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: '{}',
    })
    if (res.ok) return { ok: true }
    const j = (await res.json().catch(() => ({}))) as { error?: string }
    return { ok: false, error: j.error || String(res.status) }
  },

  // Splice a step into scenario.json (`scenario insert`). `after` is a step id.
  async insertStep(sid, kind, draft, pos) {
    const res = await fetch(`${sidPath(sid)}/insert-step`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ kind, draft, ...(pos || {}) }),
    })
    const j = (await res.json().catch(() => ({}))) as { ok?: boolean; error?: string }
    if (res.ok && j.ok !== false) return { ok: true }
    return { ok: false, error: j.error || `insert failed (${res.status})` }
  },

  async acceptShot(sid, runId, stepId) {
    const res = await fetch(`${runPath(sid, runId)}/shot-accept`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ stepId }),
    })
    if (res.ok) return { ok: true }
    const j = (await res.json().catch(() => ({}))) as { error?: string }
    return { ok: false, error: j.error || String(res.status) }
  },

  // 409 means the rebase guard fired (scenario.json drifted since the patch).
  async promoteHeal(sid, runId, stepId) {
    const res = await fetch(`${runPath(sid, runId)}/heal-promote`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ stepId }),
    })
    if (res.ok) return { ok: true }
    const j = (await res.json().catch(() => ({}))) as { error?: string }
    return { ok: false, error: j.error || String(res.status) }
  },

  async acceptDomshot(sid, runId, stepId) {
    const res = await fetch(`${runPath(sid, runId)}/domshot-accept`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ stepId }),
    })
    const j = (await res.json().catch(() => ({}))) as { ok?: boolean; minted?: string[]; error?: string }
    if (res.ok && j.ok) return { ok: true, minted: j.minted || [] }
    return { ok: false, error: j.error || String(res.status) }
  },

  async acceptAllDomshots(sid, runId) {
    const res = await fetch(`${runPath(sid, runId)}/domshot-accept`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ all: true }),
    })
    const j = (await res.json().catch(() => ({}))) as { ok?: boolean; minted?: string[]; error?: string }
    if (res.ok && j.ok) return { ok: true, minted: j.minted || [] }
    return { ok: false, error: j.error || String(res.status) }
  },

  async acceptAllShots(sid, runId) {
    const res = await fetch(`${runPath(sid, runId)}/shot-accept`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ all: true }),
    })
    const j = (await res.json().catch(() => ({}))) as { ok?: boolean; minted?: string[]; error?: string }
    if (res.ok && j.ok) return { ok: true, minted: j.minted || [] }
    return { ok: false, error: j.error || String(res.status) }
  },

  async deleteRun(sid, runId) {
    const res = await fetch(`${runPath(sid, runId)}/delete`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: '{}',
    })
    if (res.ok) return { ok: true }
    const j = (await res.json().catch(() => ({}))) as { error?: string }
    return { ok: false, error: j.error || String(res.status) }
  },

  async compareRuns(sid, runA, runB) {
    const res = await fetch(`${sidPath(sid)}/compare`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ ...(runA ? { runA } : {}), ...(runB ? { runB } : {}) }),
    })
    const j = (await res.json().catch(() => ({}))) as CompareReport & { error?: string }
    if (!res.ok) return { ok: false, error: j.error || String(res.status) }
    return { ok: true, report: j }
  },
}

export const httpSource: ScenarioSource = {
  kind: 'http',
  capabilities: { writes: true, server: true },
  writes,

  listScenarios() {
    return fetchJson<{ scenariosRoot: string; scenarios: ScenarioSummary[] }>('/api/scenarios')
  },

  getHealth() {
    return fetchJson<{ health: ScenarioHealth[] }>('/api/health')
  },

  getScenario(sid) {
    return fetchJson<{ sid: string; scenario: ScenarioDef }>(`${sidPath(sid)}/scenario`)
  },

  getTrend(sid, limit = 20) {
    return fetchJson<{ sid: string; trend: AuditTrend | null }>(`${sidPath(sid)}/audit/trend?limit=${limit}`)
  },

  listRuns(sid) {
    return fetchJson<{ sid: string; replays: RunSummary[] }>(`${sidPath(sid)}/runs`)
  },

  getRun(sid, runId) {
    return readDetail(sid, runId)
  },

  async readAudit(sid, runId) {
    return (await readDetail(sid, runId)).audit
  },

  async readEvents(sid, runId) {
    // The report server returns parsed rows, not the original jsonl bytes.
    const events = (await readDetail(sid, runId)).events ?? []
    if (events.length === 0) return null
    return events.map((row) => JSON.stringify(row)).join('\n')
  },

  async readStatus(sid, runId) {
    return (await readDetail(sid, runId)).status
  },

  async readLatestRunId(sid) {
    const data = await fetchJson<{ scenarios: { sid: string; latestRunId: string | null }[] }>('/api/scenarios')
    const row = (data.scenarios || []).find((s) => s.sid === sid)
    return row?.latestRunId ?? null
  },

  artifactUrl(sid, runId, kind, stepId) {
    return httpArtifactUrl(sid, runId, kind, stepId)
  },

  screenshotUrl(sid, runId, stepId) {
    return httpArtifactUrl(sid, runId, 'screenshots', stepId)
  },

  snapshotUrl(sid, runId, stepId) {
    return httpArtifactUrl(sid, runId, 'snapshots', stepId)
  },

  baselineShotUrl(sid, name) {
    return `${sidPath(sid)}/baselines/${encodeURIComponent(name)}`
  },

  runFileUrl(sid, runId, name) {
    return `${runPath(sid, runId)}/file/${encodeURIComponent(name)}`
  },

  compareShotUrl(sid, folder, stepId) {
    return `${sidPath(sid)}/compare/${encodeURIComponent(folder)}/shots/${encodeURIComponent(stepId)}`
  },

  replayStreamUrl(sid) {
    return `${sidPath(sid)}/replay-stream`
  },

  fetchArtifactText(sid, runId, kind, stepId, pretty) {
    return readArtifactText(httpArtifactUrl(sid, runId, kind, stepId), pretty)
  },
}

function httpArtifactUrl(sid: string, runId: string, kind: string, stepId: string): string {
  return `${runPath(sid, runId)}/artifact/${kind}/${encodeURIComponent(stepId)}`
}
