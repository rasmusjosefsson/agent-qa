// web/src/features/plugins/PluginsPage.tsx
// Lists discovered agent-qa plugins (out-of-process binaries that add vendor
// behavior — auth, session policy, setup hooks) and — the primary action —
// lets you INSTALL an extension package from npm/git/https without a terminal
// (the `agent-qa install` flow). Registering a raw binary path or importing a
// file are kept as advanced fallbacks.
import { useEffect, useRef, useState } from 'react'
import {
  DownloadIcon,
  PlusIcon,
  Loader2Icon,
  PlugIcon,
  XIcon,
  UploadIcon,
  RefreshCwIcon,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  getPlugins,
  getPluginPaths,
  setPluginPaths,
  importPlugin,
  installPackage,
  type InstallResult,
} from '@/lib/run-config-api'
import type { PluginInfo } from '@/features/environments/types'

export function PluginsPage() {
  const [plugins, setPlugins] = useState<PluginInfo[] | null>(null)
  const [available, setAvailable] = useState(true)
  const [paths, setPaths] = useState<string[]>([])
  const [draft, setDraft] = useState('')
  const [err, setErr] = useState('')
  const [installing, setInstalling] = useState(false)
  const [advanced, setAdvanced] = useState(false)
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
            Extension packages that add vendor behavior (auth, session policy, setup hooks). Install
            one from npm or git — no terminal needed.
          </p>
        </div>
        <div className="flex items-center gap-1.5">
          <Button variant="ghost" size="sm" onClick={() => void load()}>
            <RefreshCwIcon /> Refresh
          </Button>
          <Button size="sm" onClick={() => setInstalling(true)}>
            <DownloadIcon /> Install from npm / git
          </Button>
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

      <div className="min-h-0 flex-1 overflow-auto">
        {plugins === null ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading…
          </div>
        ) : plugins.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <PlugIcon className="size-8 text-muted-foreground/50" />
            <div>
              <div className="text-sm font-medium">No plugins installed</div>
              <p className="mx-auto mt-1 max-w-md text-xs text-muted-foreground">
                Install an extension package from npm or git and it's wired into your agent-qa config
                automatically.
              </p>
            </div>
            <Button size="sm" onClick={() => setInstalling(true)}>
              <DownloadIcon /> Install from npm / git
            </Button>
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

      {/* Advanced: register a raw binary path or import a file directly. */}
      <div className="border-t border-border bg-muted/20 px-5 py-2 text-[11px]">
        <button
          onClick={() => setAdvanced((v) => !v)}
          className="text-muted-foreground hover:text-foreground"
        >
          {advanced ? '▾' : '▸'} Advanced — register a binary path or import a file
        </button>
        {advanced && (
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
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
              placeholder="plugin binary path…"
              className="min-w-[14rem] flex-1 rounded border border-border bg-background px-2 py-0.5 font-mono outline-none focus-visible:border-ring"
            />
            <Button variant="ghost" size="sm" className="h-6 px-2" onClick={() => void addPath()}>
              <PlusIcon /> Add path
            </Button>
            <Button variant="ghost" size="sm" className="h-6 px-2" onClick={() => fileRef.current?.click()}>
              <UploadIcon /> Import file
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
        )}
      </div>

      {installing && (
        <InstallDialog
          onClose={() => setInstalling(false)}
          onInstalled={() => void load()}
        />
      )}
    </div>
  )
}

function InstallDialog({ onClose, onInstalled }: { onClose: () => void; onInstalled: () => void }) {
  const [source, setSource] = useState('')
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState('')
  const [result, setResult] = useState<InstallResult | null>(null)

  const run = async () => {
    const s = source.trim()
    if (!s) return
    setBusy(true)
    setErr('')
    setResult(null)
    try {
      const r = await installPackage(s)
      setBusy(false)
      if (!r.ok) {
        setErr(r.error || 'install failed')
        return
      }
      setResult(r)
      onInstalled()
    } catch (e) {
      setBusy(false)
      setErr(String((e as Error).message || e))
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Install plugin package</DialogTitle>
          <DialogDescription>
            Fetches an extension package and wires it into your agent-qa config — no terminal needed.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-2">
            <Label htmlFor="p-src">Source</Label>
            <Input
              id="p-src"
              autoFocus
              placeholder="npm:@acme/agent-qa-ext   ·   git:https://github.com/acme/ext"
              value={source}
              onChange={(e) => setSource(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !busy) {
                  e.preventDefault()
                  void run()
                }
              }}
            />
            <p className="text-[11px] text-muted-foreground">
              <code>npm:&lt;pkg&gt;</code>, <code>git:&lt;url&gt;</code>, or an <code>https://</code> git
              URL. Runs npm/git under the hood — may take a minute.
            </p>
          </div>
          {result?.ok && (
            <div className="rounded-md border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-xs">
              Installed <span className="font-medium">{result.name}</span> —{' '}
              {result.plugins?.length || 0} plugin(s)
              {result.plugins && result.plugins.length > 0 && (
                <span className="text-muted-foreground">
                  {' '}
                  ({result.plugins.map((p) => (p.kinds || []).join('/') || '?').join(', ')})
                </span>
              )}
              , {result.skills || 0} skill dir(s).
            </div>
          )}
          {err && <p className="whitespace-pre-wrap text-xs text-destructive">{err}</p>}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {result?.ok ? 'Done' : 'Cancel'}
          </Button>
          <Button onClick={() => void run()} disabled={!source.trim() || busy}>
            {busy && <Loader2Icon className="animate-spin" />} Install
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export default PluginsPage
