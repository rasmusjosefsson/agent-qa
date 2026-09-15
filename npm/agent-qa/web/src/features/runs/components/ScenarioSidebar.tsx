// web/src/features/runs/components/ScenarioSidebar.tsx
import { cn } from '@/lib/utils'
import { RefreshCwIcon, Trash2Icon } from 'lucide-react'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { cleanSummary, fmtRunTime, relRunTime, scenarioVerdict, verdictTone } from '../rows'
import type { RunsApi } from '../useRuns'

const TONE: Record<string, string> = {
  pass: 'bg-emerald-500/15 text-emerald-400',
  fail: 'bg-destructive/15 text-destructive',
  running: 'bg-amber-500/15 text-amber-400',
}

function Badge({ tone, children }: { tone: string; children: React.ReactNode }) {
  return (
    <span className={cn('rounded px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide', TONE[tone] || 'bg-muted text-muted-foreground')}>
      {children}
    </span>
  )
}

export function ScenarioSidebar({ runs }: { runs: RunsApi }) {
  const { scenarios, expanded, runsBySid, sel } = runs
  return (
    <nav className="flex h-full min-h-0 flex-col overflow-hidden">
      <div className="flex items-center justify-between border-b border-border px-3 py-2.5">
        <span className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">Scenarios</span>
        <button
          type="button"
          title="Refresh"
          aria-label="Refresh"
          onClick={runs.refresh}
          className="grid h-5 w-5 place-items-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          <RefreshCwIcon className="size-3.5" />
        </button>
      </div>
      <ul className="min-h-0 flex-1 space-y-0.5 overflow-auto p-2">
        {scenarios.length === 0 && (
          <li className="flex flex-col items-center gap-1.5 px-3 py-10 text-center">
            <div className="text-[13px] font-semibold tracking-tight">No scenarios yet</div>
            <div className="max-w-[16rem] text-xs leading-relaxed text-muted-foreground">
              Record your first scenario from Chat or the CLI — it will show up here.
            </div>
          </li>
        )}
        {scenarios.map((sc) => {
          const verdict = scenarioVerdict(sc.latestRun)
          const open = expanded.has(sc.sid)
          const list = runsBySid[sc.sid]
          return (
            <li key={sc.sid} className="group">
              <div className="flex items-center rounded-lg transition-colors hover:bg-muted/60">
                <button
                  type="button"
                  onClick={() => void runs.toggleScenario(sc.sid)}
                  className="min-w-0 flex-1 rounded-lg px-2.5 py-2 text-left focus-visible:outline-2 focus-visible:outline-ring"
                >
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13px] font-medium leading-tight">{sc.intent || sc.scenarioId || sc.sid}</span>
                    {verdict && <Badge tone={verdict}>{verdict}</Badge>}
                  </div>
                  <div className="tnum mt-0.5 truncate text-[11px] text-muted-foreground/80" title={fmtRunTime(sc.sid)}>
                    {relRunTime(sc.sid)}
                  </div>
                </button>
                <AlertDialog>
                  <AlertDialogTrigger asChild>
                    <button
                      type="button"
                      aria-label="Delete scenario"
                      title="Delete scenario"
                      className="mr-1 grid size-6 shrink-0 place-items-center rounded text-muted-foreground/40 opacity-0 transition hover:bg-muted hover:text-destructive group-hover:opacity-100"
                    >
                      <Trash2Icon className="size-3.5" />
                    </button>
                  </AlertDialogTrigger>
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>Delete this scenario?</AlertDialogTitle>
                      <AlertDialogDescription>
                        Permanently removes “{sc.scenarioId || sc.sid}” and all its replay runs. This can’t be undone.
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel>Cancel</AlertDialogCancel>
                      <AlertDialogAction onClick={() => void runs.deleteScenario(sc.sid)}>Delete</AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
              </div>
              {open && (
                <div className="ml-2 mt-0.5 border-l border-border pl-2">
                  {!list && <div className="px-1 py-1 text-xs text-muted-foreground">loading…</div>}
                  {list && list.length === 0 && <div className="px-1 py-1 text-xs text-muted-foreground">No replays yet</div>}
                  {list &&
                    [...list].reverse().map((r) => {
                      const selected = sel.sid === sc.sid && sel.runId === r.runId
                      const tone = r.state === 'running' ? 'running' : verdictTone(r.summary, r.state)
                      const label = r.state === 'running' ? 'running' : r.summary ? cleanSummary(r.summary) : 'in flight'
                      return (
                        <div
                          key={r.runId}
                          className={cn(
                            'group/run flex items-center rounded',
                            selected ? 'bg-muted' : 'hover:bg-muted/50'
                          )}
                        >
                          <button
                            type="button"
                            onClick={() => void runs.selectRun(sc.sid, r.runId)}
                            className="flex min-w-0 flex-1 items-center gap-2 px-1.5 py-1 text-left"
                          >
                            <Badge tone={tone}>{label}</Badge>
                            <span
                              className="truncate text-[11px] text-muted-foreground"
                              title={fmtRunTime(r.runId)}
                            >
                              {relRunTime(r.runId)}
                            </span>
                          </button>
                          <AlertDialog>
                            <AlertDialogTrigger asChild>
                              <button
                                type="button"
                                aria-label="Delete run"
                                title="Delete run"
                                className="mr-1 grid size-5 shrink-0 place-items-center rounded text-muted-foreground/40 opacity-0 transition hover:bg-muted hover:text-destructive group-hover/run:opacity-100"
                              >
                                <Trash2Icon className="size-3" />
                              </button>
                            </AlertDialogTrigger>
                            <AlertDialogContent>
                              <AlertDialogHeader>
                                <AlertDialogTitle>Delete this run?</AlertDialogTitle>
                                <AlertDialogDescription>
                                  Permanently removes the replay from {fmtRunTime(r.runId)}. This can’t be undone.
                                </AlertDialogDescription>
                              </AlertDialogHeader>
                              <AlertDialogFooter>
                                <AlertDialogCancel>Cancel</AlertDialogCancel>
                                <AlertDialogAction onClick={() => void runs.deleteRun(sc.sid, r.runId)}>Delete</AlertDialogAction>
                              </AlertDialogFooter>
                            </AlertDialogContent>
                          </AlertDialog>
                        </div>
                      )
                    })}
                </div>
              )}
            </li>
          )
        })}
      </ul>
    </nav>
  )
}
