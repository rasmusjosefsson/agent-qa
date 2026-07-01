// web/src/features/plugins/PluginsPage.tsx
// Lists discovered agent-qa plugins (out-of-process binaries that add vendor
// behavior — auth, session policy, setup hooks) with their kind, source, and
// ping status, and lets you register a binary path or import a downloaded
// plugin file. Read-mostly; mirrors the run-config pages.
import { useEffect, useRef, useState } from 'react'
import { PlusIcon, Loader2Icon, PlugIcon, XIcon, UploadIcon, RefreshCwIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { getPlugins, getPluginPaths, setPluginPaths, importPlugin } from '@/lib/run-config-api'
import type { PluginInfo } from '@/features/environments/types'

export function PluginsPage() {
  const [plugins, setPlugins] = useState<PluginInfo[] | null>(null)
  const [available, setAvailable] = useState(true)
  const [paths, setPaths] = useState<string[]>([])
  const [draft, setDraft] = useState('')
  const [err, setErr] = useState('')
  const fileRef = useRef<HTMLInputElement | null>(null)

  const load = () =>
    Promise.all([getPlugins(), getPluginPaths()])
      .then(([disc, cfg]) => {
        setPlugins(disc.plugins)
        setAvailable(disc.available)
        setPaths(cfg.paths)
      })
      .catch((e) => setErr(String(e.message || e)))
  useEffect(() => {
    void load()
  }, [])

  const addPath = async () => {
    const p = draft.trim()
    if (!p) return
    await setPluginPaths([...paths, p]).catch((e) => setErr(String(e.message || e)))
    setDraft('')
    void load()
  }
  const removePath = async (p: string) => {
    await setPluginPaths(paths.filter((x) => x !== p)).catch((e) => setErr(String(e.message || e)))
    void load()
  }
  const onImportFile = async (file: File) => {
    try {
      const dataUrl = await new Promise<string>((resolve, reject) => {
        const r = new FileReader()
        r.onload = () => resolve(String(r.result))
        r.onerror = () => reject(r.error)
        r.readAsDataURL(file)
      })
      const b64 = dataUrl.split(',')[1] || ''
      await importPlugin(file.name, b64)
      void load()
    } catch (e) {
      setErr(String((e as Error).message || e))
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-border px-5 py-3">
        <div>
          <h1 className="text-base font-semibold tracking-tight">Plugins</h1>
          <p className="text-xs text-muted-foreground">
            Out-of-process binaries that add vendor behavior (auth, session policy, setup hooks).
            Discovered from <code>agent-qa.toml</code>, <code>$PATH</code>, and paths you register here.
          </p>
        </div>
        <div className="flex items-center gap-1.5">
          <Button variant="ghost" size="sm" onClick={() => void load()}>
            <RefreshCwIcon /> Refresh
          </Button>
          <Button size="sm" onClick={() => fileRef.current?.click()}>
            <UploadIcon /> Import plugin
          </Button>
          <input
            ref={fileRef}
            type="file"
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0]
              if (f) void onImportFile(f)
              e.target.value = ''
            }}
          />
        </div>
      </div>

      {err && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-5 py-1.5 text-xs text-destructive">
          {err}
        </div>
      )}
      {!available && (
        <div className="border-b border-border bg-muted/20 px-5 py-1.5 text-xs text-muted-foreground">
          Plugin discovery unavailable — the agent-qa CLI binary isn't resolved.
        </div>
      )}

      <div className="border-b border-border bg-muted/20 px-5 py-2 text-[11px]">
        <div className="mb-1 font-medium text-muted-foreground">
          Registered paths (injected as <code>AGENT_QA_PLUGINS</code>)
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          {paths.map((p) => (
            <span key={p} className="inline-flex items-center gap-1 rounded bg-muted px-1.5 py-0.5 font-mono">
              {p}
              <button onClick={() => void removePath(p)} className="opacity-60 hover:opacity-100" title="Remove">
                <XIcon className="size-3" />
              </button>
            </span>
          ))}
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                void addPath()
              }
            }}
            placeholder="add plugin binary path…"
            className="min-w-[14rem] flex-1 rounded border border-border bg-background px-2 py-0.5 font-mono outline-none focus-visible:border-ring"
          />
          <Button variant="ghost" size="sm" className="h-6 px-2" onClick={() => void addPath()}>
            <PlusIcon /> Add path
          </Button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {plugins === null ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading…
          </div>
        ) : plugins.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <PlugIcon className="size-8 text-muted-foreground/50" />
            <div>
              <div className="text-sm font-medium">No plugins discovered</div>
              <p className="mx-auto mt-1 max-w-md text-xs text-muted-foreground">
                Register a plugin binary path above, drop one on <code>$PATH</code> named{' '}
                <code>agent-qa-plugin-*</code>, or declare it in <code>agent-qa.toml</code>.
              </p>
            </div>
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 z-10 bg-background">
              <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                <th className="px-5 py-2 font-medium">Plugin</th>
                <th className="px-3 py-2 font-medium">Kinds</th>
                <th className="px-3 py-2 font-medium">Source</th>
                <th className="px-3 py-2 font-medium">Status</th>
              </tr>
            </thead>
            <tbody>
              {plugins.map((p) => {
                const base = (p.binary || '').split('/').pop() || '?'
                return (
                  <tr key={p.binary || base} className="border-b border-border/60">
                    <td className="px-5 py-2.5">
                      <div className="font-medium text-foreground">{base}</div>
                      <div className="truncate font-mono text-[11px] text-muted-foreground" title={p.binary}>
                        {p.binary || '—'}
                      </div>
                    </td>
                    <td className="px-3 py-2.5 text-xs">{(p.kinds || []).join(', ') || '—'}</td>
                    <td className="px-3 py-2.5 text-xs text-muted-foreground">{p.source || '—'}</td>
                    <td className="px-3 py-2.5 text-xs">
                      {p.pingFailed ? (
                        <span className="rounded-sm bg-destructive/15 px-1.5 py-0.5 text-destructive">
                          ping failed
                        </span>
                      ) : (
                        <span className="rounded-sm bg-emerald-500/15 px-1.5 py-0.5 text-emerald-400">
                          ok{p.declared ? ' · declared' : ''}
                        </span>
                      )}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}

export default PluginsPage
