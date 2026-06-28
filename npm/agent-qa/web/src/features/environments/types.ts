// web/src/features/environments/types.ts
// Mirrors environment/1 (lib/report-server.js handleSimpleRecords).

// A target to run a scenario against. `baseUrl` + `params` are forwarded to the
// replay CLI as `--param` values (baseUrl as `--param baseUrl=…`).
export interface EnvironmentRecord {
  schema: 'environment/1'
  id: string
  name: string
  baseUrl: string
  params: Record<string, string>
  description: string
  createdAt: number
  updatedAt: number
}
