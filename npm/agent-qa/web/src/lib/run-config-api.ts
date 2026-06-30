// web/src/lib/run-config-api.ts
// Typed wrappers for the /api/personas and /api/environments record surfaces.
import type { PersonaRecord, SecretSourceRecord } from '@/features/personas/types'
import type { EnvironmentRecord, PluginInfo } from '@/features/environments/types'

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

// --- personas ---
export function getPersonas(): Promise<{ personas: PersonaRecord[] }> {
  return getJson('/api/personas')
}
export function upsertPersona(
  id: string,
  body: Partial<PersonaRecord>
): Promise<{ ok: boolean; persona: PersonaRecord }> {
  return postJson(`/api/personas/${encodeURIComponent(id)}`, body)
}
export function deletePersona(id: string): Promise<{ ok: boolean }> {
  return postJson(`/api/personas/${encodeURIComponent(id)}/delete`, {})
}

// --- secret sources (vault targets) ---
export function getSecretSources(): Promise<{ secretSources: SecretSourceRecord[] }> {
  return getJson('/api/secret-sources')
}
export function upsertSecretSource(
  id: string,
  body: Partial<SecretSourceRecord>
): Promise<{ ok: boolean; secretsource: SecretSourceRecord }> {
  return postJson(`/api/secret-sources/${encodeURIComponent(id)}`, body)
}
export function deleteSecretSource(id: string): Promise<{ ok: boolean }> {
  return postJson(`/api/secret-sources/${encodeURIComponent(id)}/delete`, {})
}

// --- environments ---
export function getEnvironments(): Promise<{ environments: EnvironmentRecord[] }> {
  return getJson('/api/environments')
}
export function upsertEnvironment(
  id: string,
  body: Partial<EnvironmentRecord>
): Promise<{ ok: boolean; environment: EnvironmentRecord }> {
  return postJson(`/api/environments/${encodeURIComponent(id)}`, body)
}
export function deleteEnvironment(id: string): Promise<{ ok: boolean }> {
  return postJson(`/api/environments/${encodeURIComponent(id)}/delete`, {})
}

// --- discovered auth plugins (read-only) ---
export function getPlugins(): Promise<{ available: boolean; plugins: PluginInfo[] }> {
  return getJson('/api/plugins')
}

// --- plugin registry (UI-managed paths injected as AGENT_QA_PLUGINS) ---
export function getPluginPaths(): Promise<{ paths: string[] }> {
  return getJson('/api/config/plugins')
}
export function setPluginPaths(paths: string[]): Promise<{ ok: boolean; paths: string[] }> {
  return postJson('/api/config/plugins', { paths })
}
// Upload a downloaded plugin file → stored, made executable, and registered.
export function importPlugin(
  filename: string,
  contentBase64: string
): Promise<{ ok: boolean; path: string; paths: string[] }> {
  return postJson('/api/config/plugins/import', { filename, contentBase64 })
}

// --- connect (bootstrap a persona's login for an environment) ---
export interface ConnectStep {
  step: string
  code: number | null
  stdout: string
  stderr: string
  spawnError: string | null
}
export interface ConnectResult {
  ok: boolean
  authenticated: boolean
  profile: string
  log: ConnectStep[]
}
export function connectPersona(personaId: string, environmentId: string): Promise<ConnectResult> {
  return postJson(`/api/personas/${encodeURIComponent(personaId)}/connect`, { environmentId })
}
