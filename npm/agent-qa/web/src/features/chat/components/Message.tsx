// web/src/features/chat/components/Message.tsx
import type { ChatItem } from '@/lib/types'
import { Markdown } from './Markdown'
import { Reasoning } from './Reasoning'
import { ToolCard } from './ToolCard'

// Renders one ordered conversation item. The reducer interleaves these in event
// order (parity with classic chat.js). Styled after the AI Elements
// AI-Elements look: user messages sit in a soft right-aligned bubble; assistant
// text renders as clean full-width markdown (no bubble); thinking renders inline
// as a collapsible Reasoning block.
export function Message({ item, thinkingStreaming }: { item: ChatItem; thinkingStreaming?: boolean }) {
  switch (item.kind) {
    case 'user':
      return (
        <div className="flex justify-end">
          <div className="max-w-[85%] whitespace-pre-wrap rounded-2xl bg-secondary px-4 py-2.5 text-sm text-secondary-foreground">
            {item.text}
          </div>
        </div>
      )

    case 'assistant':
      // Empty assistant bubble = the model is about to speak; the trailing
      // “Working…” row in ChatPage covers that state, so render nothing here.
      if (!item.text) return null
      return (
        <div className="w-full">
          <Markdown text={item.text} />
        </div>
      )

    case 'thinking':
      return <Reasoning text={item.text} isStreaming={!!thinkingStreaming} />

    case 'tool':
      return <ToolCard item={item} />

    case 'error':
      return (
        <div className="whitespace-pre-wrap rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {item.text}
        </div>
      )

    default:
      return null
  }
}

export default Message
