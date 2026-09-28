// web/src/lib/runs-api.ts
// Typed wrappers for the read-only /api/scenarios/* endpoints.
import type {
  AuditTrend,
  CompareReport,
  RunDetail,
  RunSummary,
  ScenarioDef,
  ScenarioHealth,
  ScenarioSummary,
} from '@/features/runs/types'

async function getJson<T>(path: string): Promise<T> {
  const res = await fetch(path, { headers: { accept: 'application/json' } })
  if (!res.ok) throw new Error(`${path} → ${res.status}`)
  return (await res.json()) as T
}

export function getScenarios(): Promise<{ scenariosRoot: string; scenarios: ScenarioSummary[] }> {
  return getJson('/api/scenarios')
}

// Silent-degradation rollup: per-scenario step ids flagged by audit
// flaky / slow / heal-chronic. Empty array when the suite is quiet or
// the CLI is unavailable.
export function getHealth(): Promise<{ health: ScenarioHealth[] }> {
  return getJson('/api/health')
}

export function getScenarioDef(sid: string): Promise<{ sid: string; scenario: ScenarioDef }> {
  return getJson(`/api/scenarios/${encodeURIComponent(sid)}/scenario`)
}

// `audit trend` rollup for one scenario — outcome glyphs + duration
// sparkline for the Runs header. trend is null when the scenario has
// no replayed runs or the CLI is unavailable.
export function getTrend(sid: string, limit = 20): Promise<{ sid: string; trend: AuditTrend | null }> {
  return getJson(`/api/scenarios/${encodeURIComponent(sid)}/audit/trend?limit=${limit}`)
}

export function getRuns(sid: string): Promise<{ sid: string; replays: RunSummary[] }> {
  return getJson(`/api/scenarios/${encodeURIComponent(sid)}/runs`)
}

export function getRunDetail(sid: string, runId: string): Promise<RunDetail> {
  return getJson(`/api/scenarios/${encodeURIComponent(sid)}/runs/${encodeURIComponent(runId)}`)
}

// Optional persona/environment for a replay. A named persona lets the server
// resolve + inject its credentials so an auth-walled scenario re-authenticates.
export interface ReplayOpts {
  headed?: boolean
  profile?: string
  params?: Record<string, string>
  personaId?: string
  environmentId?: string
}

export async function startReplay(
  sid: string,
  opts?: ReplayOpts
): Promise<{ ok: boolean; error?: string }> {
  const res = await fetch(`/api/scenarios/${encodeURIComponent(sid)}/replay`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(opts ?? {}),
  })
  // A named persona whose vault refs can't be resolved comes back 200 { ok:false }.
  if (res.ok) {
    const j = (await res.json().catch(() => ({}))) as { ok?: boolean; error?: string }
    if (j.ok === false) return { ok: false, error: j.error || 'replay refused' }
    return { ok: true }
  }
  const j = (await res.json().catch(() => ({}))) as { error?: string }
  return { ok: false, error: j.error || String(res.status) }
}

export async function crawlScenario(
  url: string,
  opts?: { sid?: string; max?: number },
): Promise<{ ok: boolean; stdout?: string; error?: string }> {
  const res = await fetch('/api/scenarios/crawl', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ url, ...(opts?.sid ? { sid: opts.sid } : {}), ...(opts?.max ? { max: opts.max } : {}) }),
  })
  const j = (await res.json().catch(() => ({}))) as { stdout?: string; error?: string }
  if (res.ok) return { ok: true, stdout: j.stdout }
  return { ok: false, error: j.error || String(res.status) }
}

export async function deleteScenario(sid: string): Promise<{ ok: boolean; error?: string }> {
  const res = await fetch(`/api/scenarios/${encodeURIComponent(sid)}/delete`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: '{}',
  })
  if (res.ok) return { ok: true }
  const j = (await res.json().catch(() => ({}))) as { error?: string }
  return { ok: false, error: j.error || String(res.status) }
}

// Splice a step into a saved scenario.json (delegates to `scenario insert`
// on the CLI). `after` is a step id; omit both to append.
export async function insertStep(
  sid: string,
  kind: 'do' | 'check',
  draft: Record<string, unknown>,
  pos?: { after?: string; at?: number }
): Promise<{ ok: boolean; error?: string }> {
  const res = await fetch(`/api/scenarios/${encodeURIComponent(sid)}/insert-step`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ kind, draft, ...(pos || {}) }),
  })
  const j = (await res.json().catch(() => ({}))) as { ok?: boolean; error?: string }
  if (res.ok && j.ok !== false) return { ok: true }
  return { ok: false, error: j.error || `insert failed (${res.status})` }
}

// POST .../runs/:runId/shot-accept — promote this run's screenshot for
// stepId to the checked-in baseline (the web-side `agent-qa shot-accept`).
export async function acceptShot(
  sid: string,
  runId: string,
  stepId: string
): Promise<{ ok: boolean; error?: string }> {
  const res = await fetch(
    `/api/scenarios/${encodeURIComponent(sid)}/runs/${encodeURIComponent(runId)}/shot-accept`,
    { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ stepId }) }
  )
  if (res.ok) return { ok: true }
  const j = (await res.json().catch(() => ({}))) as { error?: string }
  return { ok: false, error: j.error || String(res.status) }
}


// POST .../runs/:runId/heal-promote — absorb the run's suggested locator
// patch for stepId into scenario.json (the web-side `heal-promote --apply`).
// 409 means the rebase guard fired (scenario.json drifted since the patch
// was written — surface the stderr so the user re-runs to re-derive it).
export async function promoteHeal(
  sid: string,
  runId: string,
  stepId: string
): Promise<{ ok: boolean; error?: string }> {
  const res = await fetch(
    `/api/scenarios/${encodeURIComponent(sid)}/runs/${encodeURIComponent(runId)}/heal-promote`,
    { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ stepId }) }
  )
  if (res.ok) return { ok: true }
  const j = (await res.json().catch(() => ({}))) as { error?: string }
  return { ok: false, error: j.error || String(res.status) }
}

// Same route with {all:true} — promote every screenshot captured in the run
// (the "apply new goldens" path after an intentional UI change).
export async function acceptAllShots(
  sid: string,
  runId: string
): Promise<{ ok: boolean; minted?: string[]; error?: string }> {
  const res = await fetch(
    `/api/scenarios/${encodeURIComponent(sid)}/runs/${encodeURIComponent(runId)}/shot-accept`,
    { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ all: true }) }
  )
  const j = (await res.json().catch(() => ({}))) as { ok?: boolean; minted?: string[]; error?: string }
  if (res.ok && j.ok) return { ok: true, minted: j.minted || [] }
  return { ok: false, error: j.error || String(res.status) }
}


export async function deleteRun(sid: string, runId: string): Promise<{ ok: boolean; error?: string }> {
  const res = await fetch(
    `/api/scenarios/${encodeURIComponent(sid)}/runs/${encodeURIComponent(runId)}/delete`,
    { method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}' }
  )
  if (res.ok) return { ok: true }
  const j = (await res.json().catch(() => ({}))) as { error?: string }
  return { ok: false, error: j.error || String(res.status) }
}

export async function compareRuns(
  sid: string,
  runA?: string,
  runB?: string
): Promise<{ ok: boolean; report?: CompareReport; error?: string }> {
  const res = await fetch(`/api/scenarios/${encodeURIComponent(sid)}/compare`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ ...(runA ? { runA } : {}), ...(runB ? { runB } : {}) }),
  })
  const j = (await res.json().catch(() => ({}))) as CompareReport & { error?: string }
  if (!res.ok) return { ok: false, error: j.error || String(res.status) }
  return { ok: true, report: j }
}

export function compareShotUrl(sid: string, folder: string, stepId: string): string {
  return `/api/scenarios/${encodeURIComponent(sid)}/compare/${encodeURIComponent(folder)}/shots/${encodeURIComponent(stepId)}`
}

export function artifactUrl(sid: string, runId: string, kind: string, stepId: string): string {
  return `/api/scenarios/${encodeURIComponent(sid)}/runs/${encodeURIComponent(runId)}/artifact/${kind}/${encodeURIComponent(stepId)}`
}

export async function fetchArtifactText(
  sid: string,
  runId: string,
  kind: string,
  stepId: string,
  pretty: boolean
): Promise<string | null> {
  const res = await fetch(artifactUrl(sid, runId, kind, stepId))
  if (!res.ok) return null
  let text = await res.text()
  if (pretty) {
    try {
      text = JSON.stringify(JSON.parse(text), null, 2)
    } catch {
      /* leave raw */
    }
  }
  return text
}
