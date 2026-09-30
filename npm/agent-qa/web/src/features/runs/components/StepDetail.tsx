// web/src/features/runs/components/StepDetail.tsx
import { useCallback, useEffect, useState } from 'react'
import { BugIcon, WrenchIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { acceptDomshot, acceptShot, artifactUrl, fetchArtifactText, getScenarioDef, promoteHeal } from '@/lib/runs-api'
import { collapseEvents, fmtMs, icon } from '../rows'
import type { DetailTab, HealRow, RunDetail, RunEvent, ScenarioDef, ScenarioStep } from '../types'
import type { RunsApi as Api } from '../useRuns'

const TABS: { id: DetailTab; label: string }[] = [
  { id: 'step', label: 'Step' },
  { id: 'scenario', label: 'Scenario' },
  { id: 'context', label: 'Context' },
  { id: 'network', label: 'Network' },
  { id: 'html', label: 'HTML' },
  { id: 'console', label: 'Console' },
]

const STATUS_TONE: Record<string, string> = {
  pass: 'text-success',
  fail: 'text-destructive',
  running: 'text-warning',
  pending: 'text-muted-foreground',
}

// What did this step target? scenario.json carries the click/type target
// (on.role / on.name) and value — the events stream doesn't, so we join them.
function targetSummary(s?: ScenarioStep): string {
  if (!s) return ''
  const on = s.on
  // Raw selector (recorder fell back to css/xpath) takes priority — it's the
  // literal thing that was clicked.
  if (on?.raw?.value) return `${on.raw.kind || 'selector'} ${on.raw.value}`
  const parts: string[] = []
  if (on?.role) parts.push(on.role)
  if (on?.name) parts.push(`“${on.name}”`)
  return parts.join(' ')
}

export function StepDetail({ runs, onLightbox }: { runs: Api; onLightbox: (url: string, caption: string) => void }) {
  const { detail, sel } = runs
  const sid0 = sel.sid

  // Load the scenario.json for the selected scenario so the detail pane can show
  // what each step actually targets + its source. (Hook stays unconditional.)
  const [scenario, setScenario] = useState<ScenarioDef | null>(null)
  const reloadScenario = useCallback(() => {
    if (!sid0) return
    getScenarioDef(sid0)
      .then((r) => setScenario(r.scenario))
      .catch(() => {})
  }, [sid0])
  useEffect(() => {
    if (!sid0) {
      setScenario(null)
      return
    }
    let alive = true
    getScenarioDef(sid0)
      .then((r) => alive && setScenario(r.scenario))
      .catch(() => alive && setScenario(null))
    return () => {
      alive = false
    }
  }, [sid0])

  if (!detail || sel.stepIdx == null) return <Empty>Select a step to see details.</Empty>
  const steps = collapseEvents(detail.events || [])
  const step = steps.find((s) => s.idx === sel.stepIdx)
  if (!step) return <Empty>Step not found.</Empty>
  const prev = steps.find((s) => s.idx === step.idx - 1)
  const sid = sel.sid!
  const runId = sel.runId!
  const defStep = scenario?.steps?.find((s) => s.id === step.id)
  const heal = (detail.heals || []).find((h) => h.stepId === step.id)
  // A failing {"shot":"x"} check references the do-step whose screenshot
  // drifted; the delta map is keyed by that referenced id.
  const shotRef = (defStep?.claim as { subject?: { shot?: string } } | undefined)?.subject?.shot
  const shotDiff = shotRef && (detail.shotDiffs || []).includes(shotRef) ? shotRef : null
  // Same for {"domshot":"x"}: the unified diff is keyed by the referenced id.
  const domshotRef = (defStep?.claim as { subject?: { domshot?: string } } | undefined)?.subject?.domshot
  const domshotDiff = domshotRef && (detail.domshotDiffs || []).includes(domshotRef) ? domshotRef : null

  // Open a NEW chat seeded with the failure context so the agent can triage
  // flake-vs-real. ChatPage consumes the ?ask= param on load.
  const askAgent = () => {
    const target = targetSummary(defStep)
    const prompt = [
      `A replay of scenario "${sid}" (run ${runId}) failed — help me debug it.`,
      `Failing step ${step.idx}/${step.total ?? '?'}: "${step.intent || step.id}" (${step.kind || 'step'}).`,
      target ? `It targets: ${target}.` : null,
      step.error ? `Error: ${step.error}` : null,
      `Figure out whether this is a flake or a real problem with the scenario or our tooling, then suggest a fix. You can re-run it with \`agent-qa replay ${sid}\` and inspect the run.`,
    ]
      .filter(Boolean)
      .join('\n')
    window.open(`/chat?ask=${encodeURIComponent(prompt)}`, '_blank')
  }

  return (
    <section className="flex h-full min-h-0 flex-col overflow-hidden">
      <div className="flex items-start justify-between gap-2 border-b border-border px-4 py-3">
        <div className="min-w-0">
          <h3 className="flex items-center gap-1.5 text-sm font-semibold">
            <span className={cn(STATUS_TONE[step.status || ''] || 'text-muted-foreground')}>{icon(step.status)}</span>
            <span className="truncate">{step.intent || step.id}</span>
          </h3>
          <div className="text-xs text-muted-foreground">
            {step.kind || ''} · step {step.idx}/{step.total || ''}
            {targetSummary(defStep) ? <> · {targetSummary(defStep)}</> : null}
          </div>
        </div>
        {step.status === 'fail' && (
          <button
            type="button"
            onClick={askAgent}
            title="Open a new chat and ask the agent to debug this failure"
            className="flex h-7 shrink-0 items-center gap-1.5 rounded-lg border border-border bg-card px-2.5 text-xs font-medium text-muted-foreground shadow-xs transition-colors hover:border-primary/30 hover:text-foreground"
          >
            <BugIcon className="size-3.5" /> Ask agent
          </button>
        )}
      </div>

      <div className="grid shrink-0 grid-cols-2 gap-2 border-b border-border p-3">
        <Shot title="Before" sid={sid} runId={runId} step={prev} onLightbox={onLightbox} />
        <Shot title="After" sid={sid} runId={runId} step={step} onLightbox={onLightbox} />
      </div>

      <div className="flex shrink-0 gap-1 border-b border-border px-2 py-1.5">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => runs.selectTab(t.id)}
            className={cn(
              'rounded-md px-2.5 py-1 text-xs font-medium transition-colors',
              sel.tab === t.id
                ? 'bg-accent text-accent-foreground shadow-[inset_0_0_0_1px_color-mix(in_oklab,var(--primary)_18%,transparent)]'
                : 'text-muted-foreground hover:bg-muted/60 hover:text-foreground'
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div className="min-h-0 flex-1 overflow-auto p-3">
        {heal && <HealCard heal={heal} sid={sid} runId={runId} onPromoted={reloadScenario} />}
        {shotDiff && (
          <ShotDiffCard sid={sid} runId={runId} shotStep={shotDiff} onLightbox={onLightbox} />
        )}
        {domshotDiff && <DomshotDiffCard sid={sid} runId={runId} domshotStep={domshotDiff} />}
        {step.error && (
          <pre className="mb-3 whitespace-pre-wrap rounded-md border border-destructive/30 bg-destructive/10 p-2 text-xs text-destructive">
            {step.error}
          </pre>
        )}
        <TabBody sid={sid} runId={runId} step={step} tab={sel.tab} defStep={defStep} scenario={scenario} runNetwork={detail.network} />
      </div>
    </section>
  )
}

function Shot({
  title,
  sid,
  runId,
  step,
  onLightbox,
}: {
  title: string
  sid: string
  runId: string
  step: RunEvent | undefined
  onLightbox: (url: string, caption: string) => void
}) {
  if (!step || !step.screenshot) {
    return (
      <div>
        <h4 className="mb-1.5 text-[10.5px] font-semibold uppercase tracking-[0.07em] text-muted-foreground">{title}</h4>
        <div className="aqa-dots grid aspect-video place-items-center rounded-lg border border-dashed border-border bg-muted/30 text-xs text-muted-foreground">
          not captured
        </div>
      </div>
    )
  }
  const url = artifactUrl(sid, runId, 'screenshots', step.id!)
  const caption = `${title} · ${step.intent || step.id}`
  return (
    <div>
      <h4 className="mb-1.5 text-[10.5px] font-semibold uppercase tracking-[0.07em] text-muted-foreground">{title}</h4>
      <button type="button" onClick={() => onLightbox(url, caption)} className="block w-full">
        <img src={url} alt={title} loading="lazy" className="aspect-video w-full rounded-lg border border-border object-cover object-top transition-shadow hover:shadow-md hover:ring-1 hover:ring-primary/30" />
      </button>
    </div>
  )
}

function TabBody({
  sid,
  runId,
  step,
  tab,
  defStep,
  scenario,
  runNetwork,
}: {
  sid: string
  runId: string
  step: RunEvent
  tab: DetailTab
  defStep?: ScenarioStep
  scenario: ScenarioDef | null
  runNetwork?: RunDetail['network']
}) {
  const [text, setText] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (tab === 'step' || tab === 'console' || tab === 'scenario') return
    const kind = tab === 'context' ? 'snapshots' : tab === 'network' ? 'network' : 'probes'
    let alive = true
    setLoading(true)
    setText(null)
    fetchArtifactText(sid, runId, kind, step.id!, kind !== 'snapshots')
      .then((t) => alive && setText(t))
      .finally(() => alive && setLoading(false))
    return () => {
      alive = false
    }
  }, [sid, runId, step.id, tab])

  if (tab === 'step') {
    const rows: [string, string][] = []
    if (defStep?.verb) rows.push(['verb', defStep.verb])
    if (defStep?.on?.role) rows.push(['target role', defStep.on.role])
    if (defStep?.on?.name) rows.push(['target name', defStep.on.name])
    if (defStep?.on?.raw?.value)
      rows.push(['target', `${defStep.on.raw.kind || 'selector'}: ${defStep.on.raw.value}`])
    if (defStep?.value?.literal != null) rows.push(['value', String(defStep.value.literal)])
    rows.push(
      ['id', step.id || ''],
      ['kind', step.kind || ''],
      ['status', step.status || ''],
      ['duration', fmtMs(step.ms)],
      ['snapshot', step.snapshot || '(none)'],
      ['screenshot', step.screenshot || '(none)']
    )
    return (
      <dl className="grid grid-cols-[7rem_1fr] gap-x-3 gap-y-1 text-xs">
        {rows.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-muted-foreground">{k}</dt>
            <dd className="break-all font-mono">{v === '' ? '—' : v}</dd>
          </div>
        ))}
      </dl>
    )
  }
  if (tab === 'scenario') {
    if (!scenario) return <div className="text-xs text-muted-foreground">scenario.json not available.</div>
    return (
      <div className="space-y-4">
        <div>
          <div className="mb-1 text-xs font-medium text-muted-foreground">This step (scenario.json)</div>
          <pre className="whitespace-pre-wrap break-all rounded-md border border-border bg-muted/20 p-2 font-mono text-xs leading-relaxed">
            {JSON.stringify(defStep ?? { note: 'no matching step id in scenario.json' }, null, 2)}
          </pre>
        </div>
        <div>
          <div className="mb-1 text-xs font-medium text-muted-foreground">Full scenario.json</div>
          <pre className="whitespace-pre-wrap break-all font-mono text-xs leading-relaxed text-muted-foreground">
            {JSON.stringify(scenario, null, 2)}
          </pre>
        </div>
      </div>
    )
  }
  if (tab === 'console') {
    return <div className="text-xs text-muted-foreground">Console output is not captured for this step.</div>
  }
  if (tab === 'network') {
    // Two artifacts share this tab: the run-level request log (network.json,
    // written for every run) and this step's {"network"} claim probe
    // (per-step capture, only present when the scenario asserts on traffic).
    return (
      <div className="space-y-4">
        <RunTraffic network={runNetwork} />
        {(loading || text != null) && (
          <div>
            <div className="mb-1 text-xs font-medium text-muted-foreground">This step's network probe</div>
            {loading ? (
              <div className="text-xs text-muted-foreground">Loading…</div>
            ) : (
              <pre className="whitespace-pre-wrap break-all font-mono text-xs leading-relaxed">{text}</pre>
            )}
          </div>
        )}
      </div>
    )
  }
  if (loading) return <div className="text-xs text-muted-foreground">Loading…</div>
  if (text == null) return <div className="text-xs text-muted-foreground">Not captured for this step.</div>
  return <pre className="whitespace-pre-wrap break-all font-mono text-xs leading-relaxed">{text}</pre>
}

// The run's captured request log (network.json) as a table — method, URL,
// status. Rendered on the Network tab above the per-step claim probe.
export function RunTraffic({ network }: { network?: RunDetail['network'] }) {
  const reqs = network?.requests || []
  return (
    <div>
      <div className="mb-1 text-xs font-medium text-muted-foreground">
        Run traffic{network ? ` · ${network.requestCount ?? reqs.length} request(s)` : ''}
      </div>
      {reqs.length > 0 ? (
        <ul className="divide-y divide-border rounded-md border border-border">
          {reqs.map((r) => (
            <li key={r.requestId} className="flex items-center gap-2 px-2 py-1 text-xs">
              <span className="w-14 shrink-0 font-mono text-muted-foreground">{r.method}</span>
              <span className="min-w-0 flex-1 truncate font-mono" title={r.url}>
                {r.url}
              </span>
              {r.status != null && (
                <span
                  className={cn(
                    'shrink-0 font-mono',
                    r.status >= 400 ? 'text-destructive' : 'text-muted-foreground'
                  )}
                >
                  {r.status}
                </span>
              )}
            </li>
          ))}
        </ul>
      ) : (
        <div className="text-xs text-muted-foreground">
          {network ? 'No requests captured.' : 'network.json not written for this run.'}
        </div>
      )}
    </div>
  )
}

// A locator the auto-heal loop rewrote mid-run, or a failure it classified as
// a value rejection. Corrections carry the suggested patch (diffs/*.patch.json)
// inline so it can be promoted to scenario.json without the terminal.
function HealCard({
  heal,
  sid,
  runId,
  onPromoted,
}: {
  heal: HealRow
  sid: string
  runId: string
  onPromoted?: () => void
}) {
  const isRejection = heal.mode === 'value-rejection'
  const [promote, setPromote] = useState<'idle' | 'busy' | 'done' | 'error'>('idle')
  const [promoteError, setPromoteError] = useState('')
  const apply = async () => {
    if (!heal.stepId) return
    setPromote('busy')
    const r = await promoteHeal(sid, runId, heal.stepId)
    if (r.ok) {
      setPromote('done')
      onPromoted?.()
    } else {
      setPromote('error')
      setPromoteError(r.error || 'promote failed')
    }
  }
  return (
    <div
      className={cn(
        'mb-3 rounded-md border p-2.5 text-xs',
        isRejection
          ? 'border-destructive/30 bg-destructive/10 text-destructive'
          : 'border-warning/30 bg-warning/10 text-warning'
      )}
    >
      <div className="flex items-center gap-1.5 font-medium">
        <WrenchIcon className="size-3.5 shrink-0" />
        {isRejection ? 'Value rejection — not retried' : `Locator auto-healed via ${heal.strategy || 'name ladder'}`}
      </div>
      {!isRejection && (heal.from || heal.to) && (
        <div className="mt-1 break-all font-mono opacity-90">
          {heal.from} → <span className="font-semibold">{heal.to}</span>
        </div>
      )}
      {isRejection && heal.rationale && <div className="mt-1 opacity-90">{heal.rationale}</div>}
      {heal.patch && (
        <details className="mt-1.5">
          <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
            Suggested patch
          </summary>
          <pre className="mt-1 whitespace-pre-wrap break-all rounded border border-border bg-muted/20 p-2 font-mono leading-relaxed">
            {JSON.stringify(heal.patch, null, 2)}
          </pre>
        </details>
      )}
      {heal.patch && !isRejection && (
        <div className="mt-1.5 flex items-center gap-2">
          {promote === 'done' ? (
            <span className="text-success">Promoted into scenario.json — re-run to confirm.</span>
          ) : (
            <>
              <span className="text-muted-foreground">Looks right?</span>
              <button
                type="button"
                onClick={apply}
                disabled={promote === 'busy'}
                className="rounded border border-warning/40 px-1.5 py-0.5 font-medium transition-colors hover:bg-warning/20 disabled:opacity-50"
              >
                {promote === 'busy' ? 'Promoting…' : 'Promote patch'}
              </button>
              {promote === 'error' && <span className="text-destructive">{promoteError}</span>}
            </>
          )}
        </div>
      )}
    </div>
  )
}

// The delta map a {"shot"} claim wrote when it missed its baseline — red over
// a faded baseline. Re-mint in place when the change is legitimate.
export function ShotDiffCard({
  sid,
  runId,
  shotStep,
  onLightbox,
}: {
  sid: string
  runId: string
  shotStep: string
  onLightbox: (url: string, caption: string) => void
}) {
  const url = artifactUrl(sid, runId, 'shots-diff', shotStep)
  const caption = `Visual diff · ${shotStep}`
  const [accept, setAccept] = useState<'idle' | 'busy' | 'done' | 'error'>('idle')
  const remint = async () => {
    setAccept('busy')
    const r = await acceptShot(sid, runId, shotStep)
    setAccept(r.ok ? 'done' : 'error')
  }
  return (
    <div className="mb-3 rounded-md border border-info/30 bg-info/10 p-2.5 text-xs">
      <div className="mb-1.5 flex items-center gap-1.5 font-medium text-info">
        <WrenchIcon className="size-3.5 shrink-0" />
        Visual diff — shot “{shotStep}” changed vs baseline
      </div>
      <button type="button" onClick={() => onLightbox(url, caption)} className="block w-full">
        <img src={url} alt={caption} loading="lazy" className="w-full rounded border border-border" />
      </button>
      <div className="mt-1.5 flex items-center gap-2 text-muted-foreground">
        {accept === 'done' ? (
          <span className="text-success">Baseline re-minted — re-run to confirm.</span>
        ) : (
          <>
            <span>
              Legitimate change?{' '}
              <button
                type="button"
                onClick={remint}
                disabled={accept === 'busy'}
                className="rounded border border-info/40 px-1.5 py-0.5 font-medium text-info transition-colors hover:bg-info/20 disabled:opacity-50"
              >
                {accept === 'busy' ? 'Re-minting…' : 'Re-mint baseline'}
              </button>
            </span>
            {accept === 'error' && <span className="text-destructive">re-mint failed</span>}
          </>
        )}
      </div>
    </div>
  )
}

// The unified text diff a {"domshot"} claim wrote when the step's ARIA
// snapshot drifted from baselines/<stepId>.snap.txt. Rendered as text —
// red deletions, green additions — with a re-mint affordance for the
// legitimate-change path.
export function DomshotDiffCard({
  sid,
  runId,
  domshotStep,
}: {
  sid: string
  runId: string
  domshotStep: string
}) {
  const [diffText, setDiffText] = useState<string | null>(null)
  const [accept, setAccept] = useState<'idle' | 'busy' | 'done' | 'error'>('idle')
  useEffect(() => {
    let alive = true
    fetchArtifactText(sid, runId, 'domshots-diff', domshotStep, false)
      .then((t) => alive && setDiffText(t))
      .catch(() => alive && setDiffText(null))
    return () => {
      alive = false
    }
  }, [sid, runId, domshotStep])
  const remint = async () => {
    setAccept('busy')
    const r = await acceptDomshot(sid, runId, domshotStep)
    setAccept(r.ok ? 'done' : 'error')
  }
  return (
    <div className="mb-3 rounded-md border border-violet-500/30 bg-violet-500/10 p-2.5 text-xs">
      <div className="mb-1.5 flex items-center gap-1.5 font-medium text-violet-600 dark:text-violet-300">
        <WrenchIcon className="size-3.5 shrink-0" />
        Structural diff — domshot “{domshotStep}” changed vs baseline
      </div>
      {diffText != null && (
        <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all rounded border border-border bg-muted/20 p-2 font-mono leading-relaxed">
          {diffText.split('\n').map((line, i) => (
            <span
              key={i}
              className={cn(
                'block',
                line.startsWith('+') && !line.startsWith('+++') && 'text-success',
                line.startsWith('-') && !line.startsWith('---') && 'text-destructive',
                (line.startsWith('@@') || line.startsWith('---') || line.startsWith('+++')) &&
                  'text-muted-foreground'
              )}
            >
              {line}
            </span>
          ))}
        </pre>
      )}
      <div className="mt-1.5 flex items-center gap-2 text-muted-foreground">
        {accept === 'done' ? (
          <span className="text-success">Baseline re-minted — re-run to confirm.</span>
        ) : (
          <>
            <span>
              Legitimate change?{' '}
              <button
                type="button"
                onClick={remint}
                disabled={accept === 'busy'}
                className="rounded border border-violet-500/40 px-1.5 py-0.5 font-medium text-violet-600 transition-colors dark:text-violet-300 hover:bg-violet-500/20 disabled:opacity-50"
              >
                {accept === 'busy' ? 'Re-minting…' : 'Re-mint baseline'}
              </button>
            </span>
            {accept === 'error' && <span className="text-destructive">re-mint failed</span>}
          </>
        )}
      </div>
    </div>
  )
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <section className="aqa-dots flex h-full min-h-0 flex-col items-center justify-center bg-muted/30 p-8">
      <div className="max-w-[14rem] rounded-xl border border-border bg-card/90 px-4 py-3 text-center text-xs leading-relaxed text-muted-foreground shadow-sm backdrop-blur">
        {children}
      </div>
    </section>
  )
}
