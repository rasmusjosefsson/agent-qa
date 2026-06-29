// web/src/features/environments/types.ts
// Mirrors environment/1 (lib/report-server.js handleSimpleRecords).

// Connection block: which auth plugin signs in to this environment + the
// free-form config it reads. The plugin binary is supplied downstream
// (agent-qa.toml [plugins]); we only reference it by name.
export interface EnvironmentAuth {
  plugin: string
  loginUrl: string
  config: Record<string, string>
}

// A target to run a scenario against. `baseUrl` + `params` are forwarded to the
// replay CLI as `--param` values (baseUrl as `--param baseUrl=…`).
export interface EnvironmentRecord {
  schema: 'environment/1'
  id: string
  name: string
  baseUrl: string
  params: Record<string, string>
  auth: EnvironmentAuth
  description: string
  createdAt: number
  updatedAt: number
}

// One discovered auth plugin (shape from `agent-qa plugins list --json`).
export interface PluginInfo {
  kind?: string
  name?: string
  path?: string
  source?: string
}
