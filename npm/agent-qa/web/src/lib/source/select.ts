import { createFixtureSource } from './fixture-source'
import { httpSource } from './http-source'
import type { ScenarioSource, ScenarioWrites, SourceKind } from './types'

export interface SourceSelection {
  /** `VITE_AQA_SOURCE`. Empty and omitted both mean http. */
  kind?: string
  /** `VITE_AQA_FIXTURE_BASE`. Fixture mode only. */
  fixtureBase?: string
}

export function resolveSourceKind(raw: string | undefined): SourceKind {
  const kind = (raw ?? '').trim()
  if (kind === '' || kind === 'http') return 'http'
  if (kind === 'fixture') return 'fixture'
  throw new Error(`VITE_AQA_SOURCE must be "http" or "fixture" (got ${JSON.stringify(raw)})`)
}

export function createScenarioSource(selection?: SourceSelection): ScenarioSource {
  const kind = resolveSourceKind(selection?.kind ?? import.meta.env.VITE_AQA_SOURCE)
  if (kind === 'fixture') {
    const base = selection && 'fixtureBase' in selection ? selection.fixtureBase : import.meta.env.VITE_AQA_FIXTURE_BASE
    return createFixtureSource(base)
  }
  return httpSource
}

/** Writes are allowed only when `capabilities.writes` is set. */
export function sourceWrites(source: ScenarioSource): ScenarioWrites | null {
  if (!source.capabilities.writes) return null
  return source.writes
}

export function canWrite(source: ScenarioSource = getScenarioSource()): boolean {
  return source.capabilities.writes
}

/** Shown on nav and pages that need a local `agent-qa web`. */
export const LOCAL_ONLY_NOTE = 'Available when you run agent-qa locally'

/** The static demo has no report server. The local app does. */
export function hasServer(source: ScenarioSource = getScenarioSource()): boolean {
  return source.capabilities.server
}

/** Runs is the only workbench route the static demo can serve. */
export function routeNeedsServer(pathname: string): boolean {
  const path = pathname.split('?')[0].replace(/\/+$/, '') || '/'
  return path !== '/' && path !== '/index.html'
}

let override: ScenarioSource | null = null
let cached: ScenarioSource | null = null

export function getScenarioSource(): ScenarioSource {
  if (override) return override
  if (!cached) cached = createScenarioSource()
  return cached
}

/** Install a source for tests. Pass null to read `VITE_AQA_SOURCE` again. */
export function setScenarioSourceForTests(source: ScenarioSource | null): void {
  override = source
  if (!source) cached = null
}
