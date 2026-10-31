import { useMemo, useState } from 'react'
import { cn } from '@/lib/utils'
import { lineDiff, type DiffRow, type LineSpan } from '@/lib/line-diff'

function Spans({
  spans,
  fallback,
  markClass,
}: {
  spans?: LineSpan[]
  fallback?: string
  markClass: string
}) {
  if (!spans || spans.length === 0) return <>{fallback}</>
  return (
    <>
      {spans.map((s, i) =>
        s.changed ? (
          <mark key={i} className={cn('rounded-sm px-0.5 text-inherit', markClass)}>
            {s.text}
          </mark>
        ) : (
          <span key={i}>{s.text}</span>
        )
      )}
    </>
  )
}

function rowClass(kind: DiffRow['kind'], side: 'before' | 'after'): string {
  if (kind === 'same') return 'text-muted-foreground'
  if (kind === 'change') return side === 'before' ? 'bg-destructive/10 text-destructive' : 'bg-success/10 text-success'
  if (kind === 'del') return side === 'before' ? 'bg-destructive/10 text-destructive' : 'bg-muted/30'
  return side === 'after' ? 'bg-success/10 text-success' : 'bg-muted/30'
}

// Side-by-side (default) or unified line diff of two ARIA snapshots.
// `changedOnly` drops equal lines and leaves a gap marker where they were.
export function TextLineDiff({
  before,
  after,
  beforeLabel = 'baseline',
  afterLabel = 'current',
}: {
  before: string
  after: string
  beforeLabel?: string
  afterLabel?: string
}) {
  const [mode, setMode] = useState<'side' | 'unified'>('side')
  const [changedOnly, setChangedOnly] = useState(false)
  const rows = useMemo(() => lineDiff(before, after), [before, after])
  const changed = rows.filter((r) => r.kind !== 'same').length
  const numbered = useMemo(() => {
    let beforeNo = 0
    let afterNo = 0
    return rows.map((row) => ({
      row,
      beforeNo: row.before !== undefined ? ++beforeNo : null,
      afterNo: row.after !== undefined ? ++afterNo : null,
    }))
  }, [rows])

  const visible = useMemo(() => {
    if (!changedOnly) return numbered.map((entry) => ({ ...entry, gap: false }))
    const out: { row: DiffRow; beforeNo: number | null; afterNo: number | null; gap: boolean }[] = []
    let skipped = false
    for (const entry of numbered) {
      if (entry.row.kind === 'same') {
        skipped = true
        continue
      }
      out.push({ ...entry, gap: skipped })
      skipped = false
    }
    return out
  }, [numbered, changedOnly])

  return (
    <div className="overflow-hidden rounded border border-border bg-muted/20">
      <div className="flex flex-wrap items-center gap-1.5 border-b border-border px-2 py-1.5">
        <div className="flex overflow-hidden rounded border border-border" role="tablist" aria-label="diff layout">
          {(['side', 'unified'] as const).map((m) => (
            <button
              key={m}
              type="button"
              role="tab"
              aria-selected={mode === m}
              onClick={() => setMode(m)}
              className={cn(
                'px-2 py-0.5 text-[11px]',
                mode === m ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:text-foreground'
              )}
            >
              {m === 'side' ? 'side-by-side' : 'unified'}
            </button>
          ))}
        </div>
        <button
          type="button"
          aria-pressed={changedOnly}
          onClick={() => setChangedOnly((v) => !v)}
          className={cn(
            'rounded border px-2 py-0.5 text-[11px]',
            changedOnly ? 'border-primary/40 text-primary' : 'border-border text-muted-foreground'
          )}
        >
          changed only
        </button>
        <span className="ml-auto text-[11px] text-muted-foreground">
          {changed === 0 ? 'snapshots match' : `${changed} changed line${changed === 1 ? '' : 's'}`}
        </span>
      </div>
      {changed === 0 ? (
        <p className="px-3 py-4 text-xs text-muted-foreground">The two snapshots are identical.</p>
      ) : mode === 'side' ? (
        <div className="max-h-80 overflow-auto font-mono text-[11px] leading-relaxed">
          <div className="sticky top-0 grid grid-cols-2 border-b border-border bg-muted/80 text-[10px] uppercase tracking-wide text-muted-foreground">
            <div className="px-2 py-1">{beforeLabel}</div>
            <div className="border-l border-border px-2 py-1">{afterLabel}</div>
          </div>
          {visible.map(({ row, gap, beforeNo, afterNo }, i) => (
            <div key={i}>
              {gap && (
                <div className="border-b border-border/60 px-2 py-0.5 text-center text-muted-foreground/70">···</div>
              )}
              <div className="grid grid-cols-2 border-b border-border/40">
                <div className={cn('flex min-w-0 gap-2 px-2 py-0.5', rowClass(row.kind, 'before'))}>
                  <span className="w-6 shrink-0 text-right opacity-50">{beforeNo ?? ''}</span>
                  <span className="min-w-0 whitespace-pre-wrap break-all">
                    {row.before !== undefined ? (
                      <Spans spans={row.beforeSpans} fallback={row.before} markClass="bg-destructive/25" />
                    ) : null}
                  </span>
                </div>
                <div className={cn('flex min-w-0 gap-2 border-l border-border/40 px-2 py-0.5', rowClass(row.kind, 'after'))}>
                  <span className="w-6 shrink-0 text-right opacity-50">{afterNo ?? ''}</span>
                  <span className="min-w-0 whitespace-pre-wrap break-all">
                    {row.after !== undefined ? (
                      <Spans spans={row.afterSpans} fallback={row.after} markClass="bg-success/25" />
                    ) : null}
                  </span>
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <pre className="max-h-80 overflow-auto whitespace-pre-wrap break-all p-2 font-mono text-[11px] leading-relaxed">
          {visible.map(({ row, gap }, i) => (
            <span key={i}>
              {gap && <span className="block text-muted-foreground">···</span>}
              {row.kind === 'same' && <span className="block text-muted-foreground"> {row.before}</span>}
              {(row.kind === 'del' || row.kind === 'change') && (
                <span className="block text-destructive">
                  -<Spans spans={row.beforeSpans} fallback={row.before} markClass="bg-destructive/25" />
                </span>
              )}
              {(row.kind === 'add' || row.kind === 'change') && (
                <span className="block text-success">
                  +<Spans spans={row.afterSpans} fallback={row.after} markClass="bg-success/25" />
                </span>
              )}
            </span>
          ))}
        </pre>
      )}
    </div>
  )
}
