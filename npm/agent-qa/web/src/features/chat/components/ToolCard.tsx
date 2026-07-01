// web/src/features/chat/components/ToolCard.tsx
// Compact, collapsible tool card. Renders a single-line title summarizing what
// the tool did (e.g. "Read core SKILL.md", "$ npm test") with a status icon;
// arguments and raw output stay hidden until expanded. Collapsed by default,
// auto-opens on error so failures surface without a click. Built on the same
// Radix Collapsible primitive as Reasoning.tsx.
import { useEffect, useState } from 'react'
import {
  BotIcon,
  CheckIcon,
  ChevronDownIcon,
  CircleAlertIcon,
  FileTextIcon,
  GlobeIcon,
  type LucideIcon,
  Loader2Icon,
  PencilIcon,
  SearchIcon,
  SparklesIcon,
  TerminalIcon,
  WrenchIcon,
} from 'lucide-react'
import type { ChatItem } from '@/lib/types'
import { fmtArgs, maskHome, toolSummary } from '../chatReducer'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { cn } from '@/lib/utils'

type ToolItem = Extract<ChatItem, { kind: 'tool' }>

function iconFor(name?: string): LucideIcon {
  switch ((name || '').toLowerCase()) {
    case 'bash':
    case 'shell':
      return TerminalIcon
    case 'read':
      return FileTextIcon
    case 'write':
    case 'edit':
    case 'multiedit':
      return PencilIcon
    case 'grep':
    case 'glob':
      return SearchIcon
    case 'skill':
      return SparklesIcon
    case 'task':
    case 'agent':
      return BotIcon
    case 'webfetch':
    case 'websearch':
      return GlobeIcon
    default:
      return WrenchIcon
  }
}

function StatusIcon({ status }: { status: ToolItem['status'] }) {
  if (status === 'running')
    return <Loader2Icon className="size-3.5 shrink-0 animate-spin text-amber-400" />
  if (status === 'err') return <CircleAlertIcon className="size-3.5 shrink-0 text-destructive" />
  return <CheckIcon className="size-3.5 shrink-0 text-emerald-500/70" />
}

export function ToolCard({ item }: { item: ToolItem }) {
  const [open, setOpen] = useState(false)
  // Mask the OS home dir (`/Users/<name>` → `~`) in the expanded detail too, so
  // the username never shows and long absolute paths stay readable.
  const args = maskHome(fmtArgs(item.args))
  const out = maskHome(item.out || '')
  const hasDetail = !!(args || out)
  const title = toolSummary(item.name, item.args)
  const Icon = iconFor(item.name)
  // Show the raw tool name only when the title doesn't already lead with it
  // (so a bash call titled "Read skill core" still reveals it ran `bash`,
  // while "Read SKILL.md" doesn't repeat "read").
  const firstWord = title.toLowerCase().replace(/^[^a-z0-9]+/, '').split(/\s+/)[0]
  const showName = !!item.name && item.name.toLowerCase() !== firstWord

  // Surface failures without a click.
  useEffect(() => {
    if (item.status === 'err') setOpen(true)
  }, [item.status])

  return (
    <Collapsible
      open={open && hasDetail}
      onOpenChange={setOpen}
      className="rounded-lg border border-border bg-card/60 text-card-foreground"
    >
      <CollapsibleTrigger
        disabled={!hasDetail}
        className="flex w-full items-center gap-2 px-3 py-1.5 text-left transition-colors hover:bg-muted/40 disabled:cursor-default disabled:hover:bg-transparent"
      >
        <Icon className="size-3.5 shrink-0 text-muted-foreground" />
        <span className="min-w-0 flex-1 truncate text-xs text-foreground" title={title}>
          {title}
        </span>
        {showName && (
          <span className="shrink-0 font-mono text-[10px] text-muted-foreground/50">{item.name}</span>
        )}
        <StatusIcon status={item.status} />
        {hasDetail && (
          <ChevronDownIcon
            className={cn(
              'size-3.5 shrink-0 text-muted-foreground transition-transform',
              open ? 'rotate-180' : 'rotate-0'
            )}
          />
        )}
      </CollapsibleTrigger>

      <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-out data-[state=open]:animate-in data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0">
        {args && (
          <div className="border-t border-border px-3 py-2">
            <div className="mb-1 text-[10px] font-medium uppercase tracking-wide text-muted-foreground/70">
              arguments
            </div>
            <pre className="max-h-60 overflow-auto whitespace-pre-wrap break-words text-[11px] text-muted-foreground">
              {args}
            </pre>
          </div>
        )}
        {out && (
          <pre className="max-h-72 overflow-auto whitespace-pre-wrap break-words border-t border-border px-3 py-2 text-[11px] text-muted-foreground">
            {out}
          </pre>
        )}
      </CollapsibleContent>
    </Collapsible>
  )
}

export default ToolCard
