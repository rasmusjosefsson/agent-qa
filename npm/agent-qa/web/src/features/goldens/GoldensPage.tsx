import { useCallback, useEffect, useRef, useState } from 'react'
import {
  Columns2Icon,
  DatabaseIcon,
  FileTextIcon,
  ImageIcon,
  ImagesIcon,
  RefreshCwIcon,
} from 'lucide-react'
import { PageHeader } from '@/components/page-header'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { cn } from '@/lib/utils'
import {
  fetchGoldens,
  goldenAssetUrl,
  goldenFileUrl,
  setGoldenEnabled,
  type GoldenFile,
  type GoldensResponse,
} from '@/lib/goldens-api'

function fmtSize(bytes?: number): string {
  if (!bytes) return ''
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

type CompareMode = 'split' | 'diff'

// Before/after split: baseline on the left, latest-run actual on the right,
// a draggable divider between them (keyboard: arrows on the handle).
function SplitCompare({ sid, name }: { sid: string; name: string }) {
  const [pos, setPos] = useState(50)
  const box = useRef<HTMLDivElement>(null)
  const drag = useRef(false)

  const move = useCallback((clientX: number) => {
    const r = box.current?.getBoundingClientRect()
    if (!r || r.width === 0) return
    setPos(Math.min(100, Math.max(0, ((clientX - r.left) / r.width) * 100)))
  }, [])

  useEffect(() => {
    const onMove = (e: PointerEvent) => drag.current && move(e.clientX)
    const onUp = () => {
      drag.current = false
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
    return () => {
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
    }
  }, [move])

  return (
    <div
      ref={box}
      className="relative aspect-video w-full cursor-ew-resize touch-none select-none overflow-hidden rounded-lg border border-border bg-muted/30"
      onPointerDown={(e) => {
        drag.current = true
        move(e.clientX)
      }}
    >
      <img
        src={goldenFileUrl(sid, name)}
        alt="baseline"
        draggable={false}
        className="absolute inset-0 size-full object-cover object-top"
      />
      <img
        src={goldenAssetUrl(sid, name, 'actual')}
        alt="current run"
        draggable={false}
        className="absolute inset-0 size-full object-cover object-top"
        style={{ clipPath: `inset(0 0 0 ${pos}%)` }}
        onError={(e) => {
          e.currentTarget.style.display = 'none'
        }}
      />
      <div
        className="absolute inset-y-0 w-0.5 bg-white shadow-[0_0_0_1px_rgba(0,0,0,0.4)]"
        style={{ left: `${pos}%` }}
      />
      <div
        role="slider"
        aria-label="Compare baseline and current"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(pos)}
        tabIndex={0}
        className="absolute top-1/2 grid size-7 -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full bg-white text-foreground shadow-md outline-none ring-primary/50 focus-visible:ring-2"
        style={{ left: `${pos}%` }}
        onKeyDown={(e) => {
          if (e.key === 'ArrowLeft') setPos((p) => Math.max(0, p - 4))
          if (e.key === 'ArrowRight') setPos((p) => Math.min(100, p + 4))
        }}
      >
        <Columns2Icon className="size-3.5" />
      </div>
      <span className="absolute left-2 top-2 rounded bg-black/60 px-1.5 py-0.5 text-[10px] font-medium text-white">
        before
      </span>
      <span className="absolute right-2 top-2 rounded bg-black/60 px-1.5 py-0.5 text-[10px] font-medium text-white">
        after
      </span>
    </div>
  )
}

function DiffView({ sid, name, hasDiff }: { sid: string; name: string; hasDiff?: boolean }) {
  const [missing, setMissing] = useState(false)
  if (!hasDiff || missing) {
    return (
      <div className="grid aspect-video w-full place-items-center rounded-lg border border-border bg-muted/30 text-xs text-muted-foreground">
        No diff map — the latest run didn't flag this golden (or hasn't run yet).
      </div>
    )
  }
  return (
    <img
      src={goldenAssetUrl(sid, name, 'diff')}
      alt="pixel diff — changed pixels in red"
      onError={() => setMissing(true)}
      className="w-full rounded-lg border border-border bg-white object-contain"
    />
  )
}

function GoldenCard({
  sid,
  file,
  onToggle,
  onCompare,
}: {
  sid: string
  file: GoldenFile
  onToggle: () => void
  onCompare: () => void
}) {
  const [busy, setBusy] = useState(false)
  const isPng = file.name.endsWith('.png')
  const enabled = file.enabled !== false

  const toggle = async () => {
    setBusy(true)
    try {
      await setGoldenEnabled(sid, file.name, !enabled)
      onToggle()
    } finally {
      setBusy(false)
    }
  }

  return (
    <figure
      className={cn(
        'overflow-hidden rounded-lg border border-border/70 bg-muted/30 transition-opacity',
        !enabled && 'opacity-50'
      )}
    >
      {isPng ? (
        <button type="button" className="block w-full cursor-zoom-in" onClick={onCompare}>
          <img
            src={goldenFileUrl(sid, file.name)}
            alt={`${sid} ${file.name}`}
            loading="lazy"
            className="aspect-video w-full object-cover object-top"
          />
        </button>
      ) : (
        <div className="grid aspect-video w-full place-items-center text-muted-foreground/60">
          {file.name.endsWith('.txt') ? (
            <FileTextIcon className="size-8" />
          ) : (
            <ImageIcon className="size-8" />
          )}
        </div>
      )}
      <figcaption className="flex items-center justify-between gap-2 px-2.5 py-1.5">
        <span className="truncate font-mono text-[11px] text-muted-foreground">
          {file.name}
          {file.hasDiff && (
            <span className="ml-1.5 rounded bg-destructive/15 px-1 py-0.5 text-[9px] font-semibold text-destructive">
              diff
            </span>
          )}
        </span>
        <span className="flex shrink-0 items-center gap-1.5">
          <span className="text-[10px] text-muted-foreground/60">{fmtSize(file.size)}</span>
          <button
            type="button"
            role="switch"
            aria-checked={enabled}
            aria-label={`${enabled ? 'Disable' : 'Enable'} ${file.name}`}
            disabled={busy}
            onClick={toggle}
            className={cn(
              'relative h-4 w-7 rounded-full transition-colors',
              enabled ? 'bg-primary' : 'bg-muted-foreground/30',
              busy && 'opacity-60'
            )}
          >
            <span
              className={cn(
                'absolute top-0.5 size-3 rounded-full bg-white shadow transition-transform',
                enabled ? 'translate-x-3.5' : 'translate-x-0.5'
              )}
            />
          </button>
        </span>
      </figcaption>
    </figure>
  )
}

// Baselines (goldens) across every scenario — shot PNGs as thumbnails,
// domshot snapshots as text entries. Click a shot to compare against the
// latest run (before/after slider + red diff map); the switch per card
// disables the check claim behind that golden.
export function GoldensPage() {
  const [data, setData] = useState<GoldensResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [compare, setCompare] = useState<{ sid: string; file: GoldenFile } | null>(null)
  const [mode, setMode] = useState<CompareMode>('split')

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      setData(await fetchGoldens())
    } catch (e) {
      setError(String((e as Error)?.message || e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const scenarios = data?.scenarios ?? []
  const fileCount = scenarios.reduce((n, s) => n + s.files.length, 0)

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader
        icon={ImagesIcon}
        title="Goldens"
        description={
          data
            ? `${scenarios.length} scenario${scenarios.length === 1 ? '' : 's'} · ${fileCount} baseline${fileCount === 1 ? '' : 's'} · store: ${data.store}`
            : 'Baseline screenshots and snapshots your shot/domshot claims diff against.'
        }
        actions={
          <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
            <RefreshCwIcon className={cn('size-3.5', loading && 'animate-spin')} />
            Refresh
          </Button>
        }
      />

      <div className="min-h-0 flex-1 space-y-5 overflow-auto px-5 py-5">
        {error && (
          <div className="aqa-table-card aqa-elevated px-4 py-3 text-sm text-destructive">{error}</div>
        )}

        {!error && !loading && scenarios.length === 0 && (
          <div className="aqa-table-card aqa-elevated px-6 py-10 text-center">
            <ImagesIcon className="mx-auto mb-3 size-8 text-muted-foreground/50" />
            <p className="text-sm font-medium text-foreground">No goldens yet</p>
            <p className="mx-auto mt-1 max-w-md text-xs text-muted-foreground">
              Add a <code>shot</code> or <code>domshot</code> claim to a scenario, run it once, then
              mint with <code>agent-qa shot-accept &lt;sid&gt;</code>. Goldens land here — and in the
              store picked under Settings → Golden storage.
            </p>
          </div>
        )}

        {scenarios.map((s) => (
          <section key={s.sid} className="aqa-table-card aqa-elevated">
            <header className="flex items-center gap-2.5 border-b border-border/60 px-4 py-2.5">
              <span className="truncate text-sm font-semibold tracking-tight text-foreground">
                {s.sid}
              </span>
              <span className="text-xs text-muted-foreground">
                {s.files.length} baseline{s.files.length === 1 ? '' : 's'}
              </span>
            </header>
            <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3 p-4">
              {s.files.map((f) => (
                <GoldenCard
                  key={f.name}
                  sid={s.sid}
                  file={f}
                  onToggle={() => void load()}
                  onCompare={() => {
                    setMode(f.hasDiff ? 'diff' : 'split')
                    setCompare({ sid: s.sid, file: f })
                  }}
                />
              ))}
            </div>
          </section>
        ))}

        {data && data.store !== 'local' && (
          <p className="flex items-center gap-1.5 text-xs text-muted-foreground/70">
            <DatabaseIcon className="size-3.5" />
            Showing the local copy — pull/push from Settings → Golden storage or
            `agent-qa baselines`.
          </p>
        )}
      </div>

      <Dialog open={!!compare} onOpenChange={(o) => !o && setCompare(null)}>
        <DialogContent className="max-w-3xl">
          <DialogHeader>
            <DialogTitle className="font-mono text-sm">
              {compare?.sid}/{compare?.file.name}
            </DialogTitle>
            <DialogDescription>
              {mode === 'split'
                ? 'Drag the handle to compare baseline (left) with the latest run (right).'
                : 'Pixel diff — changed pixels in red over the faded baseline.'}
            </DialogDescription>
          </DialogHeader>
          {compare && (
            <div className="space-y-3">
              <div className="flex gap-1.5">
                {(['split', 'diff'] as const).map((m) => (
                  <Button
                    key={m}
                    size="sm"
                    variant={mode === m ? 'default' : 'outline'}
                    onClick={() => setMode(m)}
                  >
                    {m === 'split' ? 'Before / After' : 'Diff'}
                  </Button>
                ))}
              </div>
              {mode === 'split' ? (
                <SplitCompare sid={compare.sid} name={compare.file.name} />
              ) : (
                <DiffView sid={compare.sid} name={compare.file.name} hasDiff={compare.file.hasDiff} />
              )}
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  )
}

export default GoldensPage
