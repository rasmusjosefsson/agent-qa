import { useCallback, useEffect, useState } from 'react'
import { DatabaseIcon, FileTextIcon, ImageIcon, ImagesIcon, RefreshCwIcon } from 'lucide-react'
import { PageHeader } from '@/components/page-header'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { fetchGoldens, goldenFileUrl, type GoldensResponse } from '@/lib/goldens-api'

function fmtSize(bytes?: number): string {
  if (!bytes) return ''
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

// Baselines (goldens) across every scenario — shot PNGs as thumbnails,
// domshot snapshots as text entries. Files shown are the local copy of
// whichever [baselines] store is configured (pulled/minted content).
export function GoldensPage() {
  const [data, setData] = useState<GoldensResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

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
                <figure
                  key={f.name}
                  className="overflow-hidden rounded-lg border border-border/70 bg-muted/30"
                >
                  {f.name.endsWith('.png') ? (
                    <a href={goldenFileUrl(s.sid, f.name)} target="_blank" rel="noreferrer">
                      <img
                        src={goldenFileUrl(s.sid, f.name)}
                        alt={`${s.sid} ${f.name}`}
                        loading="lazy"
                        className="aspect-video w-full object-cover object-top transition-transform hover:scale-[1.02]"
                      />
                    </a>
                  ) : (
                    <div className="grid aspect-video w-full place-items-center text-muted-foreground/60">
                      {f.name.endsWith('.txt') ? (
                        <FileTextIcon className="size-8" />
                      ) : (
                        <ImageIcon className="size-8" />
                      )}
                    </div>
                  )}
                  <figcaption className="flex items-center justify-between gap-2 px-2.5 py-1.5">
                    <span className="truncate font-mono text-[11px] text-muted-foreground">
                      {f.name}
                    </span>
                    <span className="shrink-0 text-[10px] text-muted-foreground/60">
                      {fmtSize(f.size)}
                    </span>
                  </figcaption>
                </figure>
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
    </div>
  )
}

export default GoldensPage
