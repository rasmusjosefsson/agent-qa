// web/src/features/chat/components/Reasoning.tsx
// Inline, collapsible "thinking" block — ported from the AI Elements
// AI-Elements Reasoning. Auto-opens while the model is thinking, shows a
// shimmering "Thinking…" label, then auto-collapses to "Thought for Ns" once
// the thinking stream ends. Content renders via the shared Markdown renderer.
import { BrainIcon, ChevronDownIcon } from 'lucide-react'
import { memo, useCallback, useEffect, useRef, useState } from 'react'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { cn } from '@/lib/utils'
import { Markdown } from './Markdown'

const AUTO_CLOSE_DELAY = 1000
const MS_IN_S = 1000

export const Reasoning = memo(function Reasoning({
  text,
  isStreaming = false,
  defaultOpen,
}: {
  text: string
  isStreaming?: boolean
  defaultOpen?: boolean
}) {
  const resolvedDefaultOpen = defaultOpen ?? isStreaming
  const [isOpen, setIsOpen] = useState<boolean>(resolvedDefaultOpen)
  const [duration, setDuration] = useState<number | undefined>(undefined)

  const hasEverStreamed = useRef(isStreaming)
  const hasAutoClosed = useRef(false)
  const startTime = useRef<number | null>(null)

  // Measure how long the thinking stream ran.
  useEffect(() => {
    if (isStreaming) {
      hasEverStreamed.current = true
      if (startTime.current === null) startTime.current = Date.now()
    } else if (startTime.current !== null) {
      setDuration(Math.ceil((Date.now() - startTime.current) / MS_IN_S))
      startTime.current = null
    }
  }, [isStreaming])

  // Auto-open on stream start.
  useEffect(() => {
    if (isStreaming && !isOpen) setIsOpen(true)
  }, [isStreaming, isOpen, setIsOpen])

  // Auto-close shortly after the stream ends (once).
  useEffect(() => {
    if (hasEverStreamed.current && !isStreaming && isOpen && !hasAutoClosed.current) {
      const t = setTimeout(() => {
        setIsOpen(false)
        hasAutoClosed.current = true
      }, AUTO_CLOSE_DELAY)
      return () => clearTimeout(t)
    }
  }, [isStreaming, isOpen, setIsOpen])

  const onOpenChange = useCallback((o: boolean) => setIsOpen(o), [setIsOpen])

  const label = isStreaming ? (
    <span className="aqa-shimmer">Thinking…</span>
  ) : duration === undefined ? (
    <span>Thought for a few seconds</span>
  ) : (
    <span>Thought for {duration} second{duration === 1 ? '' : 's'}</span>
  )

  return (
    <Collapsible open={!!isOpen} onOpenChange={onOpenChange} className="not-prose w-full">
      <CollapsibleTrigger className="flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground">
        <BrainIcon className="size-4" />
        {label}
        <ChevronDownIcon className={cn('size-4 transition-transform', isOpen ? 'rotate-180' : 'rotate-0')} />
      </CollapsibleTrigger>
      <CollapsibleContent className="mt-2 overflow-hidden text-muted-foreground data-[state=closed]:animate-out data-[state=open]:animate-in data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:slide-out-to-top-1 data-[state=open]:slide-in-from-top-1">
        <div className="border-l-2 border-border/60 pl-3">
          <Markdown text={text} className="text-[13px] text-muted-foreground" />
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
})

export default Reasoning
