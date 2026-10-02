// web/src/features/runs/components/ScenarioSidebar.tsx
import { useState } from 'react'
import { cn } from '@/lib/utils'
import { GlobeIcon, RefreshCwIcon, Trash2Icon } from 'lucide-react'
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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { cleanSummary, fmtRunTime, relRunTime, scenarioVerdict, verdictTone } from '../rows'
import type { RunsApi } from '../useRuns'

const TONE: Record<string, string> = {
  pass: 'bg-success/15 text-success',
  fail: 'bg-destructive/15 text-destructive',
  running: 'bg-warning/15 text-warning',
  flaky: 'bg-warning/15 text-warning',
  slow: 'bg-info/15 text-info',
  chronic: 'bg-violet-500/12 text-violet-600 dark:text-violet-300',
}

function HealthBadges({ sid, runs }: { sid: string; runs: RunsApi }) {
  const h = runs.healthBySid[sid]
  if (!h) return null
  const pills: Array<{ tone: string; label: string; steps: string[] }> = [
    { tone: 'flaky', label: 'flaky', steps: h.flaky },
    { tone: 'slow', label: 'slow', steps: h.slow },
    { tone: 'chronic', label: 'chronic', steps: h.chronic },
  ].filter((p) => p.steps.length > 0)
  if (!pills.length) return null
  return (
    <>
      {pills.map((p) => (
        <span key={p.tone} data-qa-volatile title={`audit ${p.label === 'chronic' ? 'heal-chronic' : p.label}: ${p.steps.join(', ')}`}>
          <Badge tone={p.tone}>
            {p.steps.length} {p.label}
          </Badge>
        </span>
      ))}
    </>
  )
}

function Badge({ tone, title, children }: { tone: string; title?: string; children: React.ReactNode }) {
  return (
    <span data-qa-volatile title={title} className={cn('shrink-0 whitespace-nowrap rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide', TONE[tone] || 'bg-muted text-muted-foreground')}>
      {children}
    </span>
  )
}

function CrawlDialog({ runs }: { runs: RunsApi }) {
  const [open, setOpen] = useState(false)
  const [url, setUrl] = useState('')
  const [busy, setBusy] = useState(false)
  const [note, setNote] = useState<string | null>(null)
  const submit = async () => {
    setBusy(true)
    setNote(null)
    const r = await runs.crawl(url.trim())
    setBusy(false)
    if (!r.ok) {
      setNote(r.error || 'crawl failed')
      return
    }
    setOpen(false)
    setUrl('')
    setNote(null)
  }
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <button
          type="button"
          title="Crawl a URL into a draft scenario"
          aria-label="Crawl a URL into a draft scenario"
          className="grid h-5 w-5 place-items-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          <GlobeIcon className="size-3.5" />
        </button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Crawl a page</DialogTitle>
          <DialogDescription>
            Opens the URL, discovers same-origin routes, and writes a draft scenario with a
            screenshot check per route — replay it, then mint baselines.
          </DialogDescription>
        </DialogHeader>
        <Input
          autoFocus
          placeholder="https://app.example.com"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && url.trim() && !busy) void submit()
          }}
        />
        {note && <div className="text-xs text-destructive">{note}</div>}
        <DialogFooter>
          <Button variant="secondary" onClick={() => setOpen(false)} disabled={busy}>
            Cancel
          </Button>
          <Button onClick={() => void submit()} disabled={busy || !/^https?:\/\//.test(url.trim())}>
            {busy ? 'Crawling…' : 'Crawl'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function ScenarioSidebar({ runs }: { runs: RunsApi }) {
  const { scenarios, expanded, runsBySid, sel } = runs
  return (
    <nav className="flex h-full min-h-0 flex-col overflow-hidden bg-sidebar/60">
      <div className="flex h-11 items-center justify-between border-b border-border px-3">
        <span className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">Scenarios</span>
        <div className="flex items-center gap-1">
          <CrawlDialog runs={runs} />
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
      </div>
      <ul className="min-h-0 flex-1 space-y-0.5 overflow-auto p-2">
        {scenarios.length === 0 && (
          <li className="flex flex-col items-center gap-1.5 px-3 py-10 text-center">
            <div className="text-[13px] font-semibold tracking-tight">No scenarios yet</div>
            <div className="max-w-[16rem] text-xs leading-relaxed text-muted-foreground">
              Record your first scenario from Chat or the CLI — or crawl a URL for a draft — and it will show up here.
            </div>
          </li>
        )}
        {scenarios.map((sc) => {
          const verdict = scenarioVerdict(sc.latestRun)
          const open = expanded.has(sc.sid)
          const list = runsBySid[sc.sid]
          return (
            <li key={sc.sid} className="group">
              <div className={cn('flex items-center rounded-lg transition-colors', open ? 'bg-accent/60 shadow-[inset_0_0_0_1px_color-mix(in_oklab,var(--primary)_12%,transparent)]' : 'hover:bg-muted/70')}>
                <button
                  type="button"
                  onClick={() => void runs.toggleScenario(sc.sid)}
                  className="min-w-0 flex-1 rounded-lg px-2.5 py-2 text-left focus-visible:outline-2 focus-visible:outline-ring"
                >
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13px] font-medium leading-tight">{sc.intent || sc.scenarioId || sc.sid}</span>
                    {sc.scenarioError ? (
                      <Badge tone="fail" title={sc.scenarioError}>
                        unreadable
                      </Badge>
                    ) : (
                      verdict && <Badge tone={verdict}>{verdict}</Badge>
                    )}
                    <HealthBadges sid={sc.sid} runs={runs} />
                  </div>
                  {sc.tags.length > 0 && (
                    <div className="mt-0.5 flex flex-wrap items-center gap-1">
                      {sc.tags.slice(0, 4).map((t) => (
                        <span
                          key={t}
                          title={`replay --tags ${t} selects this scenario`}
                          className="rounded border border-border px-1 py-px text-[10px] leading-tight text-muted-foreground"
                        >
                          {t}
                        </span>
                      ))}
                      {sc.tags.length > 4 && (
                        <span className="text-[10px] text-muted-foreground">+{sc.tags.length - 4}</span>
                      )}
                    </div>
                  )}
                  <div data-qa-volatile className="tnum mt-0.5 truncate text-[11px] text-muted-foreground/80" title={fmtRunTime(sc.sid)}>
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
                <div className="ml-3.5 mt-1 mb-1.5 space-y-0.5 border-l border-border pl-2">
                  {!list && <div className="px-1 py-1 text-xs text-muted-foreground">loading…</div>}
                  {list && list.length === 0 && <div className="px-1 py-1 text-xs text-muted-foreground">No replays yet</div>}
                  {list &&
                    [...list].reverse().map((r) => {
                      const selected = sel.sid === sc.sid && sel.runId === r.runId
                      const tone =
                        r.state === 'running' ? 'running' : r.state === 'stale' ? 'stopped' : verdictTone(r.summary, r.state)
                      const label =
                        r.state === 'running'
                          ? 'running'
                          : r.state === 'stale'
                            ? 'interrupted'
                            : r.summary
                              ? cleanSummary(r.summary)
                              : 'in flight'
                      return (
                        <div
                          key={r.runId}
                          className={cn(
                            'group/run flex items-center rounded-md transition-colors',
                            selected ? 'bg-card shadow-sm ring-1 ring-primary/20' : 'hover:bg-muted/60'
                          )}
                        >
                          <button
                            type="button"
                            onClick={() => void runs.selectRun(sc.sid, r.runId)}
                            className="flex min-w-0 flex-1 items-center gap-2 px-1.5 py-1 text-left"
                          >
                            <Badge tone={tone}>{label}</Badge>
                            <span
                              data-qa-volatile
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
