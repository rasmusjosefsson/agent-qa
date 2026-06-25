// web/src/features/chat/BrowserPane.tsx
import { useEffect, useRef, useState } from 'react'
import { cn } from '@/lib/utils'

type Status = { text: string; tone: 'idle' | 'busy' | 'ok' | 'err' }

export interface BrowserPaneProps {
  available: boolean
  chatId: string | null
  navigate: (payload: unknown) => Promise<Response>
}

// Read-only (plus URL-bar steer) view of the agent-browser session the chat
// agent is driving, streamed as base64 JPEG frames over SSE. The session is not
// fixed: by default we "follow" this chat's own session (mirror its active
// recorder session while it records, else its bound browser tab — polled from
// /api/chat/c/<id>/active-session). The user can also pin a specific session
// from the picker, which overrides follow until they switch back to "auto".
export function BrowserPane({ available, chatId, navigate }: BrowserPaneProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const urlRef = useRef<HTMLInputElement | null>(null)
  const imgRef = useRef<HTMLImageElement | null>(null)
  const [url, setUrl] = useState('')
  // The agent's auto-followed session, and an optional manual pin ('' = follow).
  const [autoSession, setAutoSession] = useState('default')
  const [autoRecording, setAutoRecording] = useState(false)
  const [manualSession, setManualSession] = useState('')
  const [sessions, setSessions] = useState<string[]>([])
  const effective = manualSession || autoSession
  const sessionRef = useRef(effective)
  sessionRef.current = effective
  const [status, setStatus] = useState<Status>(
    available ? { text: 'idle', tone: 'idle' } : { text: 'off', tone: 'idle' }
  )
  // The canvas is blank until a frame paints, so the hint starts visible and is
  // only hidden once a real frame renders (img.onload). Starting it hidden when
  // available made it blink on every chat-switch remount: hidden → bridge-error
  // → visible. Starting visible keeps it steady across switches of idle panes.
  const [showHint, setShowHint] = useState(true)

  // Follow this chat's active session + keep the picker's option list fresh.
  useEffect(() => {
    if (!available || !chatId) return
    let alive = true
    const tick = async () => {
      try {
        const [a, s] = await Promise.all([
          fetch(`/api/chat/c/${encodeURIComponent(chatId)}/active-session`).then((r) =>
            r.ok ? r.json() : null
          ),
          fetch('/api/chat/sessions').then((r) => (r.ok ? r.json() : null)),
        ])
        if (!alive) return
        if (a && a.session) {
          setAutoSession(a.session)
          setAutoRecording(!!a.recording)
        }
        if (s && Array.isArray(s.sessions)) {
          setSessions(s.sessions)
          // If the pinned session vanished (daemon closed), fall back to follow.
          setManualSession((m) => (m && !s.sessions.includes(m) ? '' : m))
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
      setShowHint(true)
      return
    }
    let connected = false
    let settled = false
    // Don't flash "connecting…" for the common idle/fast-connect cases: the
    // resting state is "idle", and a no-page session resolves to idle (or a
    // live one to a frame) within a few ms. Only surface "connecting…" if
    // neither verdict arrives within a beat — i.e. it's genuinely connecting.
    const connectingTimer = window.setTimeout(() => {
      if (!connected && !settled) setStatus({ text: 'connecting…', tone: 'busy' })
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
        img.onload = () => {
          const cv = canvasRef.current
          if (!cv) return
          if (cv.width !== img.naturalWidth || cv.height !== img.naturalHeight) {
            cv.width = img.naturalWidth
            cv.height = img.naturalHeight
          }
          cv.getContext('2d')?.drawImage(img, 0, 0, cv.width, cv.height)
          setShowHint(false)
        }
        img.src = 'data:image/jpeg;base64,' + f.data
      } catch {
        /* keep-alive comment */
      }
    }
    es.addEventListener('url', (e) => {
      try {
        const { url: u } = JSON.parse((e as MessageEvent).data)
        if (document.activeElement !== urlRef.current) setUrl(u || '')
      } catch {
        /* ignore */
      }
    })
    es.addEventListener('bridge-error', () => {
      settled = true
      window.clearTimeout(connectingTimer)
      // Only surface the "idle" hint if we never rendered a frame. Once a frame
      // has shown, the canvas keeps it — a transient bridge-error on the SSE's
      // periodic reconnect must not re-show the hint (that's the blink).
      if (!connected) {
        setStatus({ text: 'idle', tone: 'idle' })
        setShowHint(true)
      }
    })
    es.onerror = () => {
      // Don't flap back to "waiting…" once we've connected or settled to idle.
      if (!connected && !settled) setStatus({ text: 'waiting…', tone: 'idle' })
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

  // Build the picker options: "auto" + every live session (effective shown
  // even if the list hasn't caught up yet).
  const options = Array.from(new Set([...sessions, effective]))

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden rounded-xl border border-border bg-card">
      <div className="flex items-center gap-1.5 border-b border-border px-2 py-1.5">
        <NavBtn label="‹" title="Back" onClick={() => browserAction({ action: 'back' })} />
        <NavBtn label="›" title="Forward" onClick={() => browserAction({ action: 'forward' })} />
        <NavBtn label="⟳" title="Reload" onClick={() => browserAction({ action: 'reload' })} />
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
        <select
          value={manualSession}
          onChange={(e) => setManualSession(e.target.value)}
          title="Which agent-browser session to mirror"
          className="w-[8.5rem] shrink-0 truncate rounded-md border border-border bg-background px-1.5 py-1 text-xs text-muted-foreground outline-none focus:border-ring"
        >
          <option value="">auto</option>
          {options.map((s) => (
            <option key={s} value={s}>
              {s}
            </option>
          ))}
        </select>
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
          {!manualSession && autoRecording ? `rec · ${status.text}` : status.text}
        </span>
      </div>
      <div className="relative flex min-h-0 flex-1 items-center justify-center bg-black">
        <canvas ref={canvasRef} width={1280} height={800} className="max-h-full max-w-full object-contain" />
        {showHint && (
          <div className="absolute inset-0 flex items-center justify-center p-6 text-center text-xs text-muted-foreground">
            {available
              ? "The agent's browser appears here when it opens a page. It follows the agent's active session automatically — or pick a specific session from the dropdown."
              : 'Live browser unavailable — launch via the agent-qa CLI to watch the agent drive a browser here.'}
          </div>
        )}
      </div>
    </div>
  )
}

function NavBtn({ label, title, onClick }: { label: string; title: string; onClick: () => void }) {
  return (
    <button
      type="button"
      title={title}
      onClick={onClick}
      className="grid h-7 w-7 shrink-0 place-items-center rounded-md border border-border bg-background text-sm text-muted-foreground hover:bg-muted hover:text-foreground"
    >
      {label}
    </button>
  )
}

export default BrowserPane
