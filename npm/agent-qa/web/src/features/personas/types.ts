// web/src/features/personas/types.ts
// Mirrors persona/1 (lib/report-server.js handleSimpleRecords).

// A login identity to run a scenario as. `profile` is the value forwarded to
// the replay CLI as `--profile`.
export interface PersonaRecord {
  schema: 'persona/1'
  id: string
  name: string
  profile: string
  description: string
  createdAt: number
  updatedAt: number
}
