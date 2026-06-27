import { useEffect, useRef, useState } from 'react'
import { CircleDotIcon, CheckCircle2Icon, ImageIcon, ImageOffIcon } from 'lucide-react'
import { recordingArtifactUrl, type RecordingState, type RecordingStep } from '@/lib/api'
import { cn } from '@/lib/utils'

const KIND_STYLES: Record<string, string> = {
  navigation: 'bg-sky-500/15 text-sky-300 border-sky-500/30',
  action: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30',
  wait: 'bg-amber-500/15 text-amber-300 border-amber-500/30',
  assert: 'bg-violet-500/15 text-violet-300 border-violet-500/30',
}

function summarize(step: RecordingStep): string {
  const p = (step.payload || {}) as Record<string, unknown>
  const intent = (step.intent || p.intent || '') as string
  switch (step.kind) {
    case 'navigation':
      return String(p.route ?? intent ?? '')
    case 'action': {
      const method = String(p.method ?? 'action')
      const args = Array.isArray(p.args) ? p.args : []
      const inner = args
        .map((a) => (typeof a === 'string' ? `"${a}"` : JSON.stringify(a)))
        .join(', ')
      const call = `${method}(${inner})`
      return intent ? `${call} — ${intent}` : call
    }
    case 'wait': {
      const c = (p.condition ?? {}) as Record<string, unknown>
      const kind = String(c.kind ?? 'wait')
      const detail = c.pattern ?? c.selector ?? c.text ?? c.ms ?? c.duration ?? intent ?? ''
      return detail ? `${kind}: ${detail}` : kind
    }
    case 'assert': {
      const kind = String(p.kind ?? 'assert')
      const args = Array.isArray(p.args) ? p.args.join(', ') : ''
      const tail = intent ? ` — ${intent}` : ''
      return `${kind}${args ? `(${args})` : ''}${tail}`
    }
    default:
      return intent || JSON.stringify(p)
  }
}

function shortTime(iso?: string | null): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  return d.toLocaleTimeString(undefined, { hour12: false })
}

// Live view of the chat's current recording: renders the steps recorded so far
// (polled by the parent), each expandable to its keyframe screenshot. Mirrors
// what the agent is capturing as it drives the browser.
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
      const el = listRef.current
      if (el && atBottomRef.current) el.scrollTop = el.scrollHeight
    }
  }, [steps.length])

  if (!rec || !rec.sid) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 p-6 text-center text-sm text-muted-foreground">
        <ImageIcon className="size-6 opacity-40" />
        <div>No recording yet.</div>
        <p className="max-w-xs text-xs">
          Ask the agent to record a scenario. Steps and screenshots will appear here as it drives
          the browser.
        </p>
      </div>
    )
  }

  const onScroll = () => {
    const el = listRef.current
    if (!el) return
    atBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* header */}
      <div className="flex items-start justify-between gap-2 border-b border-border px-3 py-2">
        <div className="min-w-0">
          <div className="truncate text-sm font-medium" title={rec.intent || rec.sid}>
            {rec.intent || rec.sid}
          </div>
          <div className="truncate font-mono text-[11px] text-muted-foreground" title={rec.sid}>
            {rec.sid}
            {rec.session ? ` · ${rec.session}` : ''}
          </div>
        </div>
        {rec.flushed ? (
          <span className="inline-flex shrink-0 items-center gap-1 rounded-sm border border-emerald-500/30 bg-emerald-500/10 px-2 py-0.5 text-[11px] font-medium text-emerald-300">
            <CheckCircle2Icon className="size-3" /> saved
          </span>
        ) : (
          <span className="inline-flex shrink-0 items-center gap-1 rounded-sm border border-red-500/30 bg-red-500/10 px-2 py-0.5 text-[11px] font-medium text-red-300">
            <CircleDotIcon className="size-3 animate-pulse" /> recording
          </span>
        )}
      </div>

      {/* steps */}
      <div ref={listRef} onScroll={onScroll} className="min-h-0 flex-1 overflow-auto p-2">
        {steps.length === 0 ? (
          <div className="px-2 py-6 text-center text-xs text-muted-foreground">
            Waiting for the first step…
          </div>
        ) : (
          <ol className="space-y-1">
            {steps.map((s) => {
              const open = openStep === s.stepId
              return (
                <li key={s.stepId} className="rounded-md border border-border bg-card/40">
                  <button
                    type="button"
                    onClick={() => setOpenStep(open ? null : s.stepId)}
                    className="flex w-full items-center gap-2 px-2 py-1.5 text-left"
                  >
                    <span className="w-5 shrink-0 text-right font-mono text-[11px] text-muted-foreground">
                      {s.stepIndex}
                    </span>
                    <span
                      className={cn(
                        'shrink-0 rounded border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide',
                        KIND_STYLES[s.kind] || 'border-border bg-muted text-muted-foreground'
                      )}
                    >
                      {s.kind}
                    </span>
                    <span className="min-w-0 flex-1 truncate font-mono text-xs" title={summarize(s)}>
                      {summarize(s)}
                    </span>
                    <span className="shrink-0 font-mono text-[10px] text-muted-foreground/70">
                      {shortTime(s.recordedAt)}
                    </span>
                  </button>
                  {open && (
                    <div className="border-t border-border p-2">
                      {noShot.has(s.stepId) ? (
                        <div className="flex items-center gap-2 px-1 py-3 text-xs text-muted-foreground">
                          <ImageOffIcon className="size-4 opacity-50" />
                          No keyframe captured for this step.
                        </div>
                      ) : (
                        <img
                          src={recordingArtifactUrl(cid, s.stepId, 'screenshot')}
                          alt={`step ${s.stepIndex} screenshot`}
                          loading="lazy"
                          className="w-full rounded border border-border"
                          onError={() =>
                            setNoShot((prev) => {
                              const next = new Set(prev)
                              next.add(s.stepId)
                              return next
                            })
                          }
                        />
                      )}
                    </div>
                  )}
                </li>
              )
            })}
          </ol>
        )}
      </div>
    </div>
  )
}

export default RecordingView
