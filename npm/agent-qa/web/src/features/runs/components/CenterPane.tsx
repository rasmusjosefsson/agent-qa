// web/src/features/runs/components/CenterPane.tsx
import { useState } from 'react'
import { BrowserModeToggle } from '@/components/browser-mode-toggle'
import { TrendChip } from './TrendChip'
import { cn } from '@/lib/utils'
import { CameraIcon, GitCompareIcon, Loader2Icon, PlayIcon, PlusIcon, WrenchIcon } from 'lucide-react'
import { acceptAllDomshots, acceptAllShots, runFileUrl } from '@/lib/runs-api'
import { CompareView } from './CompareView'
import { InsertCheckDialog } from './InsertCheckDialog' 
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
} from '../rows'
import type { RunsApi } from '../useRuns'
import { VERB_TONE } from '@/lib/verb-tone'


const STATUS_TONE: Record<string, string> = {
  pass: 'text-success',
  fail: 'text-destructive',
  running: 'text-warning',
  pending: 'text-muted-foreground',
}

function VerbBadge({ verb }: { verb: unknown }) {
  const cat = verbCat(verb)
  return (
    <span className={cn('shrink-0 rounded-md px-1.5 py-0.5 text-[11px] font-semibold leading-none', VERB_TONE[cat])}>
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
  const [insertAfter, setInsertAfter] = useState<{ stepId: string; label: string } | null>(null)
  const [acceptAll, setAcceptAll] = useState<'' | 'busy' | 'done' | 'error'>('')
  // Baseline runId for the compare picker ("" → the run before the selected
  // one, computed below).
  const [baseline, setBaseline] = useState('')
  // run.webm player — toggled by the video chip in the run header; the file
  // exists only for runs replayed with --record-video (detail.video).
  const [showVideo, setShowVideo] = useState(false)

  // Mode A — a recorded scenario is previewed (no run selected).
  if (scenarioDef && !detail) {
    const steps = scenarioDef.steps || []
    const runList = (sel.sid && runsBySid[sel.sid]) || []
    const lastRun = runList.length
      ? [...runList].sort((a, b) => (a.runId < b.runId ? 1 : -1))[0]
      : null
    return (
      <Pane>
        {/* Two rows: what this scenario is + the Replay action, then the
            settings the next replay uses. */}
        <div className="border-b border-border">
          <div className="flex items-start gap-4 px-5 pt-4 pb-3">
            <div className="min-w-0 flex-1">
              <h2
                className="line-clamp-2 text-[15px] font-semibold leading-snug tracking-tight"
                title={scenarioDef.intent || scenarioDef.id || 'Scenario'}
              >
                {scenarioDef.intent || scenarioDef.id || 'Scenario'}
              </h2>
              <div className="tnum mt-1 truncate text-xs text-muted-foreground">
                {steps.length} step{steps.length === 1 ? '' : 's'} · recorded {fmtRunTime(sel.sid)}
                {lastRun
                  ? ` · last run ${relRunTime(lastRun.runId)}${lastRun.summary ? ' · ' + cleanSummary(lastRun.summary) : ''}`
                  : ' · not yet replayed'}
                {sel.sid && <TrendChip sid={sel.sid} />}
              </div>
              {sel.sid && (
                <div className="mt-0.5 truncate font-mono text-[11px] text-muted-foreground opacity-50">{sel.sid}</div>
              )}
            </div>
            {sel.sid && (
              <button
                type="button"
                onClick={() => onReplay(sel.sid!)}
                disabled={busy}
                title={busy ? 'A replay is already running for this scenario' : undefined}
                className="flex h-8 shrink-0 items-center gap-1.5 rounded-lg bg-primary px-3 text-[13px] font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {busy ? (
                  <>
                    <Loader2Icon className="size-3.5 animate-spin" /> Replaying…
                  </>
                ) : (
                  <>
                    <PlayIcon className="size-3.5 fill-current" /> Replay
                  </>
                )}
              </button>
            )}
          </div>
          {sel.sid && runConfig && (
            <div className="flex flex-wrap items-center gap-x-2 gap-y-2 border-t border-border/60 bg-muted/30 px-5 py-2 text-xs text-muted-foreground">
              {(runConfig.personas.length > 0 || runConfig.environments.length > 0) && (
                <>
                  <span>Run as</span>
                  <select
                    value={runConfig.personaId}
                    onChange={(e) => runConfig.setPersonaId(e.target.value)}
                    disabled={busy}
                    aria-label="Replay as persona"
                    className="min-w-0 max-w-[12rem] h-7 rounded-lg border border-border bg-card px-2 text-xs font-medium text-foreground shadow-xs outline-none transition-colors hover:border-primary/30 focus-visible:border-primary/50 focus-visible:ring-3 focus-visible:ring-primary/10 disabled:opacity-50"
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
                    aria-label="Replay on environment"
                    className="min-w-0 max-w-[16rem] h-7 rounded-lg border border-border bg-card px-2 text-xs font-medium text-foreground shadow-xs outline-none transition-colors hover:border-primary/30 focus-visible:border-primary/50 focus-visible:ring-3 focus-visible:ring-primary/10 disabled:opacity-50"
                  >
                    <option value="">default environment</option>
                    {runConfig.environments.map((en) => (
                      <option key={en.id} value={en.id}>
                        {en.name}
                      </option>
                    ))}
                  </select>
                </>
              )}
              <div className="ml-auto">
                <BrowserModeToggle
                  headed={runConfig.headed}
                  onChange={runConfig.setHeaded}
                  disabled={busy}
                />
              </div>
            </div>
          )}
        </div>
        <ol className="min-h-0 flex-1 space-y-px overflow-auto p-2.5">
          {steps.map((st, i) => (
            <li
              key={i}
              className="group relative flex items-center gap-2.5 rounded-lg px-2.5 py-2 transition-colors hover:bg-muted/60"
            >
              <span className="tnum grid size-5 shrink-0 place-items-center rounded-full bg-muted font-mono text-[10px] text-muted-foreground">{i}</span>
              <VerbBadge verb={st.verb} />
              <span className="truncate text-[13px]">{st.intent || stepText(st)}</span>
              {st.id && (
                <button
                  type="button"
                  title={`Insert a check after “${st.id}”`}
                  onClick={() => setInsertAfter({ stepId: st.id!, label: st.intent || st.id! })}
                  className="absolute right-1.5 top-1/2 -translate-y-1/2 rounded-md border border-border bg-card p-1 text-muted-foreground opacity-0 shadow-xs transition-opacity hover:border-primary/30 hover:text-primary group-hover:opacity-100"
                >
                  <PlusIcon className="size-3.5" />
                </button>
              )}
            </li>
          ))}
          {steps.length === 0 && <li className="px-2 py-3 text-xs text-muted-foreground">This scenario has no steps.</li>}
        </ol>
        {insertDialog(insertAfter, setInsertAfter, sel.sid, runs)}
      </Pane>
    )
  }

  // Mode B — a run is selected.
  if (detail) {
    const live = isRunLive(detail)
    const a = detail.audit || {}
    const s = detail.status || {}
    // 'stale' = status.json still says "running" but hasn't been touched in
    // the staleness window — the replay process is dead. Not a verdict.
    const stale = s.state === 'stale'
    const summary = a.summary || (live ? `running ${s.currentIdx || 0}/${s.total || '?'}` : stale ? 'interrupted' : 'in flight')
    const tone = /PASS/.test(summary) ? 'pass' : /FAIL/.test(summary) ? 'fail' : stale ? 'stopped' : 'running'
    const events = collapseEvents(detail.events || [])
    const currentIdx = typeof s.currentIdx === 'number' ? s.currentIdx : -1
    const liveCurrent = live ? currentIdx : -1
    const defSteps = runDefSteps.sid === sel.sid ? runDefSteps.steps : null
    const rows = mergeRows(events, defSteps)

    // Setup (env.open, e.g. `useProfile` sign-in) runs BEFORE any step. Surface
    // its two invisible states so the run never looks silently stuck/broken:
    //   • signing in  — started, no step has begun, not terminal yet
    //   • setup failed — a `setup` event errored (e.g. no credentials)
    const terminal = !!a.summary || s.state === 'done' || stale
    const stepStarted = (detail.events || []).some((e) => e.status && e.status !== 'pending')
    const signingIn = !terminal && !stepStarted
    const setupStartedAt = Date.parse(String(a.startedAt || ''))
    const setupSlow = signingIn && Number.isFinite(setupStartedAt) && Date.now() - setupStartedAt > 60_000
    const setupFail = (detail.events || []).find((e) => e.kind === 'setup' && e.status === 'fail')
    const setupError = setupFail && setupFail.error ? cleanSummary(setupFail.error) : null

    // A STEP failure (e.g. a `goto` that hit net::ERR_CONNECTION_REFUSED) carries
    // its reason on the event, but that's a click away in StepDetail — surface it
    // at the top too, so a failed run is never "failed for no visible reason".
    const stepFail = !setupError
      ? (detail.events || []).find((e) => e.kind !== 'setup' && e.status === 'fail')
      : null
    // Other finished runs of this scenario, newest first — the baseline
    // choices the Compare picker offers.
    const otherRuns = ((sel.sid && runsBySid[sel.sid]) || [])
      .filter((r) => r.runId !== detail.runId && r.state !== 'running' && r.state !== 'stale')
      .sort((a, b) => (a.runId < b.runId ? 1 : -1))
    const baseRun = otherRuns.some((r) => r.runId === baseline)
      ? baseline
      : (otherRuns[0]?.runId ?? '')
    // Auto-heal trail for this run: locator corrections applied on retry, and
    // classified value rejections surfaced for review (never retried).
    const heals = detail.heals || []
    const healByStep = new Map(heals.map((h) => [h.stepId, h]))
    const healedCount = heals.filter((h) => h.mode === 'locator-correction').length
    // Any {"shot":…} or {"domshot":…} claim in the scenario → the run can
    // promote its captures as the new baselines ("apply new goldens").
    const hasShotClaims = !!(
      defSteps &&
      defSteps.some((s) => {
        const check = s.check as { shot?: unknown } | undefined
        const claim = s.claim as { subject?: { shot?: unknown } } | undefined
        return (check && check.shot != null) || (claim && claim.subject && claim.subject.shot != null)
      })
    )
    const hasDomshotClaims = !!(
      defSteps &&
      defSteps.some((s) => {
        const check = s.check as { domshot?: unknown } | undefined
        const claim = s.claim as { subject?: { domshot?: unknown } } | undefined
        return (check && check.domshot != null) || (claim && claim.subject && claim.subject.domshot != null)
      })
    )
    const hasGoldenClaims = hasShotClaims || hasDomshotClaims
    const stepError = stepFail && stepFail.error
      ? stepFail.error.replace(/^.*?exited \d+:\s*/, '').replace(/^[✗✘x]\s*/, '').trim()
      : null
    const looksUnreachable = stepError ? /ERR_CONNECTION|ERR_NAME_NOT_RESOLVED|ERR_TIMED_OUT|Navigation failed/i.test(stepError) : false

    return (
      <Pane>
        <div className="border-b border-border px-4 py-3" data-qa-volatile>
          <div className="flex items-center gap-2">
            <span
              className={cn(
                'inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide before:size-1.5 before:rounded-full before:bg-current',
                tone === 'pass' && 'bg-success/15 text-success',
                tone === 'fail' && 'bg-destructive/15 text-destructive',
                tone === 'running' && 'bg-warning/15 text-warning',
                tone === 'stopped' && 'bg-muted text-muted-foreground'
              )}
            >
              {cleanSummary(summary)}
            </span>
            {live && (
              <span className="inline-flex items-center gap-1.5 text-xs font-medium text-warning">
                <span className="aqa-ping size-1.5 rounded-full bg-current" /> live
              </span>
            )}
            {detail.video && (
              <button
                type="button"
                onClick={() => setShowVideo((v) => !v)}
                title="This run was recorded with --record-video — toggle the run.webm player"
                className="flex items-center gap-1 rounded bg-info/15 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-info hover:bg-info/25"
              >
                <PlayIcon className="size-3" />
                video
              </button>
            )}
            {healedCount > 0 && (
              <span
                className="flex items-center gap-1 rounded bg-warning/15 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-warning"
                title="Auto-heal corrected a drifting locator on these steps — review the diff in the step detail."
              >
                <WrenchIcon className="size-3" />
                {healedCount} healed
              </span>
            )}
            {!live && (otherRuns.length > 0 || hasGoldenClaims) && (
              <span className="ml-auto flex items-center gap-1.5">
                {hasGoldenClaims && (
                  <button
                    type="button"
                    disabled={acceptAll === 'busy'}
                    title="Promote every golden captured in this run (screenshots + ARIA snapshots) to the checked-in baselines — apply new goldens"
                    className="flex h-7 items-center gap-1.5 rounded-lg border border-border bg-card px-2.5 text-[11px] font-medium text-muted-foreground shadow-xs transition-colors hover:border-primary/30 hover:text-foreground disabled:opacity-50"
                    onClick={() => {
                      if (!sel.sid) return
                      setAcceptAll('busy')
                      void (async () => {
                        const jobs: Promise<{ ok: boolean }>[] = []
                        if (hasShotClaims) jobs.push(acceptAllShots(sel.sid!, detail.runId))
                        if (hasDomshotClaims) jobs.push(acceptAllDomshots(sel.sid!, detail.runId))
                        const rs = await Promise.all(jobs)
                        setAcceptAll(rs.every((r) => r.ok) ? 'done' : 'error')
                      })()
                    }}
                  >
                    {acceptAll === 'busy' ? (
                      <Loader2Icon className="size-3 animate-spin" />
                    ) : (
                      <CameraIcon className="size-3" />
                    )}
                    {acceptAll === 'done' ? 'Goldens accepted' : 'Accept goldens'}
                  </button>
                )}
                {otherRuns.length > 0 && (
                <select
                  value={baseRun}
                  onChange={(e) => setBaseline(e.target.value)}
                  disabled={runs.compareBusy}
                  title="Baseline run to compare against"
                  className="h-7 max-w-[11rem] rounded-lg border border-border bg-card px-2 text-[11px] font-medium text-foreground shadow-xs outline-none transition-colors hover:border-primary/30 focus-visible:border-primary/50 focus-visible:ring-3 focus-visible:ring-primary/10 disabled:opacity-50"
                >
                  {otherRuns.map((r) => (
                    <option key={r.runId} value={r.runId}>
                      {relRunTime(r.runId)}
                      {r.summary ? ` · ${cleanSummary(r.summary)}` : ''}
                    </option>
                  ))}
                </select>
                )}
                <button
                  type="button"
                  onClick={() => void runs.startCompare(baseRun)}
                  disabled={runs.compareBusy || !baseRun}
                  title="Diff this run's snapshots and screenshots against the baseline"
                  className="flex h-7 items-center gap-1.5 rounded-lg border border-border bg-card px-2.5 text-[11px] font-medium text-muted-foreground shadow-xs transition-colors hover:border-primary/30 hover:text-foreground disabled:opacity-50"
                >
                  {runs.compareBusy ? (
                    <Loader2Icon className="size-3 animate-spin" />
                  ) : (
                    <GitCompareIcon className="size-3" />
                  )}
                  Compare
                </button>
              </span>
            )}
          </div>
          <div className="mt-0.5 text-xs text-muted-foreground">
            {fmtRunTime(detail.runId)} <span className="font-mono opacity-50">· {detail.runId}</span>
          </div>
          {signingIn && (
            <div className="mt-2 flex items-center gap-2 rounded-lg border border-warning/30 bg-warning/10 px-3 py-2 text-xs text-warning">
              <Loader2Icon className="size-3.5 shrink-0 animate-spin" />
              {setupSlow
                ? 'No replay progress for over a minute. Setup may be stuck; the host watchdog will stop it and mark it failed if this continues.'
                : 'Signing in… authenticating the persona in a fresh browser — this can take ~30s.'}
            </div>
          )}
          {stale && (
            <div className="mt-2 rounded-lg border border-border bg-muted/50 px-3 py-2 text-xs text-muted-foreground">
              <div className="font-medium text-foreground">Replay interrupted — the process stopped before finishing.</div>
              <div className="mt-0.5 text-muted-foreground">
                The run's status file went quiet mid-run (killed or crashed). Replay again to rerun it.
              </div>
            </div>
          )}
          {setupError && (
            <div className="mt-2 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              <div className="font-medium">Setup failed — the run never started.</div>
              <div className="mt-0.5 opacity-90">{setupError}</div>
              <div className="mt-1 text-muted-foreground">
                This scenario signs in first. Pick the right <span className="font-medium">Replay as</span> persona
                (and environment) at the top, then replay.
              </div>
            </div>
          )}
          {stepError && (
            <div className="mt-2 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
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
          {runs.compareErr && (
            <div className="mt-2 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {runs.compareErr}
            </div>
          )}
          {detail.video && showVideo && sel.sid && (
            <video
              controls
              preload="metadata"
              src={runFileUrl(sel.sid, detail.runId, 'run.webm')}
              className="mt-2 w-full rounded-md border border-border bg-black"
            />
          )}
        </div>
        {runs.compare ? (
          <CompareView runs={runs} />
        ) : (
        <ol className="min-h-0 flex-1 overflow-auto p-2">
          {rows.map((st) => {
            const selected = sel.stepIdx === st.idx
            const isCurrent = st.idx === liveCurrent
            const pending = st.status === 'pending'
            return (
              <li key={st.idx} className="group relative">
                <button
                  type="button"
                  disabled={pending}
                  onClick={pending ? undefined : () => runs.selectStep(st.idx)}
                  className={cn(
                    'flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left transition-all',
                    selected && 'bg-card shadow-sm ring-1 ring-inset ring-primary/25',
                    isCurrent && 'bg-warning/5 ring-1 ring-warning/40',
                    pending ? 'opacity-50' : !selected && 'hover:bg-muted/60'
                  )}
                >
                  <span
                    data-qa-volatile
                    className={cn(
                      'grid size-5 shrink-0 place-items-center rounded-full bg-current/10 text-[11px] font-semibold',
                      STATUS_TONE[st.status || ''] || 'text-muted-foreground'
                    )}
                  >
                    {icon(st.status)}
                  </span>
                  <span className="tnum w-10 shrink-0 font-mono text-[11px] text-muted-foreground">
                    {st.idx}/{st.total || rows.length}
                  </span>
                  <span className="truncate text-sm">{st.intent || st.id}</span>
                  {(() => {
                    const h = st.id ? healByStep.get(st.id) : undefined
                    if (!h) return null
                    return h.mode === 'value-rejection' ? (
                      <span
                        className="shrink-0 rounded-full bg-destructive/12 px-1.5 py-0.5 text-[10px] font-semibold text-destructive"
                        title="Auto-heal classified this step's failure as a value rejection (page refused the value) — evidence is in the step detail."
                      >
                        rejected
                      </span>
                    ) : (
                      <span
                        className="shrink-0 rounded-full bg-warning/12 px-1.5 py-0.5 text-[10px] font-semibold text-warning"
                        title={`Locator auto-healed via ${h.strategy || 'the name ladder'}: ${h.from || ''} → ${h.to || ''}`}
                      >
                        healed
                      </span>
                    )
                  })()}
                  {st.kind && <span className="shrink-0 text-xs text-muted-foreground">({st.kind})</span>}
                  <span data-qa-volatile className={cn('tnum ml-auto shrink-0 font-mono text-[11px] text-muted-foreground transition-[margin]', st.id && !live && 'group-hover:mr-7')}>{pending ? '' : fmtMs(st.ms)}</span>
                </button>
                {st.id && !live && (
                  <button
                    type="button"
                    title={`Insert a check after “${st.id}” in the saved scenario`}
                    onClick={() =>
                      setInsertAfter({ stepId: st.id!, label: st.intent || st.id! })
                    }
                    className="absolute right-1.5 top-1/2 -translate-y-1/2 rounded-md border border-border bg-card p-1 text-muted-foreground opacity-0 shadow-xs transition-opacity hover:border-primary/30 hover:text-primary group-hover:opacity-100"
                  >
                    <PlusIcon className="size-3.5" />
                  </button>
                )}
              </li>
            )
          })}
          {rows.length === 0 && (
            <li className="px-2 py-3 text-xs text-muted-foreground">
              {signingIn ? 'Signing in… steps start once the persona is authenticated.' : 'No steps recorded for this run.'}
            </li>
          )}
        </ol>
        )}
        {insertDialog(insertAfter, setInsertAfter, sel.sid, runs)}
      </Pane>
    )
  }

  // Nothing selected at all: one welcoming empty state (the sidebar carries
  // its own). Copy depends on whether any scenario exists yet.
  const hasAnyScenario = (runs.scenarios || []).length > 0
  return (
    <Pane>
      <div className="aqa-dots flex flex-1 flex-col items-center justify-center gap-3 p-8 text-center">
        <div className="mb-1">
          <div className="grid size-12 place-items-center rounded-2xl bg-primary/10 text-primary">
            <PlayIcon className="size-6 fill-current" />
          </div>
        </div>
        <div className="text-lg font-semibold tracking-tight">
          {hasAnyScenario ? 'No run selected' : 'Record your first scenario'}
        </div>
        <div className="max-w-sm text-[13px] leading-relaxed text-muted-foreground">
          {hasAnyScenario
            ? 'Pick a scenario on the left to preview its steps, then press Replay.'
            : 'Head to Chat and ask it to record a flow — it will show up here ready to replay.'}
        </div>
      </div>
    </Pane>
  )
}

function Pane({ children }: { children: React.ReactNode }) {
  return <section className="@container flex h-full min-h-0 flex-col overflow-hidden">{children}</section>
}

function insertDialog(
  insertAfter: { stepId: string; label: string } | null,
  setInsertAfter: (v: { stepId: string; label: string } | null) => void,
  sid: string | null,
  runs: RunsApi
) {
  if (!sid) return null
  return (
    <InsertCheckDialog
      open={!!insertAfter}
      onOpenChange={(v) => {
        if (!v) setInsertAfter(null)
      }}
      sid={sid}
      afterStepId={insertAfter?.stepId || ''}
      afterLabel={insertAfter?.label || ''}
      onDone={() => void runs.reloadDef(sid)}
    />
  )
}
