// web/src/features/plans/types.ts
// Mirrors the plan/1 contract written by lib/report-server.js (handlePlans).

export interface PlanScope {
  setIds: string[]
  caseIds: string[]
}

export interface PlanRecord {
  schema: 'plan/1'
  id: string
  name: string
  description: string
  scope: PlanScope
  source: string
  sourceRef: string | null
  createdAt: number
  updatedAt: number
}

// A plan joined to its resolved member count (computed server-side).
export interface PlanWithCount extends PlanRecord {
  caseCount: number
}

// Result of POST /api/plans/:id/run — which member scenarios were kicked off.
export interface PlanRunResult {
  ok: boolean
  started: { caseId: string; sid: string }[]
  skipped: { caseId: string; reason: string }[]
  // Set (with ok:false) when the run is refused up front — e.g. the chosen
  // persona's vault credentials couldn't be resolved, so nothing was replayed.
  error?: string
}
