// web/src/lib/run-config-api.ts
// Typed wrappers for the /api/personas and /api/environments record surfaces.
import type { PersonaRecord } from '@/features/personas/types'
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

// Install an extension package (npm:<pkg> | git:<url> | https://…) — fetches it
// and wires ~/.agent-qa/agent-qa.toml. Returns { ok:false, error } on a failed
// install (npm/git error) rather than throwing.
export interface InstallResult {
  ok: boolean
  name?: string
  plugins?: { path: string; kinds: string[] }[]
  skills?: number
  error?: string
}
export function installPackage(source: string): Promise<InstallResult> {
  return postJson('/api/config/plugins/install', { source })
}

// --- installed packages: list / update / check-for-updates ---
export interface InstalledPackage {
  source: string
  name: string
  scheme: string
  plugins: string[]
  skills: number
  personas: number
  environments: number
}
export function getPackages(): Promise<{ packages: InstalledPackage[] }> {
  return getJson('/api/config/packages')
}
// Re-pull a package (re-runs `agent-qa install <source>`).
export function updatePackage(source: string): Promise<{ ok: boolean; updated?: string[]; error?: string }> {
  return postJson('/api/config/plugins/update', { source })
}
// Uninstall a package: removes it from the registry, rewires the config, and
// deletes its fetched files.
export function uninstallPackage(source: string): Promise<{ ok: boolean; removed?: number; error?: string }> {
  return postJson('/api/config/plugins/uninstall', { source })
}
export interface PackageUpdate {
  source: string
  name: string
  scheme: string
  current: string
  latest: string
  updateAvailable: boolean
}
export interface AppUpdate {
  name: string
  current: string
  latest: string
  updateAvailable: boolean
}
// Network-bound: checks each package's remote + the app vs its npm latest.
export function checkPackageUpdates(): Promise<{ packages: PackageUpdate[]; app: AppUpdate | null }> {
  return getJson('/api/config/packages/updates')
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
  session?: string | null
  headed?: boolean
  log: ConnectStep[]
}
export function connectPersona(
  personaId: string,
  environmentId: string,
  headed = false
): Promise<ConnectResult> {
  return postJson(`/api/personas/${encodeURIComponent(personaId)}/connect`, {
    environmentId,
    headed,
  })
}

// Sign a persona into a specific chat's OWN browser session, so that chat's
// agent operates an already-authenticated page (profile-bootstrap --session
// <chat session>). environmentId is optional — the plugin is otherwise
// discovered via agent-qa.toml / AGENT_QA_PLUGINS / $PATH.
export function connectPersonaToChat(
  chatId: string,
  personaId: string,
  environmentId?: string,
  headed = false
): Promise<ConnectResult> {
  return postJson(`/api/chat/c/${encodeURIComponent(chatId)}/connect`, {
    personaId,
    ...(environmentId ? { environmentId } : {}),
    headed,
  })
}
