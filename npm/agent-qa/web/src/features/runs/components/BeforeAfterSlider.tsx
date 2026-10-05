// web/src/features/runs/components/BeforeAfterSlider.tsx
// Draggable before/after image compare — overlays `after` clipped to the
// slider position on top of `before`. Pointer drag anywhere on the image,
// or focus the divider and use arrow keys. Both images must be same-size
// captures of the same step for the wipe to line up.
import { useCallback, useRef, useState } from 'react'
import { cn } from '@/lib/utils'

export function BeforeAfterSlider({
  before,
  after,
  beforeLabel = 'baseline',
  afterLabel = 'actual',
  className,
}: {
  before: string
  after: string
  beforeLabel?: string
  afterLabel?: string
  className?: string
}) {
  const ref = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState(0.5)

  const move = useCallback((clientX: number) => {
    const el = ref.current
    if (!el) return
    const rect = el.getBoundingClientRect()
    const x = (clientX - rect.left) / rect.width
    setPos(Math.min(1, Math.max(0, x)))
  }, [])

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId)
    move(e.clientX)
  }
  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.buttons === 0) return
    move(e.clientX)
  }
  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      e.preventDefault()
      setPos((p) => Math.min(1, Math.max(0, p + (e.key === 'ArrowRight' ? 0.05 : -0.05))))
    }
  }

  const pct = `${(pos * 100).toFixed(2)}%`
  return (
    <div
      ref={ref}
      role="slider"
      aria-label="Before/after comparison"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(pos * 100)}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onKeyDown={onKeyDown}
      className={cn(
        'relative aspect-video w-full cursor-ew-resize touch-none select-none overflow-hidden rounded-lg border border-border focus:outline-none focus:ring-1 focus:ring-primary/40',
        className
      )}
    >
      <img
        src={before}
        alt={beforeLabel}
        draggable={false}
        className="absolute inset-0 size-full object-cover object-top"
      />
      <div
        className="absolute inset-0 overflow-hidden"
        style={{ clipPath: `inset(0 0 0 ${pct})` }}
      >
        <img
          src={after}
          alt={afterLabel}
          draggable={false}
          className="absolute inset-0 size-full object-cover object-top"
        />
      </div>
      <div
        className="absolute inset-y-0 z-10 w-0.5 bg-white shadow-[0_0_0_1px_rgba(0,0,0,0.35)]"
        style={{ left: pct }}
      >
        <div className="absolute left-1/2 top-1/2 grid size-6 -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full border border-black/25 bg-white text-[9px] font-semibold text-black/70 shadow">
          ⇔
        </div>
      </div>
      <span className="absolute left-1.5 top-1.5 z-10 rounded bg-black/55 px-1.5 py-0.5 text-[10px] font-medium text-white">
        {beforeLabel}
      </span>
      <span className="absolute right-1.5 top-1.5 z-10 rounded bg-black/55 px-1.5 py-0.5 text-[10px] font-medium text-white">
        {afterLabel}
      </span>
    </div>
  )
}
