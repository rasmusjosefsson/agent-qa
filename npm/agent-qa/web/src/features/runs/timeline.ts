// Place replay steps on a recording timeline.
//
// Absolute cues (video-cues.json, milliseconds since the recording
// started) win. Without them, step durations are laid onto the media
// length: slack before the first step is treated as lead-in (setup runs
// before dispatch), and a recording shorter than the steps is scaled so
// every marker still fits.

import { verbCat, type VerbCat } from './rows'

export interface TimelineStep {
  idx: number
  id?: string
  intent?: string
  kind?: string
  status?: string
  ms?: number
  pending?: boolean
  screenshot?: string
}

export interface VideoCue {
  id?: string
  idx: number
  tMs: number
}

export interface Marker {
  idx: number
  id?: string
  label: string
  cat: VerbCat
  status?: string
  /** Seconds from the start of the recording. */
  at: number
  shot: boolean
}

export const PLAYBACK_RATES = [1, 2, 4, 8] as const

export function catFromKind(kind?: string): VerbCat {
  if (!kind) return 'action'
  if (kind === 'check') return 'assert'
  return verbCat(kind.replace(/^do:/, ''))
}

function cueSeconds(step: TimelineStep, cues: VideoCue[] | null | undefined): number | null {
  if (!cues?.length) return null
  const hit =
    cues.find((c) => c.idx === step.idx) ||
    (step.id ? cues.find((c) => c.id === step.id) : undefined)
  if (!hit || typeof hit.tMs !== 'number' || !Number.isFinite(hit.tMs)) return null
  return Math.max(0, hit.tMs / 1000)
}

export function buildMarkers(
  steps: TimelineStep[],
  cues: VideoCue[] | null | undefined,
  mediaDurationSec: number
): Marker[] {
  const live = steps.filter((s) => s.status !== 'pending' && !s.pending && typeof s.idx === 'number')
  if (!live.length) return []

  const anchored = live.map((s) => cueSeconds(s, cues))
  const anyCue = anchored.some((t) => t != null)
  let cursor = 0
  const markers: Marker[] = live.map((step, i) => {
    const dur = Math.max(0, (typeof step.ms === 'number' ? step.ms : 0) / 1000)
    const at = anyCue ? (anchored[i] ?? cursor) : cursor
    cursor = at + dur
    return {
      idx: step.idx,
      id: step.id,
      label: (step.intent || step.id || `Step ${step.idx}`).trim(),
      cat: catFromKind(step.kind),
      status: step.status,
      at,
      shot: !!step.screenshot,
    }
  })

  if (!anyCue && mediaDurationSec > 0) {
    const covered = cursor
    if (covered > mediaDurationSec && covered > 0) {
      const scale = mediaDurationSec / covered
      for (const m of markers) m.at *= scale
    } else if (covered < mediaDurationSec) {
      const lead = mediaDurationSec - covered
      for (const m of markers) m.at += lead
    }
  }

  markers.sort((a, b) => a.at - b.at || a.idx - b.idx)
  return markers
}

/** Latest marker whose start is at or before `timeSec`. Markers must be time-sorted. */
export function markerAtTime(markers: Marker[], timeSec: number): Marker | null {
  let hit: Marker | null = null
  for (const m of markers) {
    if (m.at <= timeSec + 0.001) hit = m
    else break
  }
  return hit
}

export function trackDuration(markers: Marker[], mediaDurationSec: number): number {
  const last = markers.length ? markers[markers.length - 1].at : 0
  const media = Number.isFinite(mediaDurationSec) && mediaDurationSec > 0 ? mediaDurationSec : 0
  return Math.max(media, last, 0.001)
}

/** `0:48` — minutes are unpadded, seconds are always two digits. */
export function formatClock(sec: number): string {
  if (!Number.isFinite(sec) || sec < 0) sec = 0
  const s = Math.floor(sec)
  const m = Math.floor(s / 60)
  const r = s % 60
  return `${m}:${r.toString().padStart(2, '0')}`
}

/**
 * How long Frames mode holds each screenshot. 8× is under a tenth of a
 * second; the floor keeps the advance visible instead of a blur.
 */
export function frameStepMs(rate: number): number {
  const safe = Number.isFinite(rate) && rate > 0 ? rate : 1
  return Math.max(48, Math.round(720 / safe))
}

/** `null` when the element kept the requested rate; the actual rate when it clamped. */
export function clampedRate(requested: number, actual: number): number | null {
  if (!Number.isFinite(actual)) return null
  return Math.abs(actual - requested) > 0.05 ? actual : null
}
