// web/src/features/personas/types.ts
// Mirrors persona/1 (lib/report-server.js handleSimpleRecords).

// A login identity to run a scenario as. `profile` is the value forwarded to
// the replay CLI as `--profile`; `credentials.envPrefix` names where the auth
// plugin reads this identity's secrets (e.g. AGENT_QA_PROFILE_ADMIN_*).
export interface PersonaRecord {
  schema: 'persona/1'
  id: string
  name: string
  profile: string
  credentials: { envPrefix: string }
  description: string
  createdAt: number
  updatedAt: number
}
