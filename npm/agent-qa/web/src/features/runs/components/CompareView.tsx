// web/src/features/runs/components/CompareView.tsx
// Per-step diff between two replay runs of one scenario, produced by
// `agent-qa compare` on the server. Replaces the step list in the center pane
// while active — the run header stays so context isn't lost.
import { useState } from 'react'
import { cn } from '@/lib/utils'
import { compareShotUrl } from '@/lib/runs-api'
import { ChevronDownIcon, ChevronRightIcon, XIcon } from 'lucide-react'
import { relRunTime } from '../rows'
import type { RunsApi } from '../useRuns'

const OUTCOME_TONE: Record<string, string> = {
  CHANGED: 'bg-amber-500/15 text-amber-400',
  'ONLY-A': 'bg-sky-500/15 text-sky-400',
  'ONLY-B': 'bg-sky-500/15 text-sky-400',
}

function OutcomeBadge({ outcome }: { outcome: string }) {
  return (
    <span
      className={cn(
        'shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide',
        OUTCOME_TONE[outcome] || 'bg-muted text-muted-foreground'
      )}
    >
      {outcome === 'ONLY-A' ? 'only A' : outcome === 'ONLY-B' ? 'only B' : outcome}
    </span>
  )
}

export function CompareView({ runs }: { runs: RunsApi }) {
  const r = runs.compare!
  const [openStep, setOpenStep] = useState<string | null>(null)
  const [openShot, setOpenShot] = useState<string | null>(null)
  const changed = r.snapshots.filter((s) => s.outcome !== 'SAME').length
  const shotsChanged = r.screenshots.filter((s) => s.outcome !== 'SAME').length

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex items-center gap-2 border-b border-border px-3 py-2">
        <div className="min-w-0 flex-1 text-xs text-muted-foreground">
          <span className="font-medium text-foreground">compare</span>{' '}
          <span title={r.runA || ''}>{r.runA ? relRunTime(r.runA) : '?'}</span>
          <span className="mx-1 opacity-60">→</span>
          <span title={r.runB || ''}>{r.runB ? relRunTime(r.runB) : '?'}</span>
          <span className="ml-2 opacity-70">
            {changed} changed snapshot{changed === 1 ? '' : 's'} · {shotsChanged} changed screenshot
            {shotsChanged === 1 ? '' : 's'}
          </span>
        </div>
        <button
          type="button"
          onClick={runs.clearCompare}
          title="Back to the run"
          aria-label="Close compare view"
          className="grid size-6 shrink-0 place-items-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          <XIcon className="size-3.5" />
        </button>
      </div>
      <div className="min-h-0 flex-1 space-y-3 overflow-auto p-2.5">
        <section>
          <div className="px-1 pb-1 text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
            Snapshots
          </div>
          <ul className="space-y-px">
            {r.snapshots.map((e) => {
              const open = openStep === e.stepId
              const expandable = e.outcome === 'CHANGED' && !!e.diff
              return (
                <li key={e.stepId}>
                  <button
                    type="button"
                    disabled={!expandable}
                    onClick={() => setOpenStep(open ? null : e.stepId)}
                    className={cn(
                      'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors',
                      expandable ? 'hover:bg-muted/60' : 'opacity-80'
                    )}
                  >
                    {expandable ? (
                      open ? (
                        <ChevronDownIcon className="size-3 shrink-0 text-muted-foreground" />
                      ) : (
                        <ChevronRightIcon className="size-3 shrink-0 text-muted-foreground" />
                      )
                    ) : (
                      <span className="size-3 shrink-0" />
                    )}
                    <span className="w-14 shrink-0 font-mono text-[11px] text-muted-foreground">{e.stepId}</span>
                    <OutcomeBadge outcome={e.outcome} />
                  </button>
                  {open && e.diff && (
                    <pre className="mx-2 mb-1 max-h-72 overflow-auto rounded-md border border-border bg-muted/40 p-2 font-mono text-[11px] leading-relaxed">
                      {e.diff}
                    </pre>
                  )}
                </li>
              )
            })}
            {r.snapshots.length === 0 && (
              <li className="px-2 py-2 text-xs text-muted-foreground">No snapshots in either run.</li>
            )}
          </ul>
        </section>
        {r.screenshots.length > 0 && (
          <section>
            <div className="px-1 pb-1 text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
              Screenshots
            </div>
            <ul className="space-y-px">
              {r.screenshots.map((s) => {
                const open = openShot === s.stepId
                return (
                  <li key={s.stepId}>
                    <button
                      type="button"
                      disabled={!s.hasDiffPng}
                      onClick={() => setOpenShot(open ? null : s.stepId)}
                      className={cn(
                        'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors',
                        s.hasDiffPng ? 'hover:bg-muted/60' : 'opacity-80'
                      )}
                    >
                      {s.hasDiffPng ? (
                        open ? (
                          <ChevronDownIcon className="size-3 shrink-0 text-muted-foreground" />
                        ) : (
                          <ChevronRightIcon className="size-3 shrink-0 text-muted-foreground" />
                        )
                      ) : (
                        <span className="size-3 shrink-0" />
                      )}
                      <span className="w-14 shrink-0 font-mono text-[11px] text-muted-foreground">{s.stepId}</span>
                      <OutcomeBadge outcome={s.outcome} />
                      {s.differingPixels !== null && (
                        <span className="tnum text-[11px] text-muted-foreground">
                          {(s.differingPixels * 100).toFixed(2)}% pixels
                        </span>
                      )}
                    </button>
                    {open && s.hasDiffPng && (
                      <a
                        href={compareShotUrl(r.sid, r.folder, s.stepId)}
                        target="_blank"
                        rel="noreferrer"
                        className="mx-2 mb-1 block"
                        title="Open full-size in a new tab"
                      >
                        <img
                          src={compareShotUrl(r.sid, r.folder, s.stepId)}
                          alt={`pixel diff for ${s.stepId}`}
                          className="max-h-80 rounded-md border border-border"
                        />
                      </a>
                    )}
                  </li>
                )
              })}
            </ul>
          </section>
        )}
      </div>
    </div>
  )
}
