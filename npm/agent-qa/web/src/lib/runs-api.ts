// Typed wrappers over the active ScenarioSource.
// HTTP mode calls the report server. Fixture mode reads a static tree and
// refuses writes. Callers keep these names; the source is selected once.
import {
  getScenarioSource,
  sourceWrites,
  type CrawlOpts,
  type ReplayOpts,
} from './source'
import type { CompareReport } from '@/features/runs/types'

export type { ReplayOpts } from './source'

export const READ_ONLY_ERROR = 'read-only'

async function mutate<T extends { ok: boolean; error?: string }>(
  fn: (writes: NonNullable<ReturnType<typeof sourceWrites>>) => Promise<T>,
): Promise<T> {
  const writes = sourceWrites(getScenarioSource())
  if (!writes) return { ok: false, error: READ_ONLY_ERROR } as T
  return fn(writes)
}

export function getScenarios() {
  return getScenarioSource().listScenarios()
}

export function getHealth() {
  return getScenarioSource().getHealth()
}

export function getScenarioDef(sid: string) {
  return getScenarioSource().getScenario(sid)
}

export function getTrend(sid: string, limit = 20) {
  return getScenarioSource().getTrend(sid, limit)
}

export function getRuns(sid: string) {
  return getScenarioSource().listRuns(sid)
}

export function getRunDetail(sid: string, runId: string) {
  return getScenarioSource().getRun(sid, runId)
}

export function startReplay(sid: string, opts?: ReplayOpts) {
  return mutate((writes) => writes.startReplay(sid, opts))
}

export function crawlScenario(url: string, opts?: CrawlOpts) {
  return mutate((writes) => writes.crawlScenario(url, opts))
}

export function deleteScenario(sid: string) {
  return mutate((writes) => writes.deleteScenario(sid))
}

export function insertStep(
  sid: string,
  kind: 'do' | 'check',
  draft: Record<string, unknown>,
  pos?: { after?: string; at?: number },
) {
  return mutate((writes) => writes.insertStep(sid, kind, draft, pos))
}

export function acceptShot(sid: string, runId: string, stepId: string) {
  return mutate((writes) => writes.acceptShot(sid, runId, stepId))
}

export function promoteHeal(sid: string, runId: string, stepId: string) {
  return mutate((writes) => writes.promoteHeal(sid, runId, stepId))
}

export function acceptDomshot(sid: string, runId: string, stepId: string) {
  return mutate((writes) => writes.acceptDomshot(sid, runId, stepId))
}

export function acceptAllDomshots(sid: string, runId: string) {
  return mutate((writes) => writes.acceptAllDomshots(sid, runId))
}

export function acceptAllShots(sid: string, runId: string) {
  return mutate((writes) => writes.acceptAllShots(sid, runId))
}

export function deleteRun(sid: string, runId: string) {
  return mutate((writes) => writes.deleteRun(sid, runId))
}

export function compareRuns(sid: string, runA?: string, runB?: string): Promise<{ ok: boolean; report?: CompareReport; error?: string }> {
  return mutate((writes) => writes.compareRuns(sid, runA, runB))
}

export function compareShotUrl(sid: string, folder: string, stepId: string): string {
  return getScenarioSource().compareShotUrl(sid, folder, stepId)
}

export function artifactUrl(sid: string, runId: string, kind: string, stepId: string): string {
  return getScenarioSource().artifactUrl(sid, runId, kind, stepId)
}

export function baselineShotUrl(sid: string, name: string): string {
  return getScenarioSource().baselineShotUrl(sid, name)
}

export function runFileUrl(sid: string, runId: string, name: string): string {
  return getScenarioSource().runFileUrl(sid, runId, name)
}

export function replayStreamUrl(sid: string): string | null {
  return getScenarioSource().replayStreamUrl(sid)
}

export function fetchArtifactText(sid: string, runId: string, kind: string, stepId: string, pretty: boolean) {
  return getScenarioSource().fetchArtifactText(sid, runId, kind, stepId, pretty)
}
