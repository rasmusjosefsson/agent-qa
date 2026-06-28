// web/src/features/sets/types.ts
// Mirrors the set/1 contract written by lib/report-server.js (handleSets).

// How a set resolves its members: an explicit case list, or any-of a tag query.
export type SetMode = 'manual' | 'tag'

export interface SetRecord {
  schema: 'set/1'
  id: string
  name: string
  description: string
  mode: SetMode
  caseIds: string[]
  tagQuery: string[]
  source: string
  sourceRef: string | null
  createdAt: number
  updatedAt: number
}

// A set joined to its resolved member count (computed server-side).
export interface SetWithCount extends SetRecord {
  caseCount: number
}
