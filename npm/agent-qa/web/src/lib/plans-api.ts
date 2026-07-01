// web/src/lib/plans-api.ts
// Typed wrappers for the /api/plans/* endpoints (lib/report-server.js).
import type { PlanRecord, PlanRunResult, PlanWithCount } from '@/features/plans/types'
import type { CaseWithScenario } from '@/features/cases/types'

async function getJson<T>(path: string): Promise<T> {
  const res = await fetch(path, { headers: { accept: 'application/json' } })
  if (!res.ok) throw new Error(`${path} → ${res.status}`)
  return (await res.json()) as T
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body ?? {}),
  })
  if (!res.ok) {
    const j = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(j.error || `${path} → ${res.status}`)
  }
  return (await res.json()) as T
}

export function getPlans(): Promise<{ plans: PlanWithCount[] }> {
  return getJson('/api/plans')
}

export function getPlan(id: string): Promise<{ plan: PlanWithCount }> {
  return getJson(`/api/plans/${encodeURIComponent(id)}`)
}

// Resolved member cases (joined to each case's last-run scenario summary).
export function getPlanCases(id: string): Promise<{ cases: CaseWithScenario[] }> {
  return getJson(`/api/plans/${encodeURIComponent(id)}/cases`)
}

export function upsertPlan(
  id: string,
  body: Partial<PlanRecord>
): Promise<{ ok: boolean; plan: PlanRecord }> {
  return postJson(`/api/plans/${encodeURIComponent(id)}`, body)
}

export function deletePlan(id: string): Promise<{ ok: boolean }> {
  return postJson(`/api/plans/${encodeURIComponent(id)}/delete`, {})
}

// Kick a replay of every member case's linked scenario (fire-and-forget;
// results land in each case's latest run). `opts` carries the chosen persona
// (profile) + environment values (params) for the run.
export function runPlan(
  id: string,
  opts?: {
    profile?: string
    params?: Record<string, string>
    // Named persona/environment so the server can resolve + inject the
    // persona's credentials (auth-walled scenarios re-authenticate on replay).
    personaId?: string
    environmentId?: string
  }
): Promise<PlanRunResult> {
  return postJson(`/api/plans/${encodeURIComponent(id)}/run`, opts ?? {})
}
