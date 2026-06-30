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
  // The login the chat agent picks when the user names none (falls back to the
  // sole persona when nothing is flagged).
  default?: boolean
  // Credentials handed to the auth plugin (env var → value). Each value may be
  // a literal or a `vault:<path>:<key>` reference resolved at run time.
  credentials: { entries: Record<string, string> }
  description: string
  createdAt: number
  updatedAt: number
}
