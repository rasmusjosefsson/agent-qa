import { describe, expect, it } from 'vitest'
import {
  buildMarkers,
  clampedRate,
  formatClock,
  frameStepMs,
  markerAtTime,
  trackDuration,
  type TimelineStep,
} from './timeline'

const steps: TimelineStep[] = [
  { idx: 1, id: 's1', intent: 'Open the homepage', kind: 'do:goto', status: 'pass', ms: 1000, screenshot: 'screenshots/s1.png' },
  { idx: 2, id: 's2', intent: 'Fill the email', kind: 'do:fill', status: 'pass', ms: 1000 },
  { idx: 3, id: 's3', intent: 'Verified', kind: 'check', status: 'fail', ms: 500, pending: false },
  { idx: 4, id: 's4', intent: 'not run', status: 'pending', pending: true, ms: 0 },
]

describe('buildMarkers', () => {
  it('uses recording cues as absolute seconds and skips pending steps', () => {
    const markers = buildMarkers(
      steps,
      [
        { id: 's1', idx: 1, tMs: 400 },
        { id: 's2', idx: 2, tMs: 1800 },
        { idx: 3, tMs: 3200 },
      ],
      5
    )
    expect(markers.map((m) => m.at)).toEqual([0.4, 1.8, 3.2])
    expect(markers.map((m) => m.cat)).toEqual(['nav', 'fill', 'assert'])
    expect(markers[0].shot).toBe(true)
    expect(markers[2].shot).toBe(false)
    expect(markers[2].status).toBe('fail')
  })

  it('puts leftover recording time ahead of the first step', () => {
    const markers = buildMarkers(steps, null, 4)
    // 1s + 1s + 0.5s of steps inside a 4s recording → 1.5s of lead-in.
    expect(markers.map((m) => m.at)).toEqual([1.5, 2.5, 3.5])
  })

  it('scales markers down when the steps outlast the recording', () => {
    const long: TimelineStep[] = [
      { idx: 1, id: 'a', intent: 'a', ms: 3000, status: 'pass' },
      { idx: 2, id: 'b', intent: 'b', ms: 3000, status: 'pass' },
    ]
    const markers = buildMarkers(long, null, 2)
    expect(markers[0].at).toBeCloseTo(0)
    expect(markers[1].at).toBeCloseTo(1)
  })
})

describe('markerAtTime', () => {
  const markers = buildMarkers(steps, [
    { idx: 1, tMs: 0 },
    { idx: 2, tMs: 2000 },
  ], 0)

  it('returns the step the playhead has reached', () => {
    expect(markerAtTime(markers, 0)?.idx).toBe(1)
    expect(markerAtTime(markers, 1.9)?.idx).toBe(1)
    expect(markerAtTime(markers, 2)?.idx).toBe(2)
  })

  it('is empty before the first cue', () => {
    const later = buildMarkers(steps, [{ idx: 1, tMs: 1500 }], 0)
    expect(markerAtTime(later, 0.2)).toBeNull()
  })
})

describe('clock and speed', () => {
  it('formats a clock like the scrubber', () => {
    expect(formatClock(0)).toBe('0:00')
    expect(formatClock(48)).toBe('0:48')
    expect(formatClock(75)).toBe('1:15')
    expect(formatClock(Number.NaN)).toBe('0:00')
  })

  it('steps frames faster at 8x without dropping the floor', () => {
    expect(frameStepMs(1)).toBe(720)
    expect(frameStepMs(8)).toBe(90)
    expect(frameStepMs(0)).toBe(720)
  })

  it('reports a clamped playback rate and stays quiet when the rate stuck', () => {
    expect(clampedRate(8, 8)).toBeNull()
    expect(clampedRate(8, 4)).toBe(4)
    expect(clampedRate(8, Number.NaN)).toBeNull()
  })

  it('keeps the track at least as long as the last marker', () => {
    const markers = buildMarkers(steps, [{ idx: 1, tMs: 9000 }], 0)
    expect(trackDuration(markers, 0)).toBeGreaterThanOrEqual(9)
    expect(trackDuration([], 3)).toBe(3)
  })
})
