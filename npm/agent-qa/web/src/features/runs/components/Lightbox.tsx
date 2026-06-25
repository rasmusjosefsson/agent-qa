// web/src/features/runs/components/Lightbox.tsx
import { useEffect } from 'react'

// shadcn Dialog-style image viewer: blurred backdrop + centered card,
// dismissed by backdrop click, the ✕ button, or Escape.
export function Lightbox({ url, caption, onClose }: { url: string; caption: string; onClose: () => void }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [onClose])

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-8 backdrop-blur-sm"
      onClick={onClose}
    >
      <div className="flex max-h-full max-w-5xl flex-col overflow-hidden rounded-xl border border-border bg-card" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center gap-3 border-b border-border px-3 py-2">
          <span className="truncate text-xs text-muted-foreground" title={caption}>
            {caption}
          </span>
          <div className="ml-auto flex items-center gap-3">
            <a href={url} target="_blank" rel="noopener" className="text-xs text-muted-foreground hover:text-foreground hover:underline">
              Open original ↗
            </a>
            <button type="button" title="Close (Esc)" aria-label="Close" onClick={onClose} className="text-muted-foreground hover:text-foreground">
              ✕
            </button>
          </div>
        </div>
        <img src={url} alt={caption} className="min-h-0 object-contain" />
      </div>
    </div>
  )
}
