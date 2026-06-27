// web/src/features/runs/components/StepDetail.tsx
import { useEffect, useState } from 'react'
import { BugIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { artifactUrl, fetchArtifactText, getScenarioDef } from '@/lib/runs-api'
import { collapseEvents, fmtMs, icon } from '../rows'
import type { DetailTab, RunEvent, ScenarioDef, ScenarioStep } from '../types'
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
  pass: 'text-emerald-400',
  fail: 'text-destructive',
  running: 'text-amber-400',
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
            className="flex shrink-0 items-center gap-1.5 rounded-md border border-border px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            <BugIcon className="size-3.5" /> Ask agent
          </button>
        )}
      </div>

      <div className="grid shrink-0 grid-cols-2 gap-2 border-b border-border p-3">
        <Shot title="Before" sid={sid} runId={runId} step={prev} onLightbox={onLightbox} />
        <Shot title="After" sid={sid} runId={runId} step={step} onLightbox={onLightbox} />
      </div>

      <div className="flex shrink-0 gap-1 border-b border-border px-2 py-1">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => runs.selectTab(t.id)}
            className={cn(
              'rounded px-2 py-1 text-xs',
              sel.tab === t.id ? 'bg-muted text-foreground' : 'text-muted-foreground hover:bg-muted/50'
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div className="min-h-0 flex-1 overflow-auto p-3">
        {step.error && (
          <pre className="mb-3 whitespace-pre-wrap rounded-md border border-destructive/30 bg-destructive/10 p-2 text-xs text-destructive">
            {step.error}
          </pre>
        )}
        <TabBody sid={sid} runId={runId} step={step} tab={sel.tab} defStep={defStep} scenario={scenario} />
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
        <h4 className="mb-1 text-xs text-muted-foreground">{title}</h4>
        <div className="grid aspect-video place-items-center rounded border border-border bg-muted/30 text-xs text-muted-foreground">
          not captured
        </div>
      </div>
    )
  }
  const url = artifactUrl(sid, runId, 'screenshots', step.id!)
  const caption = `${title} · ${step.intent || step.id}`
  return (
    <div>
      <h4 className="mb-1 text-xs text-muted-foreground">{title}</h4>
      <button type="button" onClick={() => onLightbox(url, caption)} className="block w-full">
        <img src={url} alt={title} loading="lazy" className="aspect-video w-full rounded border border-border object-cover object-top" />
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
}: {
  sid: string
  runId: string
  step: RunEvent
  tab: DetailTab
  defStep?: ScenarioStep
  scenario: ScenarioDef | null
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
  if (loading) return <div className="text-xs text-muted-foreground">Loading…</div>
  if (text == null) return <div className="text-xs text-muted-foreground">Not captured for this step.</div>
  return <pre className="whitespace-pre-wrap break-all font-mono text-xs leading-relaxed">{text}</pre>
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <section className="flex h-full min-h-0 flex-col items-center justify-center p-8 text-sm text-muted-foreground">
      {children}
    </section>
  )
}
