// web/src/features/editor/components/LiveCanvas.tsx
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { ChevronLeftIcon, ChevronRightIcon, CircleDotIcon, CrosshairIcon, MousePointer2Icon, RotateCwIcon } from 'lucide-react'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { cn } from '@/lib/utils'
import type { ClickMode, LiveInput, LiveStatus, PickedElement } from '../types'

const PREVENT_KEYS = new Set(['Enter', 'Backspace', 'Tab', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'])

interface Overlay {
  hidden: boolean
  left: number
  top: number
  width: number
  height: number
  label: string
  noninteractive: boolean
}
const HIDDEN_OVERLAY: Overlay = { hidden: true, left: 0, top: 0, width: 0, height: 0, label: '', noninteractive: false }

export function LiveCanvas({
  subscribeFrame,
  sendInput,
  pick,
  reload,
  connectLive,
  liveUrl,
  liveStatus,
  liveHint,
  clickMode,
  onClickModeChange,
  onCanvasPick,
}: {
  subscribeFrame: (cb: (b64: string) => void) => () => void
  sendInput: (evt: LiveInput) => void
  pick: (nx: number, ny: number) => Promise<PickedElement | null>
  reload: () => void
  connectLive: () => void
  liveUrl: string
  liveStatus: LiveStatus
  liveHint: string
  clickMode: ClickMode
  onClickModeChange: (m: ClickMode) => void
  onCanvasPick: (el: PickedElement) => void
}) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const stageRef = useRef<HTMLDivElement | null>(null)
  const [addr, setAddr] = useState('')
  const [overlay, setOverlay] = useState<Overlay>(HIDDEN_OVERLAY)
  const addrFocused = useRef(false)

  // Latest props for the stable native listeners.
  const bag = useRef({ sendInput, pick, onCanvasPick, clickMode, liveStatus })
  bag.current = { sendInput, pick, onCanvasPick, clickMode, liveStatus }

  // Keep the address bar synced to the live URL unless the user is editing it.
  useEffect(() => {
    if (!addrFocused.current && liveUrl) setAddr(liveUrl)
  }, [liveUrl])

  // Draw incoming frames straight to the canvas (imperative; never React state).
  useEffect(() => {
    const img = new Image()
    const unsub = subscribeFrame((b64) => {
      img.onload = () => {
        const cv = canvasRef.current
        if (!cv) return
        if (cv.width !== img.naturalWidth || cv.height !== img.naturalHeight) {
          cv.width = img.naturalWidth
          cv.height = img.naturalHeight
        }
        cv.getContext('2d')?.drawImage(img, 0, 0, cv.width, cv.height)
      }
      img.src = 'data:image/jpeg;base64,' + b64
    })
    return unsub
  }, [subscribeFrame])

  // Native input forwarding (non-passive so wheel/keys can preventDefault).
  useEffect(() => {
    const cv = canvasRef.current
    if (!cv) return
    cv.tabIndex = 0

    const norm = (ev: MouseEvent) => {
      const r = cv.getBoundingClientRect()
      if (!r.width || !r.height) return null
      return {
        nx: Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width)),
        ny: Math.min(1, Math.max(0, (ev.clientY - r.top) / r.height)),
      }
    }

    const hideHover = () => setOverlay((o) => (o.hidden ? o : HIDDEN_OVERLAY))

    const showHover = (el: PickedElement) => {
      const box = el && el.box
      const stage = stageRef.current
      if (!box || !stage) return hideHover()
      const r = cv.getBoundingClientRect()
      const s = stage.getBoundingClientRect()
      const offX = r.left - s.left + stage.scrollLeft
      const offY = r.top - s.top + stage.scrollTop
      const recordable = el.interactive !== false
      const tail = !recordable && bag.current.clickMode === 'record' ? '  (not recordable)' : ''
      setOverlay({
        hidden: false,
        left: offX + box.nx * r.width,
        top: offY + box.ny * r.height,
        width: Math.max(2, box.nw * r.width),
        height: Math.max(2, box.nh * r.height),
        label: (el.role || '?') + (el.name ? ' · ' + el.name : '') + tail,
        noninteractive: !recordable,
      })
    }

    const onMouseDown = async (ev: MouseEvent) => {
      const c = norm(ev)
      if (!c) return
      ev.preventDefault()
      const mode = bag.current.clickMode
      if (mode === 'pick') {
        const el = await bag.current.pick(c.nx, c.ny)
        if (el && (el.role || el.name)) bag.current.onCanvasPick(el)
        return
      }
      bag.current.sendInput({ type: 'click', ...c, ...(mode === 'record' ? { record: true } : {}) })
      cv.focus()
    }

    const onWheel = (ev: WheelEvent) => {
      if (bag.current.clickMode === 'pick') return
      const c = norm(ev)
      if (c) bag.current.sendInput({ type: 'scroll', ...c, dx: ev.deltaX, dy: ev.deltaY })
      ev.preventDefault()
    }

    const onKeyDown = (ev: KeyboardEvent) => {
      const mode = bag.current.clickMode
      if (mode === 'pick') return
      const record = mode === 'record'
      if (ev.key.length === 1) bag.current.sendInput({ type: 'key', text: ev.key, record })
      else bag.current.sendInput({ type: 'key', key: ev.key, record })
      if (PREVENT_KEYS.has(ev.key)) ev.preventDefault()
    }

    let hoverTimer: ReturnType<typeof setTimeout> | null = null
    let hoverLatest: { nx: number; ny: number } | null = null
    const onMouseMove = (ev: MouseEvent) => {
      const mode = bag.current.clickMode
      if (mode !== 'pick' && mode !== 'record') return hideHover()
      const c = norm(ev)
      if (!c) return
      hoverLatest = c
      if (hoverTimer) return
      hoverTimer = setTimeout(async () => {
        hoverTimer = null
        if (!hoverLatest) return
        const el = await bag.current.pick(hoverLatest.nx, hoverLatest.ny)
        if (el && bag.current.clickMode !== 'interact') showHover(el)
        else hideHover()
      }, 90)
    }

    cv.addEventListener('mousedown', onMouseDown)
    cv.addEventListener('wheel', onWheel, { passive: false })
    cv.addEventListener('keydown', onKeyDown)
    cv.addEventListener('mousemove', onMouseMove)
    cv.addEventListener('mouseleave', hideHover)
    return () => {
      cv.removeEventListener('mousedown', onMouseDown)
      cv.removeEventListener('wheel', onWheel)
      cv.removeEventListener('keydown', onKeyDown)
      cv.removeEventListener('mousemove', onMouseMove)
      cv.removeEventListener('mouseleave', hideHover)
      if (hoverTimer) clearTimeout(hoverTimer)
    }
  }, [])

  const submitNav = () => {
    const url = addr.trim()
    if (!url) return
    if (bag.current.liveStatus.tone !== 'ok') connectLive()
    sendInput({ type: 'navigate', url, record: clickMode === 'record' })
  }

  const tone = liveStatus.tone

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <div className="flex items-center gap-1.5 border-b border-border px-2 py-1.5">
        <NavBtn icon={<ChevronLeftIcon className="size-4" />} title="Back" onClick={() => sendInput({ type: 'back' })} />
        <NavBtn icon={<ChevronRightIcon className="size-4" />} title="Forward" onClick={() => sendInput({ type: 'forward' })} />
        <NavBtn icon={<RotateCwIcon className="size-3.5" />} title="Reload" onClick={reload} />
        <input
          value={addr}
          onChange={(e) => setAddr(e.target.value)}
          onFocus={() => (addrFocused.current = true)}
          onBlur={() => (addrFocused.current = false)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              submitNav()
              ;(e.target as HTMLInputElement).blur()
            }
          }}
          spellCheck={false}
          placeholder="https://example.com/"
          className="h-7 flex-1 rounded-md border border-border bg-background px-2 text-xs outline-none focus:border-ring"
        />
        <span
          className={cn(
            'shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide',
            tone === 'ok' && 'text-emerald-400',
            tone === 'busy' && 'text-amber-400',
            tone === 'err' && 'text-destructive',
            tone === 'idle' && 'text-muted-foreground'
          )}
        >
          {liveStatus.text}
        </span>
        <Select value={clickMode} onValueChange={(v) => onClickModeChange(v as ClickMode)}>
          <SelectTrigger size="sm" className="h-7 w-[140px] text-xs" title="What a click on the live page does">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="interact"><span className="flex items-center gap-1.5"><MousePointer2Icon className="size-3.5" /> Browse</span></SelectItem>
            <SelectItem value="record"><span className="flex items-center gap-1.5"><CircleDotIcon className="size-3.5 text-red-400" /> Record</span></SelectItem>
            <SelectItem value="pick"><span className="flex items-center gap-1.5"><CrosshairIcon className="size-3.5" /> Pick element</span></SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div ref={stageRef} className="relative flex min-h-0 flex-1 items-center justify-center overflow-auto bg-black">
        <canvas ref={canvasRef} width={1280} height={800} className="max-h-full max-w-full object-contain" />
        {!overlay.hidden && (
          <div
            className={cn(
              'pointer-events-none absolute z-10 border-2',
              overlay.noninteractive ? 'border-amber-400/80' : 'border-sky-400/80'
            )}
            style={{ left: overlay.left, top: overlay.top, width: overlay.width, height: overlay.height }}
          >
            <span
              className={cn(
                'absolute -top-5 left-0 whitespace-nowrap rounded px-1 py-0.5 text-[10px] text-black',
                overlay.noninteractive ? 'bg-amber-400' : 'bg-sky-400'
              )}
            >
              {overlay.label}
            </span>
          </div>
        )}
        {liveHint && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center p-6 text-center text-xs text-muted-foreground">
            {liveHint}
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

export default LiveCanvas
