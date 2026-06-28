// web/src/lib/sets-api.ts
// Typed wrappers for the /api/sets/* endpoints (lib/report-server.js).
import type { SetRecord, SetWithCount } from '@/features/sets/types'
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

export function getSets(): Promise<{ sets: SetWithCount[] }> {
  return getJson('/api/sets')
}

export function getSet(id: string): Promise<{ set: SetWithCount }> {
  return getJson(`/api/sets/${encodeURIComponent(id)}`)
}

// Resolved member cases (joined to each case's last-run scenario summary).
export function getSetCases(id: string): Promise<{ cases: CaseWithScenario[] }> {
  return getJson(`/api/sets/${encodeURIComponent(id)}/cases`)
}

export function upsertSet(
  id: string,
  body: Partial<SetRecord>
): Promise<{ ok: boolean; set: SetRecord }> {
  return postJson(`/api/sets/${encodeURIComponent(id)}`, body)
}

export function deleteSet(id: string): Promise<{ ok: boolean }> {
  return postJson(`/api/sets/${encodeURIComponent(id)}/delete`, {})
}
