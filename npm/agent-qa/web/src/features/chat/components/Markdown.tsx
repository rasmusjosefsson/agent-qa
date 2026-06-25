// web/src/features/chat/components/Markdown.tsx
import { Streamdown } from 'streamdown'
import { cn } from '@/lib/utils'

// Progressive markdown renderer. streamdown is built for streaming: it parses
// incomplete markdown safely (so growing assistant deltas render cleanly) and
// ships its own Tailwind styling (registered via the `@source` line in
// index.css). We only add light spacing so it sits flush in the bubble.
export function Markdown({ text, className }: { text: string; className?: string }) {
  return (
    <Streamdown
      className={cn(
        'max-w-none break-words text-sm leading-relaxed',
        '[&_p]:my-2 [&_ul]:my-2 [&_ol]:my-2 [&_pre]:my-2 first:[&>*]:mt-0 last:[&>*]:mb-0',
        className
      )}
    >
      {text}
    </Streamdown>
  )
}

export default Markdown

