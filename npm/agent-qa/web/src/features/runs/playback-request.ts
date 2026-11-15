// A demo Replay click asks the event player to start. The player may not
// be mounted yet (the run is still loading), so the request waits until
// the matching run's player is on screen.

type Request = { sid: string; runId: string }

let pending: Request | null = null
const listeners = new Set<() => void>()

export function requestPlayback(sid: string, runId: string) {
  pending = { sid, runId }
  for (const listener of listeners) listener()
}

export function consumePlaybackRequest(sid: string, runId: string): boolean {
  if (!pending || pending.sid !== sid || pending.runId !== runId) return false
  pending = null
  return true
}

export function onPlaybackRequest(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}
