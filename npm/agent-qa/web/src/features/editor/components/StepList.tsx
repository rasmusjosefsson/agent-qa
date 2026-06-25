// web/src/features/editor/components/StepList.tsx
import type { BufferRow } from '../types'
import { rowLabel } from '../compose'
import { cn } from '@/lib/utils'

const BADGE: Record<string, string> = {
  nav: 'bg-sky-500/15 text-sky-400',
  click: 'bg-violet-500/15 text-violet-400',
  fill: 'bg-emerald-500/15 text-emerald-400',
  press: 'bg-amber-500/15 text-amber-400',
  wait: 'bg-zinc-500/15 text-zinc-400',
  assert: 'bg-rose-500/15 text-rose-400',
  action: 'bg-zinc-500/15 text-zinc-400',
}

export function StepList({
  rows,
  onMove,
  onDelete,
}: {
  rows: BufferRow[]
  onMove: (from: number, to: number) => void
  onDelete: (index: number) => void
}) {
  if (rows.length === 0) {
    return <div className="px-3 py-6 text-center text-xs text-muted-foreground">No steps recorded yet.</div>
  }
  return (
    <ol className="flex flex-col gap-1">
      {rows.map((row, i) => {
        const { cls, title, detail } = rowLabel(row)
        return (
          <li
            key={i}
            className="group flex items-start gap-2 rounded-md border border-border bg-card px-2 py-1.5"
          >
            <span className="mt-0.5 w-4 shrink-0 text-right font-mono text-[10px] text-muted-foreground">{i}</span>
            <span className="min-w-0 flex-1">
              <span
                className={cn(
                  'inline-block rounded px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide',
                  BADGE[cls] || BADGE.action
                )}
              >
                {title}
              </span>
              <span className="mt-0.5 block truncate text-xs text-foreground" title={detail}>
                {detail}
              </span>
            </span>
            <span className="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
              <IconBtn label="↑" title="Move up" disabled={i === 0} onClick={() => onMove(i, i - 1)} />
              <IconBtn label="↓" title="Move down" disabled={i === rows.length - 1} onClick={() => onMove(i, i + 1)} />
              <IconBtn label="✕" title="Delete" danger onClick={() => onDelete(i)} />
            </span>
          </li>
        )
      })}
    </ol>
  )
}

function IconBtn({
  label,
  title,
  onClick,
  disabled,
  danger,
}: {
  label: string
  title: string
  onClick: () => void
  disabled?: boolean
  danger?: boolean
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        'grid h-5 w-5 place-items-center rounded text-[11px] text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30',
        danger && 'hover:text-destructive'
      )}
    >
      {label}
    </button>
  )
}

export default StepList
