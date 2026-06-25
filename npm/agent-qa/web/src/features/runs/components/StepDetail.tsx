// web/src/features/runs/components/StepDetail.tsx
import { useEffect, useState } from 'react'
import { cn } from '@/lib/utils'
import { artifactUrl, fetchArtifactText } from '@/lib/runs-api'
import { collapseEvents, fmtMs, icon } from '../rows'
import type { DetailTab, RunEvent } from '../types'
import type { RunsApi as Api } from '../useRuns'

const TABS: { id: DetailTab; label: string }[] = [
  { id: 'step', label: 'Step' },
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

export function StepDetail({ runs, onLightbox }: { runs: Api; onLightbox: (url: string, caption: string) => void }) {
  const { detail, sel } = runs
  if (!detail || sel.stepIdx == null) {
    return <Empty>Select a step to see details.</Empty>
  }
  const steps = collapseEvents(detail.events || [])
  const step = steps.find((s) => s.idx === sel.stepIdx)
  if (!step) return <Empty>Step not found.</Empty>
  const prev = steps.find((s) => s.idx === step.idx - 1)
  const sid = sel.sid!
  const runId = sel.runId!

  return (
    <section className="flex h-full min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
      <div className="border-b border-border px-4 py-3">
        <h3 className="flex items-center gap-1.5 text-sm font-semibold">
          <span className={cn(STATUS_TONE[step.status || ''] || 'text-muted-foreground')}>{icon(step.status)}</span>
          <span className="truncate">{step.intent || step.id}</span>
        </h3>
        <div className="text-xs text-muted-foreground">
          {step.kind || ''} · step {step.idx}/{step.total || ''}
        </div>
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
        <TabBody sid={sid} runId={runId} step={step} tab={sel.tab} />
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

function TabBody({ sid, runId, step, tab }: { sid: string; runId: string; step: RunEvent; tab: DetailTab }) {
  const [text, setText] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (tab === 'step' || tab === 'console') return
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
    const rows: [string, string][] = [
      ['id', step.id || ''],
      ['kind', step.kind || ''],
      ['status', step.status || ''],
      ['duration', fmtMs(step.ms)],
      ['snapshot', step.snapshot || '(none)'],
      ['screenshot', step.screenshot || '(none)'],
    ]
    return (
      <dl className="grid grid-cols-[6rem_1fr] gap-x-3 gap-y-1 text-xs">
        {rows.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-muted-foreground">{k}</dt>
            <dd className="break-all font-mono">{v === '' ? '—' : v}</dd>
          </div>
        ))}
      </dl>
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
    <section className="flex h-full min-h-0 flex-col items-center justify-center rounded-lg border border-border bg-card p-8 text-sm text-muted-foreground">
      {children}
    </section>
  )
}
