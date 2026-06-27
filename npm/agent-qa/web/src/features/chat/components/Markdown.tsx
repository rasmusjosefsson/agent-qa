// web/src/features/chat/components/Markdown.tsx
import { Streamdown } from 'streamdown'
import { cn } from '@/lib/utils'

// Progressive markdown renderer. streamdown is built for streaming: it parses
// incomplete markdown safely (so growing assistant deltas render cleanly) and
// ships its own Tailwind styling (registered via the `@source` line in
// index.css). We override it with a shadcn-docs-style typography scale via
// descendant selectors (higher specificity than streamdown's element classes,
// so these win): clear heading hierarchy, h2 underline rule, readable leading,
// real bordered tables, spaced lists, inline-code chips, and styled links.
const TYPOGRAPHY = cn(
  'max-w-none break-words text-sm leading-7 text-foreground',
  // headings
  '[&_h1]:mt-6 [&_h1]:mb-3 [&_h1]:text-2xl [&_h1]:font-semibold [&_h1]:tracking-tight',
  '[&_h2]:mt-6 [&_h2]:mb-3 [&_h2]:border-b [&_h2]:border-border [&_h2]:pb-1.5 [&_h2]:text-xl [&_h2]:font-semibold [&_h2]:tracking-tight',
  '[&_h3]:mt-5 [&_h3]:mb-2 [&_h3]:text-base [&_h3]:font-semibold [&_h3]:tracking-tight',
  '[&_h4]:mt-4 [&_h4]:mb-2 [&_h4]:text-sm [&_h4]:font-semibold',
  // body + inline
  '[&_p]:my-3 [&_p]:leading-7',
  '[&_strong]:font-semibold [&_strong]:text-foreground',
  '[&_a]:font-medium [&_a]:underline [&_a]:underline-offset-4 [&_a]:decoration-muted-foreground hover:[&_a]:decoration-foreground',
  '[&_:not(pre)>code]:rounded [&_:not(pre)>code]:bg-muted [&_:not(pre)>code]:px-1.5 [&_:not(pre)>code]:py-0.5 [&_:not(pre)>code]:font-mono [&_:not(pre)>code]:text-[0.85em]',
  // lists
  '[&_ul]:my-3 [&_ul]:ml-5 [&_ul]:list-disc [&_ul]:space-y-1.5',
  '[&_ol]:my-3 [&_ol]:ml-5 [&_ol]:list-decimal [&_ol]:space-y-1.5',
  '[&_li]:leading-7 [&_li]:marker:text-muted-foreground',
  '[&_li>ul]:my-1.5 [&_li>ol]:my-1.5',
  // quotes / rules
  '[&_blockquote]:my-3 [&_blockquote]:border-l-2 [&_blockquote]:border-border [&_blockquote]:pl-4 [&_blockquote]:text-muted-foreground [&_blockquote]:italic',
  '[&_hr]:my-5 [&_hr]:border-border',
  // tables
  '[&_table]:my-4 [&_table]:w-full [&_table]:border-collapse [&_table]:overflow-hidden [&_table]:rounded-md [&_table]:text-sm',
  '[&_th]:border [&_th]:border-border [&_th]:bg-muted/50 [&_th]:px-3 [&_th]:py-2 [&_th]:text-left [&_th]:font-medium',
  '[&_td]:border [&_td]:border-border [&_td]:px-3 [&_td]:py-2 [&_td]:align-top',
  // code blocks: keep streamdown's highlighting, just tidy spacing
  '[&_pre]:my-3 [&_pre]:rounded-lg [&_pre]:text-xs [&_pre]:leading-relaxed',
  // no stray top/bottom margin inside the bubble
  'first:[&>*]:mt-0 last:[&>*]:mb-0'
)

export function Markdown({ text, className }: { text: string; className?: string }) {
  return <Streamdown className={cn(TYPOGRAPHY, className)}>{text}</Streamdown>
}

export default Markdown
