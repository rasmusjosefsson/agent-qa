// web/src/features/runs/components/ReplayLive.tsx
import { useEffect, useRef, useState } from 'react'

// Read-only CDP screencast of an in-flight replay's browser. Owns its own
// EventSource keyed by sid (frames arrive as `message` events carrying
// { data: <base64 jpeg> } — same shape as the editor/chat live panes).
export function ReplayLive({ sid, onLightbox }: { sid: string; onLightbox: (url: string, caption: string) => void }) {
  const imgRef = useRef<HTMLImageElement | null>(null)
  const [src, setSrc] = useState('')

  useEffect(() => {
    const es = new EventSource(`/api/scenarios/${encodeURIComponent(sid)}/replay-stream`)
    es.onmessage = (ev) => {
      try {
        const f = JSON.parse(ev.data)
        if (f.data) setSrc('data:image/jpeg;base64,' + f.data)
      } catch {
        /* keep-alive comment */
      }
    }
    return () => es.close()
  }, [sid])

  return (
    <section className="flex h-full min-h-0 flex-col overflow-hidden bg-card">
      <div className="flex h-11 items-center gap-2 border-b border-border px-3 text-xs font-medium text-warning">
        <span className="aqa-ping size-1.5 rounded-full bg-current" />
        <span>Live browser</span>
      </div>
      <div className="aqa-dots grid min-h-0 flex-1 place-items-center overflow-auto bg-muted/40 p-3">
        {src ? (
          <img
            ref={imgRef}
            src={src}
            alt="live replay browser"
            title="Click to enlarge"
            onClick={() => src && onLightbox(src, 'Live replay browser')}
            className="max-h-full max-w-full cursor-zoom-in rounded-md object-contain shadow-lg ring-1 ring-border"
          />
        ) : (
          <div className="rounded-full border border-border bg-card px-3 py-1.5 text-xs text-muted-foreground shadow-sm">Connecting to live browser…</div>
        )}
      </div>
    </section>
  )
}
