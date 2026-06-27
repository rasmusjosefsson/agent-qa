// web/src/features/editor/components/ElementPicker.tsx
import { useState } from 'react'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { RefreshCwIcon } from 'lucide-react'
import type { AriaNode } from '../types'

export function ElementPicker({
  nodes,
  interactiveOnly,
  onInteractiveChange,
  onSnapshot,
  onPick,
}: {
  nodes: AriaNode[]
  interactiveOnly: boolean
  onInteractiveChange: (v: boolean) => void
  onSnapshot: () => void
  onPick: (n: AriaNode) => void
}) {
  const [filter, setFilter] = useState('')
  const f = filter.trim().toLowerCase()
  const shown = nodes.filter(
    (n) => !f || (n.role || '').toLowerCase().includes(f) || (n.name || '').toLowerCase().includes(f)
  )

  return (
    <div className="flex h-full min-h-0 flex-col gap-2 p-3">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-muted-foreground">Element picker</span>
        <span className="flex items-center gap-2">
          <label
            className="flex items-center gap-1.5 text-[11px] text-muted-foreground"
            title="Show only interactive elements (buttons, links, inputs)"
          >
            <Checkbox
              checked={interactiveOnly}
              onCheckedChange={(v) => onInteractiveChange(v === true)}
              className="size-3.5"
            />
            interactive only
          </label>
          <button
            type="button"
            title="Snapshot the live page"
            aria-label="Snapshot the live page"
            onClick={onSnapshot}
            className="grid h-6 w-6 place-items-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            <RefreshCwIcon className="size-3.5" />
          </button>
        </span>
      </div>

      <Input
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
        placeholder="filter by role or name…"
        className="h-7 text-xs"
      />

      {shown.length === 0 ? (
        <div className="px-1 py-4 text-center text-xs text-muted-foreground">
          Snapshot the live page to pick an element.
        </div>
      ) : (
        <ul className="min-h-0 flex-1 overflow-auto text-xs">
          {shown.map((n, i) => (
            <li
              key={i}
              onClick={n.pickable ? () => onPick(n) : undefined}
              style={{ paddingLeft: 8 + Math.min(n.depth ?? 0, 12) * 12 }}
              className={cn(
                'rounded py-0.5 pr-1',
                n.pickable ? 'cursor-pointer hover:bg-muted' : 'opacity-60'
              )}
            >
              <span className="text-sky-400">{n.role || ''}</span>
              {n.name ? <span className="text-foreground"> “{n.name}”</span> : null}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

export default ElementPicker
