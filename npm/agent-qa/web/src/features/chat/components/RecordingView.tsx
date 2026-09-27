import { useEffect, useRef, useState } from 'react'
import { CircleDotIcon, CheckCircle2Icon, ClipboardCheckIcon, ImageIcon, ImageOffIcon, PauseIcon, PlayIcon, PencilIcon, Trash2Icon } from 'lucide-react'
import {
  recordingArtifactUrl,
  pauseChatRecording,
  resumeChatRecording,
  editChatRecordingStep,
  deleteChatRecordingStep,
  checkChatRecording,
  type RecordingState,
  type RecordingStep,
} from '@/lib/api'
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
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [report, setReport] = useState<string | null>(null)
  const [editing, setEditing] = useState<number | null>(null)
  const [draft, setDraft] = useState('')
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
    <div className="flex items-start justify-between gap-2 border-b border-border px-3 py-2">
      <div className="min-w-0">
        <div className="truncate text-sm font-medium" title={rec.intent || rec.sid}>{rec.intent || rec.sid}</div>
        <div className="truncate font-mono text-[11px] text-muted-foreground" title={rec.sid}>{rec.sid}{rec.session ? ` · ${rec.session}` : ''}</div>
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        {editable && (
          <button
            type="button"
            disabled={busy}
            title="Validate the buffer as the scenario flush would write (schema + lint)"
            onClick={() => {
              setBusy(true)
              setError('')
              void checkChatRecording(cid)
                .then((r) => setReport(r.report))
                .finally(() => setBusy(false))
            }}
            className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-0.5 text-[11px] text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-40"
          >
            <ClipboardCheckIcon className="size-3" />
            Check
          </button>
        )}
        {editable && (
          <button
            type="button"
            disabled={busy}
            onClick={() => void run(() => (rec.paused ? resumeChatRecording(cid) : pauseChatRecording(cid)))}
            className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-0.5 text-[11px] text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-40"
          >
            {rec.paused ? <PlayIcon className="size-3" /> : <PauseIcon className="size-3" />}
            {rec.paused ? 'Resume' : 'Pause'}
          </button>
        )}
        {rec.flushed ? <span className="inline-flex shrink-0 items-center gap-1 rounded-sm border border-emerald-500/30 bg-emerald-500/10 px-2 py-0.5 text-[11px] font-medium text-emerald-300"><CheckCircle2Icon className="size-3" /> saved</span> : rec.paused ? <span className="inline-flex shrink-0 items-center gap-1 rounded-sm border border-amber-500/30 bg-amber-500/10 px-2 py-0.5 text-[11px] font-medium text-amber-300"><PauseIcon className="size-3" /> paused</span> : <span className="inline-flex shrink-0 items-center gap-1 rounded-sm border border-red-500/30 bg-red-500/10 px-2 py-0.5 text-[11px] font-medium text-red-300"><CircleDotIcon className="size-3 animate-pulse" /> recording</span>}
      </div>
    </div>
    {error && <div className="border-b border-border bg-destructive/10 px-3 py-1.5 text-[11px] text-destructive">{error}</div>}
    {report && (
      <div className="relative border-b border-border bg-muted/40 px-3 py-1.5">
        <button
          type="button"
          aria-label="Dismiss check report"
          onClick={() => setReport(null)}
          className="absolute right-1.5 top-1.5 grid h-4 w-4 place-items-center rounded text-muted-foreground hover:text-foreground"
        >
          ×
        </button>
        <pre className="max-h-44 overflow-auto whitespace-pre-wrap pr-4 font-mono text-[11px] leading-snug text-foreground">
          {report}
        </pre>
      </div>
    )}
    <div ref={listRef} onScroll={() => { const element = listRef.current; if (element) atBottomRef.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80 }} className="min-h-0 flex-1 overflow-auto p-2">{steps.length === 0 ? <div className="px-2 py-6 text-center text-xs text-muted-foreground">Waiting for the first step…</div> : <ol className="space-y-1">{steps.map((step) => { const open = openStep === step.stepId; return <li key={step.stepId} className="rounded-md border border-border bg-card/40">
      <button type="button" onClick={() => { setOpenStep(open ? null : step.stepId); setEditing(null) }} className="flex w-full items-center gap-2 px-2 py-1.5 text-left"><span className="w-5 shrink-0 text-right font-mono text-[11px] text-muted-foreground">{step.stepIndex}</span><span className={cn('shrink-0 rounded border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide', KIND_STYLES[step.kind] || 'border-border bg-muted text-muted-foreground')}>{step.kind}</span><span className="min-w-0 flex-1 truncate font-mono text-xs" title={summarize(step)}>{summarize(step)}</span><span className="shrink-0 font-mono text-[10px] text-muted-foreground/70">{shortTime(step.recordedAt)}</span></button>
      {open && <div className="border-t border-border/60 px-2 pb-2 pt-1.5">
        {noShot.has(step.stepId) ? <div className="flex items-center gap-2 px-1 py-3 text-xs text-muted-foreground"><ImageOffIcon className="size-4 opacity-50" />No keyframe captured for this step.</div> : <img src={recordingArtifactUrl(cid, step.stepId, 'screenshot')} alt={`step ${step.stepIndex} screenshot`} loading="lazy" className="w-full rounded border border-border" onError={() => setNoShot((previous) => new Set(previous).add(step.stepId))} />}
        {editable && editing !== step.stepIndex && (
          <div className="mt-1.5 flex justify-end gap-1.5">
            <button type="button" disabled={busy} onClick={() => { setEditing(step.stepIndex); setDraft(draftOf(step)); setError('') }} className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-0.5 text-[11px] text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-40"><PencilIcon className="size-3" />Edit draft</button>
            <button type="button" disabled={busy} onClick={() => void run(() => deleteChatRecordingStep(cid, step.stepIndex))} className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-0.5 text-[11px] text-muted-foreground hover:bg-destructive/15 hover:text-destructive disabled:opacity-40"><Trash2Icon className="size-3" />Delete</button>
          </div>
        )}
        {editable && editing === step.stepIndex && (
          <div className="mt-1.5 space-y-1.5">
            <textarea
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              spellCheck={false}
              rows={Math.min(14, draft.split('\n').length + 1)}
              className="w-full resize-y rounded-md border border-border bg-background p-2 font-mono text-[11px] leading-snug outline-none focus:border-ring"
            />
            <div className="flex justify-end gap-1.5">
              <button type="button" onClick={() => setEditing(null)} className="rounded-md border border-border px-2 py-0.5 text-[11px] text-muted-foreground hover:bg-muted">Cancel</button>
              <button type="button" disabled={busy} onClick={() => void saveEdit(step)} className="rounded-md bg-primary px-2 py-0.5 text-[11px] font-medium text-primary-foreground hover:opacity-90 disabled:opacity-40">Save</button>
            </div>
          </div>
        )}
      </div>}
    </li> })}</ol>}</div>
  </div>
}

export default RecordingView
