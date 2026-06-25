// web/src/lib/runs-api.ts
// Typed wrappers for the read-only /api/scenarios/* endpoints.
import type { RunDetail, RunSummary, ScenarioDef, ScenarioSummary } from '@/features/runs/types'

async function getJson<T>(path: string): Promise<T> {
  const res = await fetch(path, { headers: { accept: 'application/json' } })
  if (!res.ok) throw new Error(`${path} → ${res.status}`)
  return (await res.json()) as T
}

export function getScenarios(): Promise<{ scenariosRoot: string; scenarios: ScenarioSummary[] }> {
  return getJson('/api/scenarios')
}

export function getScenarioDef(sid: string): Promise<{ sid: string; scenario: ScenarioDef }> {
  return getJson(`/api/scenarios/${encodeURIComponent(sid)}/scenario`)
}

export function getRuns(sid: string): Promise<{ sid: string; replays: RunSummary[] }> {
  return getJson(`/api/scenarios/${encodeURIComponent(sid)}/runs`)
}

export function getRunDetail(sid: string, runId: string): Promise<RunDetail> {
  return getJson(`/api/scenarios/${encodeURIComponent(sid)}/runs/${encodeURIComponent(runId)}`)
}

export async function startReplay(sid: string): Promise<{ ok: boolean; error?: string }> {
  const res = await fetch(`/api/scenarios/${encodeURIComponent(sid)}/replay`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: '{}',
  })
  if (res.ok) return { ok: true }
  const j = (await res.json().catch(() => ({}))) as { error?: string }
  return { ok: false, error: j.error || String(res.status) }
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
