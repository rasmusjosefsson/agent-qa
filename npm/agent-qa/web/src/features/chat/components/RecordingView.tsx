import { useEffect, useRef, useState } from 'react'
import { CircleDotIcon, CheckCircle2Icon, ImageIcon, ImageOffIcon } from 'lucide-react'
import { recordingArtifactUrl, type RecordingState, type RecordingStep } from '@/lib/api'
import { cn } from '@/lib/utils'

const KIND_STYLES: Record<string, string> = {
  do: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30',
  check: 'bg-violet-500/15 text-violet-300 border-violet-500/30',
}

function summarize(step: RecordingStep): string {
  const payload = step.payload || {}
  if (payload.verb === 'goto') return String(payload.value?.literal ?? step.intent ?? '')
  if (payload.on?.name) return `${payload.verb || 'do'} ${payload.on.name}`
  return step.intent || payload.verb || step.kind
}

function shortTime(iso?: string | null): string {
  if (!iso) return ''
  const date = new Date(iso)
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleTimeString(undefined, { hour12: false })
}

export function RecordingView({ cid, rec }: { cid: string; rec: RecordingState | null }) {
  const [openStep, setOpenStep] = useState<string | null>(null)
  const [noShot, setNoShot] = useState<Set<string>>(() => new Set())
  const listRef = useRef<HTMLDivElement | null>(null)
  const atBottomRef = useRef(true)
  const prevCount = useRef(0)
  const steps = rec?.steps ?? []

  useEffect(() => {
    if (steps.length !== prevCount.current) {
      prevCount.current = steps.length
      const element = listRef.current
      if (element && atBottomRef.current) element.scrollTop = element.scrollHeight
    }
  }, [steps.length])

  if (!rec || !rec.sid) return <div className="flex h-full flex-col items-center justify-center gap-2 p-6 text-center text-sm text-muted-foreground"><ImageIcon className="size-6 opacity-40" /><div>No recording yet.</div></div>

  return <div className="flex h-full min-h-0 flex-col">
    <div className="flex items-start justify-between gap-2 border-b border-border px-3 py-2"><div className="min-w-0"><div className="truncate text-sm font-medium" title={rec.intent || rec.sid}>{rec.intent || rec.sid}</div><div className="truncate font-mono text-[11px] text-muted-foreground" title={rec.sid}>{rec.sid}{rec.session ? ` · ${rec.session}` : ''}</div></div>{rec.flushed ? <span className="inline-flex shrink-0 items-center gap-1 rounded-sm border border-emerald-500/30 bg-emerald-500/10 px-2 py-0.5 text-[11px] font-medium text-emerald-300"><CheckCircle2Icon className="size-3" /> saved</span> : <span className="inline-flex shrink-0 items-center gap-1 rounded-sm border border-red-500/30 bg-red-500/10 px-2 py-0.5 text-[11px] font-medium text-red-300"><CircleDotIcon className="size-3 animate-pulse" /> recording</span>}</div>
    <div ref={listRef} onScroll={() => { const element = listRef.current; if (element) atBottomRef.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80 }} className="min-h-0 flex-1 overflow-auto p-2">{steps.length === 0 ? <div className="px-2 py-6 text-center text-xs text-muted-foreground">Waiting for the first step…</div> : <ol className="space-y-1">{steps.map((step) => { const open = openStep === step.stepId; return <li key={step.stepId} className="rounded-md border border-border bg-card/40"><button type="button" onClick={() => setOpenStep(open ? null : step.stepId)} className="flex w-full items-center gap-2 px-2 py-1.5 text-left"><span className="w-5 shrink-0 text-right font-mono text-[11px] text-muted-foreground">{step.stepIndex}</span><span className={cn('shrink-0 rounded border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide', KIND_STYLES[step.kind] || 'border-border bg-muted text-muted-foreground')}>{step.kind}</span><span className="min-w-0 flex-1 truncate font-mono text-xs" title={summarize(step)}>{summarize(step)}</span><span className="shrink-0 font-mono text-[10px] text-muted-foreground/70">{shortTime(step.recordedAt)}</span></button>{open && (noShot.has(step.stepId) ? <div className="flex items-center gap-2 px-3 py-4 text-xs text-muted-foreground"><ImageOffIcon className="size-4 opacity-50" />No keyframe captured for this step.</div> : <img src={recordingArtifactUrl(cid, step.stepId, 'screenshot')} alt={`step ${step.stepIndex} screenshot`} loading="lazy" className="w-full rounded border border-border" onError={() => setNoShot((previous) => new Set(previous).add(step.stepId))} />)}</li> })}</ol>}</div>
  </div>
}

export default RecordingView
