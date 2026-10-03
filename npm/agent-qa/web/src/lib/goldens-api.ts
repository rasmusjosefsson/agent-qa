// Typed wrappers for /api/goldens — the per-scenario baseline files behind
// the Goldens page. Files are served by /api/goldens/<sid>/<name>.

export interface GoldenFile {
  name: string
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
