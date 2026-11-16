// web/src/lib/plans-api.ts
// Reads go through the active source (report server, or the static demo).
// Writes and run stay on /api/plans and are only called when a server is up.
import type { PlanRecord, PlanRunResult, PlanWithCount } from '@/features/plans/types'
import type { CaseWithScenario } from '@/features/cases/types'
import { getScenarioSource } from '@/lib/source'

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
  return getScenarioSource().listPlans()
}

export function getPlan(id: string): Promise<{ plan: PlanWithCount }> {
  return getScenarioSource().getPlan(id)
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
    headed?: boolean
    profile?: string
    params?: Record<string, string>
    // Named persona/environment so the server can resolve + inject the
    // persona's credentials (auth-walled scenarios re-authenticate on replay).
    personaId?: string
    environmentId?: string
    // After the run settles, spawn a repair chat per failed member.
    autoRepair?: boolean
  }
): Promise<PlanRunResult> {
  return postJson(`/api/plans/${encodeURIComponent(id)}/run`, opts ?? {})
}
