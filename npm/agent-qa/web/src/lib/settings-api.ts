// web/src/lib/settings-api.ts
// Typed wrappers for /api/config/settings — the flat workbench preferences
// store under <root>/_config/settings.json. Env vars win over stored values;
// `env` reports which fields are env-forced, `effective` the resolved value.
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

export type ChatBackend = 'auto' | 'pi' | 'opencode'

export interface WorkbenchSettings {
  chatBackend?: 'pi' | 'opencode'
  headedDefault?: boolean
}

export interface SettingsResponse {
  settings: WorkbenchSettings
  env: { chatBackend: string | null }
  effective: { chatBackend: ChatBackend; headedDefault: boolean }
  root: string
}

// Fetch-once cache — populated by updateSettings too so post-write readers
// see fresh values.
let cached: Promise<SettingsResponse> | null = null

export function getSettings(): Promise<SettingsResponse> {
  return getJson('/api/config/settings')
}

export function updateSettings(patch: {
  chatBackend?: ChatBackend
  headedDefault?: boolean
}): Promise<SettingsResponse> {
  return postJson<SettingsResponse>('/api/config/settings', patch).then((r) => {
    cached = Promise.resolve(r)
    return r
  })
}

// Fetch-once helper for pages that just need the resolved defaults (e.g. to
// seed the headed toggle). Cached so repeat callers share the one request.
export function effectiveSettings(): Promise<SettingsResponse> {
  cached = cached || getSettings()
  return cached
}

// ---- Golden storage ([baselines] in agent-qa.toml) ----

export type BaselineStore = 'local' | 'github' | 'turso'

export interface BaselinesConfig {
  configPath: string | null
  store: BaselineStore
  repo: string
  branch: string
  prefix: string
  url: string
  tokenEnv: string
}

export interface BaselinesStatus {
  store: string
  scenarios?: Array<{ sid: string; remoteFiles: number; localFiles: number }>
}

export interface BaselinesSyncResult {
  code: number
  stdout: string
  stderr: string
}

export function getBaselinesConfig(): Promise<BaselinesConfig> {
  return getJson('/api/baselines/config')
}

export function updateBaselinesConfig(body: {
  store: BaselineStore
  repo?: string
  branch?: string
  prefix?: string
  url?: string
  tokenEnv?: string
}): Promise<{ ok: boolean; configPath: string }> {
  return postJson('/api/baselines/config', body)
}

export function getBaselinesStatus(): Promise<BaselinesStatus> {
  return getJson('/api/baselines/status')
}

export function syncBaselines(direction: 'pull' | 'push'): Promise<BaselinesSyncResult> {
  return postJson('/api/baselines/sync', { direction })
}
