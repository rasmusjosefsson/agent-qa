// web/src/lib/cases-api.ts
// Typed wrappers for the /api/cases/* endpoints (lib/report-server.js).
import type { CaseRecord, CaseWithScenario } from '@/features/cases/types'

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

export function getCases(): Promise<{ cases: CaseWithScenario[] }> {
  return getJson('/api/cases')
}

export function getCase(id: string): Promise<{ case: CaseWithScenario }> {
  return getJson(`/api/cases/${encodeURIComponent(id)}`)
}

export function upsertCase(
  id: string,
  body: Partial<CaseRecord>
): Promise<{ ok: boolean; case: CaseRecord }> {
  return postJson(`/api/cases/${encodeURIComponent(id)}`, body)
}

export function deleteCase(id: string): Promise<{ ok: boolean }> {
  return postJson(`/api/cases/${encodeURIComponent(id)}/delete`, {})
}

export function linkCase(
  id: string,
  scenarioSid: string | null
): Promise<{ ok: boolean; case: CaseWithScenario }> {
  return postJson(`/api/cases/${encodeURIComponent(id)}/link`, { scenarioSid })
}
