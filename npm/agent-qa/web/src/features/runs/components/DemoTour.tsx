import { useEffect, useState } from 'react'
import { DEMO_QUICKSTART, TOUR_STEPS } from '../demo-tour'
import type { RunsApi } from '../useRuns'
import { getRunDetail } from '@/lib/runs-api'
import { hasPageFrame } from '../shot-evidence'
import type { RunEvent } from '../types'

function terminalEvents(events: RunEvent[]): RunEvent[] {
  const byIdx = new Map<number, RunEvent>()
  for (const event of events) {
    if (typeof event.idx !== 'number') continue
    const prev = byIdx.get(event.idx)
    if (!prev || event.status !== 'running') byIdx.set(event.idx, event)
  }
  return [...byIdx.values()].sort((a, b) => a.idx - b.idx)
}

/** Stay on a passing run's page frames. A failure still opens on the miss. */
export function tourStepEvent(events: RunEvent[], tourId: string): RunEvent | undefined {
  const steps = terminalEvents(events)
  const failed = steps.find((step) => step.status === 'fail')
  if (failed) return failed
  const pages = steps.filter((step) => hasPageFrame(step))
  if (!pages.length) return steps[0]
  if (tourId === 'playback' && pages.length > 2) return pages[Math.floor(pages.length / 2)]
  return pages[0]
}

export function DemoTour({ runs, onClose }: { runs: RunsApi; onClose: () => void }) {
  const [index, setIndex] = useState(0)
  const step = TOUR_STEPS[index]

  useEffect(() => {
    let alive = true
    void (async () => {
      if (step.id === 'runs' || step.id === 'local') return
      const currentSid = runs.sel.sid
      const currentRun = runs.sel.runId
      const sc =
        (currentSid && runs.scenarios.find((row) => row.sid === currentSid)) ||
        runs.scenarios.find((row) => row.latestRun?.ok === true || /PASS/.test(row.latestRun?.summary || '')) ||
        runs.scenarios[0]
      const runId = (sc && sc.sid === currentSid && currentRun) || sc?.latestRunId
      if (!sc || !runId) return
      if (!runs.expanded.has(sc.sid)) await runs.toggleScenario(sc.sid)
      if (!alive) return
      if (runs.sel.sid !== sc.sid || runs.sel.runId !== runId) await runs.selectRun(sc.sid, runId)
      if (!alive) return
      const detail = runs.detail?.runId === runId ? runs.detail : await getRunDetail(sc.sid, runId)
      const shown = tourStepEvent(detail.events, step.id)
      if (shown && typeof shown.idx === 'number') runs.selectStep(shown.idx)
    })()
    return () => {
      alive = false
    }
    // Selection helpers change identity every render. The step index is the trigger.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [index])

  useEffect(() => {
    const targetId = step.id === 'checks' || step.id === 'heals' ? 'steps' : step.id === 'diffs' ? 'evidence' : step.id
    const target = document.querySelector(`[data-tour="${targetId}"]`)
    document.querySelectorAll('[data-tour-current]').forEach((el) => {
      el.removeAttribute('data-tour-current')
      if (el instanceof HTMLElement) el.style.outline = ''
    })
    if (target instanceof HTMLElement) {
      target.setAttribute('data-tour-current', 'true')
      target.style.outline = '2px solid var(--primary, #5b4dff)'
      target.style.outlineOffset = '2px'
      target.scrollIntoView({ block: 'nearest' })
    }
    return () => {
      document.querySelectorAll('[data-tour-current]').forEach((el) => {
        el.removeAttribute('data-tour-current')
        if (el instanceof HTMLElement) el.style.outline = ''
      })
    }
  }, [step.id, runs.detail, runs.sel.stepIdx])

  return (
    <div
      role="dialog"
      aria-modal="false"
      aria-label="Demo tour"
      className="pointer-events-auto fixed bottom-4 left-1/2 z-50 w-[min(32rem,calc(100vw-2rem))] -translate-x-1/2 rounded-xl border border-border bg-card p-4 text-foreground shadow-lg"
    >
      <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
        Step {index + 1} of {TOUR_STEPS.length}
      </div>
      <h2 className="mt-1 text-sm font-semibold">{step.title}</h2>
      <p className="mt-1 text-[13px] leading-relaxed text-muted-foreground">{step.body}</p>
      {step.id === 'local' && (
        <a href={DEMO_QUICKSTART} className="mt-2 inline-block text-[13px] font-medium text-primary underline-offset-2 hover:underline">
          Open the quickstart
        </a>
      )}
      <div className="mt-3 flex items-center justify-between gap-2">
        <button type="button" onClick={onClose} className="text-xs text-muted-foreground hover:text-foreground">
          Close
        </button>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => setIndex((n) => Math.max(0, n - 1))}
            disabled={index === 0}
            className="rounded-md border border-border px-2.5 py-1 text-xs font-medium disabled:opacity-40"
          >
            Back
          </button>
          {index < TOUR_STEPS.length - 1 ? (
            <button
              type="button"
              onClick={() => setIndex((n) => n + 1)}
              className="rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground"
            >
              Next
            </button>
          ) : (
            <button
              type="button"
              onClick={onClose}
              className="rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground"
            >
              Done
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
