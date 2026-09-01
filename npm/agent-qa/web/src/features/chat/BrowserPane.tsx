// web/src/features/chat/BrowserPane.tsx
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { ChevronLeftIcon, ChevronRightIcon, Loader2Icon, RotateCwIcon } from 'lucide-react'
import { cn } from '@/lib/utils'

type Status = { text: string; tone: 'idle' | 'busy' | 'ok' | 'err' }
// Center-pane phase — drives the overlay (spinner vs. short hint vs. nothing):
//   live       a frame is painted (cached or streamed) — no overlay
//   blank      connecting but too early to show anything (<300ms) — no overlay
//   connecting still connecting after a beat — show a spinner
//   idle       settled with no page open — show a one-line hint
//   off        live browser unavailable
type Phase = 'off' | 'blank' | 'connecting' | 'idle' | 'live'

// Last screencast frame per session (data URL). The pane is remounted on every
// chat-tab switch (ChatConversation is keyed by chat id), which drops the
// canvas. Caching the last frame lets a re-mounted pane repaint instantly — and
// start in the "live" state — instead of flashing the idle hint while the
// screencast SSE reconnects. Bounded by the number of sessions opened this page.
const lastFrame = new Map<string, string>()

function drawToCanvas(canvas: HTMLCanvasElement | null, src: string) {
  if (!canvas) return
  const img = new Image()
  img.onload = () => {
    if (canvas.width !== img.naturalWidth || canvas.height !== img.naturalHeight) {
      canvas.width = img.naturalWidth
      canvas.height = img.naturalHeight
    }
    canvas.getContext('2d')?.drawImage(img, 0, 0, canvas.width, canvas.height)
  }
  img.src = src
}

export interface BrowserPaneProps {
  available: boolean
  chatId: string | null
  navigate: (payload: unknown) => Promise<Response>
  // This chat's own session, so we follow it from the first frame instead of
  // briefly connecting to the shared 'default' session (which shows another
  // chat's page) until the active-session poll catches up.
  initialSession?: string
}

// Read-only (plus URL-bar steer) view of the agent-browser session the chat
// agent is driving, streamed as base64 JPEG frames over SSE. The pane always
// follows this chat's own session — its active recorder session while it
// records, else its bound browser tab — polled from
// /api/chat/c/<id>/active-session.
export function BrowserPane({ available, chatId, navigate, initialSession }: BrowserPaneProps) {
  const initialSessionKey = initialSession || 'default'
  // True when we already have a cached frame for the session this pane follows
  // first — i.e. we've shown it before and a remount can repaint immediately.
  const seeded = available && lastFrame.has(initialSessionKey)
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const urlRef = useRef<HTMLInputElement | null>(null)
  const imgRef = useRef<HTMLImageElement | null>(null)
  const [url, setUrl] = useState('')
  // The session this pane follows: this chat's active session (its browser, or
  // its recorder while recording). Updated by the active-session poll below.
  const [autoSession, setAutoSession] = useState(initialSessionKey)
  const [autoRecording, setAutoRecording] = useState(false)
  const effective = autoSession
  const sessionRef = useRef(effective)
  sessionRef.current = effective
  const [status, setStatus] = useState<Status>(() =>
    !available
      ? { text: 'off', tone: 'idle' }
      : seeded
        ? { text: 'live', tone: 'ok' }
        : { text: 'idle', tone: 'idle' }
  )
  // Center-overlay phase. A cached frame starts us straight at "live" (no flash
  // on tab switch); otherwise "blank" until we learn whether the session is
  // connecting, idle (no page), or live.
  const [phase, setPhase] = useState<Phase>(() => (!available ? 'off' : seeded ? 'live' : 'blank'))

  // Follow this chat's active session (its browser, or its recorder while recording).
  useEffect(() => {
    if (!available || !chatId) return
    let alive = true
    const tick = async () => {
      try {
        const a = await fetch(`/api/chat/c/${encodeURIComponent(chatId)}/active-session`).then((r) =>
          r.ok ? r.json() : null
        )
        if (!alive) return
        if (a && a.session) {
          setAutoSession(a.session)
          setAutoRecording(!!a.recording)
        }
      } catch {
        /* keep current */
      }
    }
    void tick()
    const id = setInterval(tick, 1500)
    return () => {
      alive = false
      clearInterval(id)
    }
  }, [available, chatId])

  // (Re)connect the screencast whenever the effective session changes.
  useEffect(() => {
    if (!available) {
      setStatus({ text: 'off', tone: 'idle' })
      setPhase('off')
      return
    }
    let connected = false
    let settled = false
    // Repaint the last frame for this session right away so a remount (tab
    // switch) doesn't flash the idle hint while the screencast reconnects.
    // `hasFrame` then suppresses the idle/connecting/waiting fallbacks below —
    // there's already something on the canvas.
    let hasFrame = lastFrame.has(effective)
    if (hasFrame) {
      drawToCanvas(canvasRef.current, lastFrame.get(effective)!)
      setPhase('live')
      setStatus({ text: 'live', tone: 'ok' })
    } else {
      setPhase('blank')
    }
    // Don't flash "connecting…" for the common idle/fast-connect cases: the
    // resting state is "idle", and a no-page session resolves to idle (or a
    // live one to a frame) within a few ms. Only surface "connecting…" if
    // neither verdict arrives within a beat — i.e. it's genuinely connecting.
    const connectingTimer = window.setTimeout(() => {
      if (!connected && !settled && !hasFrame) {
        setStatus({ text: 'connecting…', tone: 'busy' })
        setPhase('connecting')
      }
    }, 300)
    const img = (imgRef.current = new Image())
    const es = new EventSource('/api/chat/browser-stream?session=' + encodeURIComponent(effective))

    es.onmessage = (e) => {
      try {
        const f = JSON.parse((e as MessageEvent).data)
        if (!f.data) return
        connected = true
        window.clearTimeout(connectingTimer)
        setStatus({ text: 'live', tone: 'ok' })
        const src = 'data:image/jpeg;base64,' + f.data
        img.onload = () => {
          const cv = canvasRef.current
          if (!cv) return
          if (cv.width !== img.naturalWidth || cv.height !== img.naturalHeight) {
            cv.width = img.naturalWidth
            cv.height = img.naturalHeight
          }
          cv.getContext('2d')?.drawImage(img, 0, 0, cv.width, cv.height)
          setPhase('live')
          hasFrame = true
          lastFrame.set(effective, src) // cache for instant repaint on remount
        }
        img.src = src
      } catch {
        /* keep-alive comment */
      }
    }
    es.addEventListener('url', (e) => {
      try {
        const { url: u } = JSON.parse((e as MessageEvent).data)
        // A blank page (about:blank) is "no URL" — keep the placeholder rather
        // than flashing the literal "about:blank" into the bar.
        const clean = u && u !== 'about:blank' ? u : ''
        if (document.activeElement !== urlRef.current) setUrl(clean)
      } catch {
        /* ignore */
      }
    })
    es.addEventListener('bridge-error', () => {
      settled = true
      window.clearTimeout(connectingTimer)
      // Only surface the "idle" hint if nothing is painted — no live frame this
      // connection AND no cached frame from a previous view. Once a frame has
      // shown, the canvas keeps it; a transient bridge-error on the SSE's
      // periodic reconnect must not re-show the hint (that's the blink).
      if (!connected && !hasFrame) {
        setStatus({ text: 'idle', tone: 'idle' })
        setPhase('idle')
      }
    })
    es.onerror = () => {
      // Still trying — keep the spinner (not the idle hint) unless we've already
      // connected, settled, or have a cached frame painted.
      if (!connected && !settled && !hasFrame) {
        setStatus({ text: 'waiting…', tone: 'idle' })
        setPhase('connecting')
      }
    }

    return () => {
      window.clearTimeout(connectingTimer)
      try {
        es.close()
      } catch {
        /* ignore */
      }
    }
  }, [available, effective])

  async function browserAction(payload: Record<string, unknown>) {
    try {
      const res = await navigate({ ...payload, session: sessionRef.current })
      if (!res.ok) {
        setStatus({ text: res.status === 409 ? 'no page yet' : 'nav failed', tone: 'err' })
      }
    } catch {
      setStatus({ text: 'nav failed', tone: 'err' })
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-background">
      <div className="flex items-center gap-1.5 border-b border-border px-2 py-1.5">
        <NavBtn icon={<ChevronLeftIcon className="size-4" />} title="Back" onClick={() => browserAction({ action: 'back' })} />
        <NavBtn icon={<ChevronRightIcon className="size-4" />} title="Forward" onClick={() => browserAction({ action: 'forward' })} />
        <NavBtn icon={<RotateCwIcon className="size-3.5" />} title="Reload" onClick={() => browserAction({ action: 'reload' })} />
        <form
          className="flex-1"
          onSubmit={(e) => {
            e.preventDefault()
            const u = url.trim()
            if (u) browserAction({ action: 'navigate', url: u })
            urlRef.current?.blur()
          }}
        >
          <input
            ref={urlRef}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            spellCheck={false}
            placeholder="Enter a URL to drive the agent's browser…"
            className="w-full rounded-md border border-border bg-background px-2 py-1 text-xs outline-none focus:border-ring"
          />
        </form>
        <span
          className={cn(
            // Fixed min-width + right-align so swapping status text
            // (idle ↔ connecting… ↔ live) never reflows the header / URL bar.
            'shrink-0 rounded px-1.5 py-0.5 text-right text-[10px] font-medium uppercase tracking-wide tabular-nums min-w-[6rem]',
            status.tone === 'ok' && 'text-emerald-400',
            status.tone === 'busy' && 'text-amber-400',
            status.tone === 'err' && 'text-destructive',
            status.tone === 'idle' && 'text-muted-foreground'
          )}
        >
          {autoRecording ? `rec · ${status.text}` : status.text}
        </span>
      </div>
      <div className="relative flex min-h-0 flex-1 items-center justify-center bg-black">
        <canvas ref={canvasRef} width={1280} height={800} className="max-h-full max-w-full object-contain" />
        {phase === 'connecting' && (
          <div className="absolute inset-0 flex items-center justify-center gap-2 text-xs text-muted-foreground">
            <Loader2Icon className="size-4 animate-spin" />
            Connecting to the browser…
          </div>
        )}
        {phase === 'idle' && (
          <div className="absolute inset-0 flex items-center justify-center p-6 text-center text-xs text-muted-foreground">
            No page open yet — type a URL above, or ask the agent to open one.
          </div>
        )}
        {phase === 'off' && (
          <div className="absolute inset-0 flex items-center justify-center p-6 text-center text-xs text-muted-foreground">
            Live browser unavailable — launch via the agent-qa CLI.
          </div>
        )}
      </div>
    </div>
  )
}

function NavBtn({ icon, title, onClick }: { icon: ReactNode; title: string; onClick: () => void }) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onClick={onClick}
      className="grid h-7 w-7 shrink-0 place-items-center rounded-md border border-border bg-background text-muted-foreground hover:bg-muted hover:text-foreground"
    >
      {icon}
    </button>
  )
}

export default BrowserPane
