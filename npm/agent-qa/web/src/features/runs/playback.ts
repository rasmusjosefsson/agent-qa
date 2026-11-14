// Timed playback of a finished run's events.jsonl.
//
// Each collapsed step holds for its recorded duration (`ms`). Steps shorter
// than MIN_HOLD_SEC stay on screen long enough to see; longer steps keep
// their real length, so a navigation still outlasts a click. Speed scales
// the clock. The video scrubber is a different control and is unchanged.

import type { RunEvent } from './types'

export const PLAYBACK_SPEEDS = [1, 2, 4, 8] as const

/** Short steps are still visible at 1×. Longer steps keep their recorded length. */
export const MIN_HOLD_SEC = 0.4

export interface PlaybackMarker {
  idx: number
  id?: string
  label: string
  /** Seconds from the start of the trace. */
  at: number
  /** How long this step stays current, in seconds. */
  dur: number
  shot: boolean
  status?: string
}

export function holdSeconds(ms: number | undefined): number {
  const raw = typeof ms === 'number' && Number.isFinite(ms) && ms > 0 ? ms / 1000 : MIN_HOLD_SEC
  return Math.max(MIN_HOLD_SEC, raw)
}

export function buildPlayback(steps: RunEvent[]): PlaybackMarker[] {
  let cursor = 0
  const markers: PlaybackMarker[] = []
  for (const step of steps) {
    if (typeof step.idx !== 'number') continue
    if (step.status === 'pending' || step.pending) continue
    const dur = holdSeconds(step.ms)
    markers.push({
      idx: step.idx,
      id: step.id,
      label: (step.intent || step.id || `Step ${step.idx}`).trim(),
      at: cursor,
      dur,
      shot: typeof step.screenshot === 'string' && step.screenshot.length > 0,
      status: step.status,
    })
    cursor += dur
  }
  return markers
}

export function playbackEnd(markers: PlaybackMarker[]): number {
  if (!markers.length) return 0
  const last = markers[markers.length - 1]
  return last.at + last.dur
}

/** Step that is current at `timeSec`. The last step stays current at the end. */
export function markerAtPlayback(markers: PlaybackMarker[], timeSec: number): PlaybackMarker | null {
  if (!markers.length) return null
  const t = Number.isFinite(timeSec) ? timeSec : 0
  for (const marker of markers) {
    if (t < marker.at + marker.dur) return marker
  }
  return markers[markers.length - 1]
}

export function advancePlayback(
  timeSec: number,
  dtSec: number,
  rate: number,
  endSec: number
): { time: number; done: boolean } {
  const speed = Number.isFinite(rate) && rate > 0 ? rate : 1
  const dt = Number.isFinite(dtSec) && dtSec > 0 ? dtSec : 0
  const end = Number.isFinite(endSec) && endSec > 0 ? endSec : 0
  const next = Math.max(0, timeSec) + dt * speed
  if (next >= end) return { time: end, done: true }
  return { time: next, done: false }
}
