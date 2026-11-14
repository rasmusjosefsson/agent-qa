// Play / pause / scrub / speed for a finished run that has no video.
// The clock is the collapsed events.jsonl trace (see playback.ts).

import { useEffect, useMemo, useRef, useState } from 'react'
import { PauseIcon, PlayIcon } from 'lucide-react'
import { artifactUrl } from '@/lib/runs-api'
import { formatClock } from '../timeline'
import {
  advancePlayback,
  buildPlayback,
  markerAtPlayback,
  playbackEnd,
  PLAYBACK_SPEEDS,
  type PlaybackMarker,
} from '../playback'
import type { RunEvent } from '../types'

export function EventPlayback({
  sid,
  runId,
  steps,
  selectedIdx,
  onSelect,
}: {
  sid: string
  runId: string
  steps: RunEvent[]
  selectedIdx: number | null
  onSelect: (idx: number) => void
}) {
  const markers = useMemo(() => buildPlayback(steps), [steps])
  const end = playbackEnd(markers)
  const [time, setTime] = useState(0)
  const [playing, setPlaying] = useState(false)
  const [rate, setRate] = useState<number>(1)
  const trackRef = useRef<HTMLDivElement>(null)
  const timeRef = useRef(time)
  const rateRef = useRef(rate)
  const fromPlayer = useRef(false)
  const selectedRef = useRef(selectedIdx)
  const onSelectRef = useRef(onSelect)
  timeRef.current = time
  rateRef.current = rate
  selectedRef.current = selectedIdx
  onSelectRef.current = onSelect

  const shown: PlaybackMarker | null = markerAtPlayback(markers, time)
  const pct = end > 0 ? Math.min(100, Math.max(0, (time / end) * 100)) : 0

  useEffect(() => {
    setTime(0)
    setPlaying(false)
  }, [runId])

  useEffect(() => {
    if (!playing) return
    let raf = 0
    let last = performance.now()
    const tick = (now: number) => {
      const dt = (now - last) / 1000
      last = now
      const next = advancePlayback(timeRef.current, dt, rateRef.current, end)
      timeRef.current = next.time
      setTime(next.time)
      const hit = markerAtPlayback(markers, next.time)
      if (hit && hit.idx !== selectedRef.current) {
        fromPlayer.current = true
        onSelectRef.current(hit.idx)
      }
      if (next.done) {
        setPlaying(false)
        return
      }
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [playing, end, markers])

  // A click in the step list seeks the trace. A selection the playhead
  // just reported must not jump back to the start of that step.
  useEffect(() => {
    if (fromPlayer.current) {
      fromPlayer.current = false
      return
    }
    if (selectedIdx == null) return
    const marker = markers.find((m) => m.idx === selectedIdx)
    if (!marker) return
    const now = timeRef.current
    if (now >= marker.at && now < marker.at + marker.dur) return
    setTime(marker.at)
  }, [selectedIdx, markers])

  function seek(next: number) {
    const clamped = Math.min(Math.max(0, next), end)
    setTime(clamped)
    const hit = markerAtPlayback(markers, clamped)
    if (hit) {
      fromPlayer.current = true
      onSelect(hit.idx)
    }
  }

  function seekFromClientX(clientX: number) {
    const box = trackRef.current?.getBoundingClientRect()
    if (!box || box.width <= 0 || end <= 0) return
    const frac = Math.min(1, Math.max(0, (clientX - box.left) / box.width))
    seek(frac * end)
  }

  const frame = shown?.shot && shown.id ? artifactUrl(sid, runId, 'screenshots', shown.id) : null

  return (
    <div className="shrink-0 border-b border-border bg-card text-foreground" data-testid="event-playback">
      <div className="relative bg-muted">
        {frame ? (
          <img
            src={frame}
            alt={shown?.label || 'Step frame'}
            className="mx-auto max-h-[min(42vh,360px)] w-full object-contain object-top"
          />
        ) : (
          <div className="grid h-36 place-items-center text-xs text-muted-foreground">
            {shown ? 'No frame captured for this step' : 'No events in this run'}
          </div>
        )}
      </div>
      <div className="px-3 pt-2.5">
        <div
          ref={trackRef}
          role="slider"
          tabIndex={0}
          aria-label="Event timeline"
          aria-valuemin={0}
          aria-valuemax={Math.round(end * 1000)}
          aria-valuenow={Math.round(time * 1000)}
          aria-valuetext={`${formatClock(time)} of ${formatClock(end)}`}
          className="relative h-8 cursor-pointer touch-none outline-none focus-visible:ring-1 focus-visible:ring-primary/50"
          onPointerDown={(e) => {
            e.currentTarget.setPointerCapture(e.pointerId)
            seekFromClientX(e.clientX)
          }}
          onPointerMove={(e) => {
            if (e.currentTarget.hasPointerCapture(e.pointerId)) seekFromClientX(e.clientX)
          }}
          onKeyDown={(e) => {
            if (e.key === 'ArrowRight') seek(time + 0.5)
            else if (e.key === 'ArrowLeft') seek(time - 0.5)
            else if (e.key === 'Home') seek(0)
            else if (e.key === 'End') seek(end)
            else return
            e.preventDefault()
          }}
        >
          <div className="absolute top-1/2 right-0 left-0 h-[3px] -translate-y-1/2 rounded-full bg-border">
            <div className="h-full rounded-full bg-primary" style={{ width: `${pct}%` }} />
          </div>
          <div
            className="pointer-events-none absolute top-1/2 z-10 h-3.5 w-1 -translate-x-1/2 -translate-y-1/2 rounded-full bg-foreground shadow-sm"
            style={{ left: `${pct}%` }}
          />
        </div>
        <div className="tnum pb-1 text-right font-mono text-[11px] text-muted-foreground">
          {formatClock(time)} / {formatClock(end)}
        </div>
      </div>
      <div className="flex items-center gap-2 border-t border-border px-3 py-2">
        <span className="min-w-0 flex-1 truncate text-[13px]" title={shown?.label}>
          {shown?.label || 'Events'}
        </span>
        <button
          type="button"
          onClick={() => {
            if (time >= end && end > 0) setTime(0)
            setPlaying((p) => !p)
          }}
          aria-label={playing ? 'Pause' : 'Play'}
          className="grid size-7 shrink-0 place-items-center rounded-md bg-muted text-foreground hover:bg-accent"
        >
          {playing ? <PauseIcon className="size-3.5" /> : <PlayIcon className="size-3.5 fill-current" />}
        </button>
        <label className="sr-only" htmlFor={`event-rate-${runId}`}>
          Playback speed
        </label>
        <select
          id={`event-rate-${runId}`}
          aria-label="Playback speed"
          value={rate}
          onChange={(e) => setRate(Number(e.target.value))}
          className="h-7 rounded-md border border-border bg-card px-1.5 text-xs font-medium text-foreground outline-none hover:bg-muted"
        >
          {PLAYBACK_SPEEDS.map((n) => (
            <option key={n} value={n}>
              {n}x
            </option>
          ))}
        </select>
      </div>
    </div>
  )
}
