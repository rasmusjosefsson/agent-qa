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
    <section className="flex h-full min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
      <div className="flex items-center gap-1.5 border-b border-border px-3 py-2 text-xs text-amber-400">
        <span>● live browser</span>
      </div>
      <div className="grid min-h-0 flex-1 place-items-center overflow-auto bg-black p-2">
        {src ? (
          <img
            ref={imgRef}
            src={src}
            alt="live replay browser"
            title="Click to enlarge"
            onClick={() => src && onLightbox(src, 'Live replay browser')}
            className="max-h-full max-w-full cursor-zoom-in object-contain"
          />
        ) : (
          <div className="text-xs text-muted-foreground">connecting to live browser…</div>
        )}
      </div>
    </section>
  )
}
