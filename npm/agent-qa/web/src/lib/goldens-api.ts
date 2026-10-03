// Typed wrappers for /api/goldens — the per-scenario baseline files behind
// the Goldens page. Files are served by /api/goldens/<sid>/<name>; run
// artifacts (actual screenshot, diff map) by /api/goldens/<sid>/<name>/<which>.

export interface GoldenFile {
  name: string
  /** Scenario step id the baseline is keyed on (name minus .png/.snap.txt). */
  stepId?: string
  /** Whether the check claim behind this golden is enabled (default true). */
  enabled?: boolean
  /** The latest run produced a shots-diff map for this step. */
  hasDiff?: boolean
  size?: number
  mtime?: number
}

export interface GoldenScenario {
  sid: string
  files: GoldenFile[]
}

export interface GoldensResponse {
  store: string
  scenarios: GoldenScenario[]
}

export async function fetchGoldens(): Promise<GoldensResponse> {
  const res = await fetch('/api/goldens', { headers: { accept: 'application/json' } })
  if (!res.ok) throw new Error(`/api/goldens → ${res.status}`)
  return (await res.json()) as GoldensResponse
}

export function goldenFileUrl(sid: string, name: string): string {
  return `/api/goldens/${encodeURIComponent(sid)}/${encodeURIComponent(name)}`
}

/** `which` is 'actual' (latest run's screenshot) or 'diff' (pixel-diff map). */
export function goldenAssetUrl(sid: string, name: string, which: 'actual' | 'diff'): string {
  return `${goldenFileUrl(sid, name)}/${which}`
}

export async function setGoldenEnabled(
  sid: string,
  name: string,
  enabled: boolean
): Promise<void> {
  const res = await fetch(`${goldenFileUrl(sid, name)}/enabled`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ enabled }),
  })
  if (!res.ok) throw new Error(`enable golden → ${res.status}`)
}
