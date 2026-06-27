// web/src/features/chat/components/ToolCard.tsx
import { useState } from 'react'
import type { ChatItem } from '@/lib/types'
import { fmtArgs } from '../chatReducer'
import { cn } from '@/lib/utils'

type ToolItem = Extract<ChatItem, { kind: 'tool' }>

const STATUS_LABEL: Record<ToolItem['status'], string> = {
  running: 'running',
  ok: 'done',
  err: 'error',
}

export function ToolCard({ item }: { item: ToolItem }) {
  const [argsOpen, setArgsOpen] = useState(false)
  const args = fmtArgs(item.args)

  return (
    <div className="rounded-lg border border-border bg-card/60 text-card-foreground">
      <div className="flex items-center justify-between gap-2 px-3 py-2">
        <span className="font-mono text-xs font-medium">{item.name || 'tool'}</span>
        <span
          className={cn(
            'rounded-sm px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide',
            item.status === 'running' && 'bg-amber-500/15 text-amber-400',
            item.status === 'ok' && 'bg-emerald-500/15 text-emerald-400',
            item.status === 'err' && 'bg-destructive/15 text-destructive'
          )}
        >
          {STATUS_LABEL[item.status]}
        </span>
      </div>

      {args && (
        <div className="border-t border-border px-3 py-1.5">
          <button
            type="button"
            onClick={() => setArgsOpen((v) => !v)}
            className="text-[11px] text-muted-foreground hover:text-foreground"
          >
            {argsOpen ? '▾' : '▸'} arguments
          </button>
          {argsOpen && (
            <pre className="mt-1 max-h-60 overflow-auto whitespace-pre-wrap break-words text-[11px] text-muted-foreground">
              {args}
            </pre>
          )}
        </div>
      )}

      {item.out && (
        <pre className="max-h-72 overflow-auto whitespace-pre-wrap break-words border-t border-border px-3 py-2 text-[11px] text-muted-foreground">
          {item.out}
        </pre>
      )}
    </div>
  )
}

export default ToolCard
