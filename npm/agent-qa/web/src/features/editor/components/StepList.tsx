// web/src/features/editor/components/StepList.tsx
import type { BufferRow } from '../types'
import { rowLabel } from '../compose'
import { cn } from '@/lib/utils'
import { useState, type ReactNode } from 'react'
import { ArrowUpIcon, ArrowDownIcon, XIcon, PencilIcon, CameraIcon, PlayIcon } from 'lucide-react'

const BADGE: Record<string, string> = {
  nav: 'bg-sky-500/15 text-sky-400',
  click: 'bg-violet-500/15 text-violet-400',
  fill: 'bg-emerald-500/15 text-emerald-400',
  press: 'bg-amber-500/15 text-amber-400',
  wait: 'bg-zinc-500/15 text-zinc-400',
  assert: 'bg-rose-500/15 text-rose-400',
  action: 'bg-zinc-500/15 text-zinc-400',
}

// The editable draft of a step is its JSON minus the recorder-assigned
// id/kind — same contract as `record-step` / `buffer edit`.
function draftOf(row: BufferRow): string {
  const { id: _id, kind: _kind, ...rest } = row.step as Record<string, unknown>
  return JSON.stringify(rest, null, 2)
}

export function StepList({
  rows,
  onMove,
  onDelete,
  onEdit,
  onAddShot,
  onRun,
}: {
  rows: BufferRow[]
  onMove: (from: number, to: number) => void
  onDelete: (index: number) => void
  onEdit?: (index: number, draft: Record<string, unknown>) => Promise<boolean> | boolean
  onAddShot?: (index: number, stepId: string) => void
  // Dispatch one buffered step against the live session (run-step) —
  // author-time feedback without flushing the buffer.
  onRun?: (row: BufferRow) => void
}) {
  const [editing, setEditing] = useState<number | null>(null)
  const [draft, setDraft] = useState('')
  const [editError, setEditError] = useState('')

  const beginEdit = (i: number, row: BufferRow) => {
    setEditing(i)
    setDraft(draftOf(row))
    setEditError('')
  }

  const saveEdit = async () => {
    if (editing == null || !onEdit) return
    let parsed: Record<string, unknown>
    try {
      parsed = JSON.parse(draft)
    } catch {
      setEditError('not valid JSON')
      return
    }
    if (parsed == null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      setEditError('draft must be a JSON object')
      return
    }
    const ok = await onEdit(editing, parsed)
    if (ok) setEditing(null)
  }
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
            className="group flex flex-wrap items-start gap-2 rounded-md border border-border bg-card px-2 py-1.5"
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
              {onAddShot && row.step.kind === 'do' && (
                <IconBtn
                  icon={<CameraIcon className="size-3.5" />}
                  title="Add a visual check ({shot} claim) after this step"
                  onClick={() => onAddShot(i, row.stepId)}
                />
              )}
              {onRun && (
                <IconBtn icon={<PlayIcon className="size-3.5" />} title="Run this step live" onClick={() => onRun(row)} />
              )}
              {onEdit && (
                <IconBtn icon={<PencilIcon className="size-3" />} title="Edit step (draft JSON)" onClick={() => beginEdit(i, row)} />
              )}
              <IconBtn icon={<ArrowUpIcon className="size-3.5" />} title="Move up" disabled={i === 0} onClick={() => onMove(i, i - 1)} />
              <IconBtn icon={<ArrowDownIcon className="size-3.5" />} title="Move down" disabled={i === rows.length - 1} onClick={() => onMove(i, i + 1)} />
              <IconBtn icon={<XIcon className="size-3.5" />} title="Delete" danger onClick={() => onDelete(i)} />
            </span>
            {editing === i && (
              <div className="mt-1.5 w-full basis-full space-y-1.5">
                <textarea
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  spellCheck={false}
                  rows={Math.min(14, draft.split('\n').length + 1)}
                  className="w-full resize-y rounded-md border border-border bg-background p-2 font-mono text-[11px] leading-snug outline-none focus:border-ring"
                />
                {editError && <div className="text-[11px] text-destructive">{editError}</div>}
                <div className="flex justify-end gap-1.5">
                  <button
                    type="button"
                    onClick={() => setEditing(null)}
                    className="rounded-md border border-border px-2 py-0.5 text-[11px] text-muted-foreground hover:bg-muted"
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    onClick={() => void saveEdit()}
                    className="rounded-md bg-primary px-2 py-0.5 text-[11px] font-medium text-primary-foreground hover:opacity-90"
                  >
                    Save
                  </button>
                </div>
              </div>
            )}
          </li>
        )
      })}
    </ol>
  )
}

function IconBtn({
  icon,
  title,
  onClick,
  disabled,
  danger,
}: {
  icon: ReactNode
  title: string
  onClick: () => void
  disabled?: boolean
  danger?: boolean
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        'grid h-5 w-5 place-items-center rounded text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30',
        danger && 'hover:text-destructive'
      )}
    >
      {icon}
    </button>
  )
}

export default StepList
