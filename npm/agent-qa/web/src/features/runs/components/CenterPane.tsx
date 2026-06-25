// web/src/features/runs/components/CenterPane.tsx
import { cn } from '@/lib/utils'
import {
  cleanSummary,
  collapseEvents,
  fmtMs,
  icon,
  isRunLive,
  mergeRows,
  stepText,
  verbBadge,
  verbCat,
  type VerbCat,
} from '../rows'
import type { RunsApi } from '../useRuns'

const VERB_TONE: Record<VerbCat, string> = {
  nav: 'bg-sky-500/15 text-sky-400',
  click: 'bg-indigo-500/15 text-indigo-400',
  fill: 'bg-teal-500/15 text-teal-400',
  press: 'bg-violet-500/15 text-violet-400',
  assert: 'bg-rose-500/15 text-rose-400',
  wait: 'bg-zinc-500/15 text-zinc-300',
  action: 'bg-zinc-500/15 text-zinc-300',
}

const STATUS_TONE: Record<string, string> = {
  pass: 'text-emerald-400',
  fail: 'text-destructive',
  running: 'text-amber-400',
  pending: 'text-muted-foreground',
}

function VerbBadge({ verb }: { verb: unknown }) {
  const cat = verbCat(verb)
  return (
    <span className={cn('shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium', VERB_TONE[cat])}>
      {verbBadge(verb)}
    </span>
  )
}

export function CenterPane({ runs, onReplay }: { runs: RunsApi; onReplay: (sid: string) => void }) {
  const { detail, scenarioDef, sel, runDefSteps } = runs

  // Mode A — a recorded scenario is previewed (no run selected).
  if (scenarioDef && !detail) {
    const steps = scenarioDef.steps || []
    return (
      <Pane>
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{scenarioDef.intent || scenarioDef.id || 'Scenario'}</h2>
            <div className="text-xs text-muted-foreground">
              {steps.length} step{steps.length === 1 ? '' : 's'} · recorded · not yet replayed
            </div>
          </div>
          {sel.sid && (
            <button
              type="button"
              onClick={() => onReplay(sel.sid!)}
              className="shrink-0 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:opacity-90"
            >
              ▶ Replay
            </button>
          )}
        </div>
        <ol className="min-h-0 flex-1 overflow-auto p-2">
          {steps.map((st, i) => (
            <li key={i} className="flex items-center gap-2 rounded-md px-2 py-1.5">
              <span className="w-5 text-right text-xs text-muted-foreground">{i}</span>
              <VerbBadge verb={st.verb} />
              <span className="truncate text-sm">{st.intent || stepText(st)}</span>
            </li>
          ))}
          {steps.length === 0 && <li className="px-2 py-3 text-xs text-muted-foreground">This scenario has no steps.</li>}
        </ol>
      </Pane>
    )
  }

  // Mode B — a run is selected.
  if (detail) {
    const live = isRunLive(detail)
    const a = detail.audit || {}
    const s = detail.status || {}
    const summary = a.summary || (live ? `running ${s.currentIdx || 0}/${s.total || '?'}` : 'in flight')
    const tone = /PASS/.test(summary) ? 'pass' : /FAIL/.test(summary) ? 'fail' : 'running'
    const events = collapseEvents(detail.events || [])
    const currentIdx = typeof s.currentIdx === 'number' ? s.currentIdx : -1
    const liveCurrent = live ? currentIdx : -1
    const defSteps = runDefSteps.sid === sel.sid ? runDefSteps.steps : null
    const rows = mergeRows(events, defSteps)

    return (
      <Pane>
        <div className="border-b border-border px-4 py-3">
          <div className="flex items-center gap-2">
            <span
              className={cn(
                'rounded px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide',
                tone === 'pass' && 'bg-emerald-500/15 text-emerald-400',
                tone === 'fail' && 'bg-destructive/15 text-destructive',
                tone === 'running' && 'bg-amber-500/15 text-amber-400'
              )}
            >
              {cleanSummary(summary)}
            </span>
            {live && <span className="text-xs text-amber-400">● live</span>}
          </div>
          <div className="mt-0.5 font-mono text-xs text-muted-foreground">{detail.runId}</div>
        </div>
        <ol className="min-h-0 flex-1 overflow-auto p-2">
          {rows.map((st) => {
            const selected = sel.stepIdx === st.idx
            const isCurrent = st.idx === liveCurrent
            const pending = st.status === 'pending'
            return (
              <li key={st.idx}>
                <button
                  type="button"
                  disabled={pending}
                  onClick={pending ? undefined : () => runs.selectStep(st.idx)}
                  className={cn(
                    'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left',
                    selected && 'bg-muted',
                    isCurrent && 'ring-1 ring-amber-400/40',
                    pending ? 'opacity-50' : 'hover:bg-muted/50'
                  )}
                >
                  <span className={cn('w-4 text-center', STATUS_TONE[st.status || ''] || 'text-muted-foreground')}>
                    {icon(st.status)}
                  </span>
                  <span className="w-12 shrink-0 text-xs text-muted-foreground">
                    {st.idx}/{st.total || rows.length}
                  </span>
                  <span className="truncate text-sm">{st.intent || st.id}</span>
                  {st.kind && <span className="shrink-0 text-xs text-muted-foreground">({st.kind})</span>}
                  <span className="ml-auto shrink-0 text-xs text-muted-foreground">{pending ? '' : fmtMs(st.ms)}</span>
                </button>
              </li>
            )
          })}
          {rows.length === 0 && <li className="px-2 py-3 text-xs text-muted-foreground">No steps recorded for this run.</li>}
        </ol>
      </Pane>
    )
  }

  return (
    <Pane>
      <div className="flex flex-1 items-center justify-center p-8 text-sm text-muted-foreground">
        Select a run from the left.
      </div>
    </Pane>
  )
}

function Pane({ children }: { children: React.ReactNode }) {
  return <section className="flex h-full min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">{children}</section>
}
