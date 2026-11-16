// web/src/lib/cases-api.ts
// Reads go through the active source (report server, or the static demo).
// Writes stay on /api/cases and are only called when the source can write.
import type { CaseRecord, CaseWithScenario } from '@/features/cases/types'
import { getScenarioSource } from '@/lib/source'

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
  return getScenarioSource().listCases()
}

export function getCase(id: string): Promise<{ case: CaseWithScenario }> {
  return getScenarioSource().getCase(id)
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
