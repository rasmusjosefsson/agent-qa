// Replay scrubber — a recording (or its step frames) with one marker per
// step, a caption for wherever the playhead is, and playback up to 8×.
//
// Video mode asks the browser for that rate. Some engines clamp it; the
// control says so. Frames mode steps the captured screenshots on our own
// timer, so 8× still moves even when the video decoder will not.

import { useEffect, useMemo, useRef, useState } from 'react'
import {
  BracesIcon,
  CheckIcon,
  CircleIcon,
  ClockIcon,
  CompassIcon,
  KeyboardIcon,
  MousePointer2Icon,
  PauseIcon,
  PlayIcon,
} from 'lucide-react'
import { artifactUrl, runFileUrl } from '@/lib/runs-api'
import { cn } from '@/lib/utils'
import {
  PLAYBACK_RATES,
  buildMarkers,
  clampedRate,
  formatClock,
  frameStepMs,
  markerAtTime,
  trackDuration,
  type Marker,
  type TimelineStep,
  type VideoCue,
} from '../timeline'
import type { VerbCat } from '../rows'

function MarkerIcon({ cat, className }: { cat: VerbCat; className?: string }) {
  const props = { className: cn('size-3', className), strokeWidth: 2.25, 'aria-hidden': true }
  switch (cat) {
    case 'nav':
      return <CompassIcon {...props} />
    case 'click':
      return <MousePointer2Icon {...props} />
    case 'fill':
      return <BracesIcon {...props} />
    case 'press':
      return <KeyboardIcon {...props} />
    case 'assert':
      return <CheckIcon {...props} />
    case 'wait':
      return <ClockIcon {...props} />
    default:
      return <CircleIcon {...props} />
  }
}

function applyRate(el: HTMLVideoElement, rate: number): number | null {
  el.preservesPitch = true
  try {
    el.playbackRate = rate
  } catch {
    /* the element keeps whatever rate it already had */
  }
  return clampedRate(rate, el.playbackRate)
}

export function ReplayScrubber({
  sid,
  runId,
  steps,
  cues,
  videoFile,
  selectedIdx,
  onSelect,
}: {
  sid: string
  runId: string
  steps: TimelineStep[]
  cues?: VideoCue[] | null
  videoFile?: string | null
  selectedIdx: number | null
  onSelect: (idx: number) => void
}) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const trackRef = useRef<HTMLDivElement>(null)
  const fromPlayer = useRef(false)
  const [mode, setMode] = useState<'video' | 'frames'>('video')
  const [rate, setRate] = useState<number>(1)
  const [capped, setCapped] = useState<number | null>(null)
  const [playing, setPlaying] = useState(false)
  const [time, setTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const [file, setFile] = useState(videoFile || 'run.mp4')

  const markers = useMemo(() => buildMarkers(steps, cues, duration), [steps, cues, duration])
  const dur = trackDuration(markers, duration)
  const shown: Marker | null =
    mode === 'frames'
      ? (markers.find((m) => m.idx === selectedIdx) ?? markers[0] ?? null)
      : (markerAtTime(markers, time) ?? markers[0] ?? null)
  const head = mode === 'frames' && shown ? shown.at : time
  const pct = Math.min(100, Math.max(0, (head / dur) * 100))

  useEffect(() => {
    setFile(videoFile || 'run.mp4')
  }, [videoFile, runId])

  useEffect(() => {
    const el = videoRef.current
    if (!el || mode !== 'video') return
    setCapped(applyRate(el, rate))
  }, [rate, mode, duration])

  useEffect(() => {
    if (mode === 'frames') videoRef.current?.pause()
  }, [mode])

  const idxRef = useRef(selectedIdx)
  idxRef.current = selectedIdx
  const markersRef = useRef(markers)
  markersRef.current = markers
  const onSelectRef = useRef(onSelect)
  onSelectRef.current = onSelect
  // Stable across parent re-renders that rebuild the same marker list.
  const markerKey = markers.map((m) => `${m.idx}@${m.at.toFixed(3)}`).join('|')

  // A click in the step list seeks the recording. Selection that the
  // playhead itself just reported must not yank the playhead backwards
  // to the start of that step.
  useEffect(() => {
    if (fromPlayer.current) {
      fromPlayer.current = false
      return
    }
    if (selectedIdx == null) return
    const list = markersRef.current
    const i = list.findIndex((m) => m.idx === selectedIdx)
    if (i < 0) return
    const start = list[i].at
    const end = list[i + 1]?.at ?? Number.POSITIVE_INFINITY
    const now = videoRef.current?.currentTime ?? 0
    if (now >= start - 0.05 && now < end - 0.05) return
    setTime(start)
    const el = videoRef.current
    if (!el || mode !== 'video') return
    const seek = () => {
      if (Number.isFinite(el.duration) && el.duration > 0) {
        el.currentTime = Math.min(start, Math.max(0, el.duration - 0.001))
      } else {
        el.currentTime = start
      }
    }
    if (el.readyState >= 1) seek()
    else el.addEventListener('loadedmetadata', seek, { once: true })
  }, [selectedIdx, markerKey, mode])

  useEffect(() => {
    if (mode !== 'frames' || !playing) return
    const id = window.setInterval(() => {
      const list = markersRef.current
      if (!list.length) {
        setPlaying(false)
        return
      }
      const cur = idxRef.current
      const i = list.findIndex((m) => m.idx === cur)
      if (i < 0) {
        fromPlayer.current = true
        onSelectRef.current(list[0].idx)
        return
      }
      const next = list[i + 1]
      if (!next) {
        setPlaying(false)
        return
      }
      fromPlayer.current = true
      onSelectRef.current(next.idx)
    }, frameStepMs(rate))
    return () => window.clearInterval(id)
  }, [mode, playing, rate])

  function choose(idx: number) {
    fromPlayer.current = true
    onSelect(idx)
  }

  function seekSeconds(t: number) {
    const next = Math.min(Math.max(0, t), dur)
    setTime(next)
    const el = videoRef.current
    if (el && mode === 'video') {
      const cap = Number.isFinite(el.duration) && el.duration > 0 ? el.duration : next
      el.currentTime = Math.min(next, cap)
    }
    const hit = markerAtTime(markers, next)
    if (hit) choose(hit.idx)
  }

  function seekFromClientX(clientX: number) {
    const box = trackRef.current?.getBoundingClientRect()
    if (!box || box.width <= 0) return
    const frac = Math.min(1, Math.max(0, (clientX - box.left) / box.width))
    seekSeconds(frac * dur)
  }

  async function toggle() {
    if (mode === 'frames') {
      setPlaying((p) => !p)
      return
    }
    const el = videoRef.current
    if (!el) return
    if (el.paused) {
      setCapped(applyRate(el, rate))
      try {
        await el.play()
      } catch {
        setPlaying(false)
      }
    } else {
      el.pause()
    }
  }

  const frame = shown?.shot && shown.id ? artifactUrl(sid, runId, 'screenshots', shown.id) : null

  return (
    <div className="shrink-0 border-b border-border bg-card text-foreground">
      <div className="relative bg-muted">
        <video
          ref={videoRef}
          src={runFileUrl(sid, runId, file)}
          preload="metadata"
          playsInline
          className={cn('mx-auto max-h-[min(42vh,360px)] w-full bg-muted', mode !== 'video' && 'hidden')}
          onLoadedMetadata={(e) => {
            const el = e.currentTarget
            setDuration(Number.isFinite(el.duration) ? el.duration : 0)
            setCapped(applyRate(el, rate))
          }}
          onTimeUpdate={(e) => {
            if (mode !== 'video') return
            const t = e.currentTarget.currentTime
            setTime(t)
            const hit = markerAtTime(markersRef.current, t)
            if (hit && hit.idx !== idxRef.current) choose(hit.idx)
          }}
          onPlay={() => setPlaying(true)}
          onPause={() => setPlaying(false)}
          onEnded={() => setPlaying(false)}
          onError={() => {
            if (file !== 'run.webm') setFile('run.webm')
          }}
        />
        {mode === 'frames' &&
          (frame ? (
            <img
              src={frame}
              alt={shown?.label || 'Step frame'}
              className="mx-auto max-h-[min(42vh,360px)] w-full object-contain object-top"
            />
          ) : (
            <div className="grid h-36 place-items-center text-xs text-muted-foreground">
              {shown ? 'No frame captured for this step' : 'No frames in this run'}
            </div>
          ))}
      </div>

      <div className="px-3 pt-2.5">
        <div
          ref={trackRef}
          role="slider"
          tabIndex={0}
          aria-label="Replay timeline"
          aria-valuemin={0}
          aria-valuemax={Math.round(dur * 1000)}
          aria-valuenow={Math.round(head * 1000)}
          aria-valuetext={`${formatClock(head)} of ${formatClock(dur)}`}
          className="relative h-8 cursor-pointer touch-none outline-none focus-visible:ring-1 focus-visible:ring-primary/50"
          onPointerDown={(e) => {
            e.currentTarget.setPointerCapture(e.pointerId)
            seekFromClientX(e.clientX)
          }}
          onPointerMove={(e) => {
            if (e.currentTarget.hasPointerCapture(e.pointerId)) seekFromClientX(e.clientX)
          }}
          onKeyDown={(e) => {
            if (e.key === 'ArrowRight') seekSeconds(head + 1)
            else if (e.key === 'ArrowLeft') seekSeconds(head - 1)
            else if (e.key === 'Home') seekSeconds(0)
            else if (e.key === 'End') seekSeconds(dur)
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
          {markers.map((m) => {
            const active = shown?.idx === m.idx
            const left = Math.min(100, Math.max(0, (m.at / dur) * 100))
            return (
              <button
                key={m.idx}
                type="button"
                title={m.label}
                aria-label={m.label}
                aria-current={active ? 'true' : undefined}
                style={{ left: `${left}%` }}
                onPointerDown={(e) => e.stopPropagation()}
                onClick={(e) => {
                  e.stopPropagation()
                  seekSeconds(m.at)
                }}
                className={cn(
                  'absolute top-1/2 z-20 grid size-[18px] -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full border transition-transform',
                  active
                    ? 'size-5 border-primary bg-primary text-primary-foreground'
                    : m.status === 'fail'
                      ? 'border-destructive/70 bg-card text-destructive hover:bg-muted'
                      : 'border-border bg-card text-foreground hover:bg-muted'
                )}
              >
                <MarkerIcon cat={m.cat} className={active ? 'size-3' : 'size-2.5'} />
              </button>
            )
          })}
        </div>
        <div className="tnum pb-1 text-right font-mono text-[11px] text-muted-foreground">
          {formatClock(head)} / {formatClock(mode === 'video' && duration > 0 ? duration : dur)}
        </div>
      </div>

      <div className="flex items-center gap-2 border-t border-border px-3 py-2">
        <div className="flex min-w-0 flex-1 items-center gap-2 text-[13px]">
          {shown && (
            <span
              className={cn(
                'grid size-5 shrink-0 place-items-center rounded-full',
                shown.status === 'fail' ? 'bg-destructive/15 text-destructive' : 'bg-muted text-muted-foreground'
              )}
            >
              <MarkerIcon cat={shown.cat} />
            </span>
          )}
          <span className="truncate" title={shown?.label}>
            {shown?.label || 'Replay'}
          </span>
        </div>
        <button
          type="button"
          onClick={() => void toggle()}
          aria-label={playing ? 'Pause' : 'Play'}
          className="grid size-7 shrink-0 place-items-center rounded-md bg-muted text-foreground hover:bg-accent"
        >
          {playing ? <PauseIcon className="size-3.5" /> : <PlayIcon className="size-3.5 fill-current" />}
        </button>
        <label className="sr-only" htmlFor={`replay-rate-${runId}`}>
          Playback speed
        </label>
        <select
          id={`replay-rate-${runId}`}
          aria-label="Playback speed"
          value={rate}
          title={
            capped != null && mode === 'video'
              ? `This browser capped video playback at ${capped}x. Frames still steps at ${rate}x.`
              : 'Play faster. 8x asks the browser for eight times speed; Frames always steps at that rate.'
          }
          onChange={(e) => setRate(Number(e.target.value))}
          className="h-7 rounded-md border border-border bg-card px-1.5 text-xs font-medium text-foreground outline-none hover:bg-muted"
        >
          {PLAYBACK_RATES.map((n) => (
            <option key={n} value={n}>
              {n}x
            </option>
          ))}
        </select>
        {capped != null && mode === 'video' && (
          <span className="shrink-0 text-[10px] text-muted-foreground" data-qa-volatile>
            capped {capped}x
          </span>
        )}
        <div className="flex shrink-0 overflow-hidden rounded-md border border-border bg-muted text-[11px] font-medium">
          <button
            type="button"
            aria-pressed={mode === 'frames'}
            onClick={() => setMode('frames')}
            className={cn(
              'px-2.5 py-1',
              mode === 'frames' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:text-foreground'
            )}
          >
            Frames
          </button>
          <button
            type="button"
            aria-pressed={mode === 'video'}
            onClick={() => setMode('video')}
            className={cn(
              'px-2.5 py-1',
              mode === 'video' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:text-foreground'
            )}
          >
            Video
          </button>
        </div>
      </div>
    </div>
  )
}
