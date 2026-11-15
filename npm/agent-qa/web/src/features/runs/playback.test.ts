import { describe, expect, it } from 'vitest'
import {
  advancePlayback,
  buildPlayback,
  holdSeconds,
  markerAtPlayback,
  MIN_HOLD_SEC,
  playbackEnd,
} from './playback'
import type { RunEvent } from './types'

const steps: RunEvent[] = [
  { idx: 1, id: 's0', intent: 'Open', kind: 'do:goto', status: 'pass', ms: 100, screenshot: 'screenshots/s0.png' },
  { idx: 2, id: 's1', intent: 'Save', kind: 'do:click', status: 'pass', ms: 1500, screenshot: 'screenshots/s1.png' },
  { idx: 3, id: 's2', intent: 'Pending row', status: 'pending', pending: true, ms: 10 },
]

describe('event playback clock', () => {
  it('holds short steps at the minimum and keeps longer steps', () => {
    expect(holdSeconds(100)).toBe(MIN_HOLD_SEC)
    expect(holdSeconds(1500)).toBe(1.5)
    expect(holdSeconds(undefined)).toBe(MIN_HOLD_SEC)
    const markers = buildPlayback(steps)
    expect(markers.map((m) => m.id)).toEqual(['s0', 's1'])
    expect(markers[0].at).toBe(0)
    expect(markers[0].dur).toBe(MIN_HOLD_SEC)
    expect(markers[1].at).toBe(MIN_HOLD_SEC)
    expect(markers[1].dur).toBe(1.5)
    expect(markers[0].shot).toBe(true)
    expect(playbackEnd(markers)).toBeCloseTo(MIN_HOLD_SEC + 1.5)
  })

  it('maps a scrub time onto the step that is playing', () => {
    const markers = buildPlayback(steps)
    expect(markerAtPlayback(markers, 0)?.id).toBe('s0')
    expect(markerAtPlayback(markers, MIN_HOLD_SEC - 0.01)?.id).toBe('s0')
    expect(markerAtPlayback(markers, MIN_HOLD_SEC)?.id).toBe('s1')
    expect(markerAtPlayback(markers, playbackEnd(markers))?.id).toBe('s1')
    expect(markerAtPlayback([], 0)).toBeNull()
  })

  it('advances with speed and stops at the end', () => {
    const end = 2
    expect(advancePlayback(0, 0.2, 2, end)).toEqual({ time: 0.4, done: false })
    expect(advancePlayback(1.9, 0.2, 1, end)).toEqual({ time: end, done: true })
    expect(advancePlayback(0, 0.1, 8, end).time).toBeCloseTo(0.8)
  })
})
