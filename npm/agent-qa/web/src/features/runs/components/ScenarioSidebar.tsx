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
import { cleanSummary, scenarioVerdict, verdictTone } from '../rows'
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
      <div className="flex items-center justify-between border-b border-border px-3 py-2">
        <span className="text-xs font-medium text-muted-foreground">Scenarios</span>
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
      <ul className="min-h-0 flex-1 overflow-auto p-1.5">
        {scenarios.length === 0 && (
          <li className="px-2 py-3 text-xs text-muted-foreground">No scenarios under the root yet.</li>
        )}
        {scenarios.map((sc) => {
          const verdict = scenarioVerdict(sc.latestRun)
          const open = expanded.has(sc.sid)
          const list = runsBySid[sc.sid]
          return (
            <li key={sc.sid} className="group mb-0.5">
              <div className="flex items-stretch rounded-md hover:bg-muted/50">
                <button
                  type="button"
                  onClick={() => void runs.toggleScenario(sc.sid)}
                  className="min-w-0 flex-1 px-2 py-1.5 text-left"
                >
                  <div className="flex items-center gap-2">
                    <span className="truncate text-sm">{sc.scenarioId || sc.sid}</span>
                    {verdict && <Badge tone={verdict}>{verdict}</Badge>}
                  </div>
                  <div className="truncate text-xs text-muted-foreground">{sc.intent || sc.sid}</div>
                </button>
                <AlertDialog>
                  <AlertDialogTrigger asChild>
                    <button
                      type="button"
                      aria-label="Delete scenario"
                      title="Delete scenario"
                      className="my-1 mr-1 grid shrink-0 place-items-center rounded p-1 text-muted-foreground/40 opacity-0 transition hover:bg-muted hover:text-destructive group-hover:opacity-100"
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
                        <button
                          key={r.runId}
                          type="button"
                          onClick={() => void runs.selectRun(sc.sid, r.runId)}
                          className={cn(
                            'flex w-full items-center gap-2 rounded px-1.5 py-1 text-left',
                            selected ? 'bg-muted' : 'hover:bg-muted/50'
                          )}
                        >
                          <Badge tone={tone}>{label}</Badge>
                          <span className="truncate font-mono text-[10px] text-muted-foreground">{r.runId}</span>
                        </button>
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
