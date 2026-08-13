// web/src/features/runs/components/CenterPane.tsx
import { BrowserModeToggle } from '@/components/browser-mode-toggle'
import { cn } from '@/lib/utils'
import { Loader2Icon, PlayIcon } from 'lucide-react'
import {
  cleanSummary,
  collapseEvents,
  fmtMs,
  fmtRunTime,
  relRunTime,
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

// Per-replay run config: which login/target the NEXT replay of the selected
// scenario uses. Rendered inline with the Replay button (not a global bar),
// defaulted upstream to the scenario's recorded persona.
export interface RunConfig {
  personas: { id: string; name: string }[]
  environments: { id: string; name: string }[]
  personaId: string
  envId: string
  headed: boolean
  setPersonaId: (v: string) => void
  setEnvId: (v: string) => void
  setHeaded: (v: boolean) => void
}

export function CenterPane({
  runs,
  onReplay,
  busy,
  runConfig,
}: {
  runs: RunsApi
  onReplay: (sid: string) => void
  // A replay of the selected scenario is already in flight — disable Replay so
  // a second run can't collide with it on the shared browser session.
  busy?: boolean
  runConfig?: RunConfig
}) {
  const { detail, scenarioDef, sel, runDefSteps, runsBySid } = runs

  // Mode A — a recorded scenario is previewed (no run selected).
  if (scenarioDef && !detail) {
    const steps = scenarioDef.steps || []
    const runList = (sel.sid && runsBySid[sel.sid]) || []
    const lastRun = runList.length
      ? [...runList].sort((a, b) => (a.runId < b.runId ? 1 : -1))[0]
      : null
    return (
      <Pane>
        <div className="flex flex-col items-stretch gap-3 border-b border-border px-4 py-3 @3xl:flex-row @3xl:items-center @3xl:justify-between">
          <div className="min-w-0 @3xl:flex-1">
            <h2 className="truncate text-sm font-semibold">{scenarioDef.intent || scenarioDef.id || 'Scenario'}</h2>
            <div className="truncate text-xs text-muted-foreground">
              {steps.length} step{steps.length === 1 ? '' : 's'} · recorded {fmtRunTime(sel.sid)}
              {lastRun
                ? ` · last run ${relRunTime(lastRun.runId)}${lastRun.summary ? ' · ' + cleanSummary(lastRun.summary) : ''}`
                : ' · not yet replayed'}
            </div>
            {sel.sid && (
              <div className="mt-0.5 truncate font-mono text-[11px] text-muted-foreground opacity-50">{sel.sid}</div>
            )}
          </div>
          {sel.sid && (
            <div className="flex w-full min-w-0 flex-wrap items-center gap-2 @3xl:w-auto @3xl:shrink-0 @3xl:justify-end">
              {runConfig && (
                <BrowserModeToggle
                  headed={runConfig.headed}
                  onChange={runConfig.setHeaded}
                  disabled={busy}
                />
              )}
              {runConfig && (runConfig.personas.length > 0 || runConfig.environments.length > 0) && (
                <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5 text-xs text-muted-foreground @3xl:flex-none @3xl:flex-nowrap">
                  <span>as</span>
                  <select
                    value={runConfig.personaId}
                    onChange={(e) => runConfig.setPersonaId(e.target.value)}
                    disabled={busy}
                    className="min-w-0 max-w-[10rem] rounded-md border border-border bg-background px-2 py-1 text-xs text-foreground outline-none focus-visible:border-ring disabled:opacity-50"
                  >
                    <option value="">default login</option>
                    {runConfig.personas.map((p) => (
                      <option key={p.id} value={p.id}>
                        {p.name}
                      </option>
                    ))}
                  </select>
                  <span>on</span>
                  <select
                    value={runConfig.envId}
                    onChange={(e) => runConfig.setEnvId(e.target.value)}
                    disabled={busy}
                    className="min-w-0 max-w-full flex-1 rounded-md border border-border bg-background px-2 py-1 text-xs text-foreground outline-none focus-visible:border-ring disabled:opacity-50 @3xl:max-w-[18rem] @3xl:flex-none"
                  >
                    <option value="">default environment</option>
                    {runConfig.environments.map((en) => (
                      <option key={en.id} value={en.id}>
                        {en.name}
                      </option>
                    ))}
                  </select>
                </div>
              )}
              <button
                type="button"
                onClick={() => onReplay(sel.sid!)}
                disabled={busy}
                title={busy ? 'A replay is already running for this scenario' : undefined}
                className="flex shrink-0 items-center gap-2 rounded-md border border-border bg-card px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-muted disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-card"
              >
                {busy ? (
                  <>
                    <Loader2Icon className="size-4 animate-spin" /> Replaying…
                  </>
                ) : (
                  <>
                    <PlayIcon className="size-4 fill-emerald-400 text-emerald-400" /> Replay
                  </>
                )}
              </button>
            </div>
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

    // Setup (env.open, e.g. `useProfile` sign-in) runs BEFORE any step. Surface
    // its two invisible states so the run never looks silently stuck/broken:
    //   • signing in  — started, no step has begun, not terminal yet
    //   • setup failed — a `setup` event errored (e.g. no credentials)
    const terminal = !!a.summary || s.state === 'done'
    const stepStarted = (detail.events || []).some((e) => e.status && e.status !== 'pending')
    const signingIn = !terminal && !stepStarted
    const setupFail = (detail.events || []).find((e) => e.kind === 'setup' && e.status === 'fail')
    const setupError = setupFail && setupFail.error ? cleanSummary(setupFail.error) : null

    // A STEP failure (e.g. a `goto` that hit net::ERR_CONNECTION_REFUSED) carries
    // its reason on the event, but that's a click away in StepDetail — surface it
    // at the top too, so a failed run is never "failed for no visible reason".
    const stepFail = !setupError
      ? (detail.events || []).find((e) => e.kind !== 'setup' && e.status === 'fail')
      : null
    const stepError = stepFail && stepFail.error
      ? stepFail.error.replace(/^.*?exited \d+:\s*/, '').replace(/^[✗✘x]\s*/, '').trim()
      : null
    const looksUnreachable = stepError ? /ERR_CONNECTION|ERR_NAME_NOT_RESOLVED|ERR_TIMED_OUT|Navigation failed/i.test(stepError) : false

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
          <div className="mt-0.5 text-xs text-muted-foreground">
            {fmtRunTime(detail.runId)} <span className="font-mono opacity-50">· {detail.runId}</span>
          </div>
          {signingIn && (
            <div className="mt-2 flex items-center gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-2.5 py-1.5 text-xs text-amber-300">
              <Loader2Icon className="size-3.5 shrink-0 animate-spin" />
              Signing in… authenticating the persona in a fresh browser — this can take ~30s.
            </div>
          )}
          {setupError && (
            <div className="mt-2 rounded-md border border-destructive/30 bg-destructive/10 px-2.5 py-1.5 text-xs text-destructive">
              <div className="font-medium">Setup failed — the run never started.</div>
              <div className="mt-0.5 opacity-90">{setupError}</div>
              <div className="mt-1 text-muted-foreground">
                This scenario signs in first. Pick the right <span className="font-medium">Replay as</span> persona
                (and environment) at the top, then replay.
              </div>
            </div>
          )}
          {stepError && (
            <div className="mt-2 rounded-md border border-destructive/30 bg-destructive/10 px-2.5 py-1.5 text-xs text-destructive">
              <div className="font-medium">
                Failed at step {stepFail?.idx ?? '?'}/{stepFail?.total ?? rows.length}
                {stepFail?.intent ? ` — ${cleanSummary(stepFail.intent)}` : ''}
              </div>
              <div className="mt-0.5 font-mono opacity-90">{stepError}</div>
              {looksUnreachable && (
                <div className="mt-1 text-muted-foreground">
                  The target app refused the connection or was unreachable. Check the app/staging is up and
                  you're on the right network (VPN), then replay — transient blips are retried automatically.
                </div>
              )}
            </div>
          )}
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
          {rows.length === 0 && (
            <li className="px-2 py-3 text-xs text-muted-foreground">
              {signingIn ? 'Signing in… steps start once the persona is authenticated.' : 'No steps recorded for this run.'}
            </li>
          )}
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
  return <section className="@container flex h-full min-h-0 flex-col overflow-hidden">{children}</section>
}
