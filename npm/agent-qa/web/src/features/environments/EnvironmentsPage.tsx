import { useEffect, useRef, useState } from 'react'
import { PlusIcon, Loader2Icon, GlobeIcon, Trash2Icon, XIcon, UploadIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  getEnvironments,
  upsertEnvironment,
  deleteEnvironment,
  getPlugins,
  getPluginPaths,
  setPluginPaths,
  importPlugin,
} from '@/lib/run-config-api'
import type { EnvironmentRecord, PluginInfo } from './types'

function slugify(s: string): string {
  return (
    s
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 60) || 'environment'
  )
}

export function EnvironmentsPage() {
  const [envs, setEnvs] = useState<EnvironmentRecord[] | null>(null)
  const [plugins, setPlugins] = useState<PluginInfo[]>([])
  const [paths, setPaths] = useState<string[]>([])
  const [pluginDraft, setPluginDraft] = useState('')
  const [err, setErr] = useState('')
  const [editing, setEditing] = useState<EnvironmentRecord | 'new' | null>(null)

  const load = () =>
    getEnvironments()
      .then((r) => setEnvs(r.environments))
      .catch((e) => setErr(String(e.message || e)))
  const refreshPlugins = () =>
    Promise.all([getPluginPaths(), getPlugins()])
      .then(([cfg, disc]) => {
        setPaths(cfg.paths)
        setPlugins(disc.plugins)
      })
      .catch(() => {})
  useEffect(() => {
    void load()
    void refreshPlugins()
  }, [])

  const addPlugin = async () => {
    const p = pluginDraft.trim()
    if (!p) return
    await setPluginPaths([...paths, p]).catch((e) => setErr(String(e.message || e)))
    setPluginDraft('')
    void refreshPlugins()
  }
  const removePlugin = async (p: string) => {
    await setPluginPaths(paths.filter((x) => x !== p)).catch((e) => setErr(String(e.message || e)))
    void refreshPlugins()
  }
  const fileRef = useRef<HTMLInputElement | null>(null)
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
      void refreshPlugins()
    } catch (e) {
      setErr(String((e as Error).message || e))
    }
  }

  const remove = async (id: string) => {
    await deleteEnvironment(id).catch((e) => setErr(String(e.message || e)))
    void load()
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-border px-5 py-3">
        <div>
          <h1 className="text-base font-semibold tracking-tight">Environments</h1>
          <p className="text-xs text-muted-foreground">
            Where a test runs — a named target (base URL + values) passed to a run as params.
          </p>
        </div>
        <Button size="sm" onClick={() => setEditing('new')}>
          <PlusIcon /> New environment
        </Button>
      </div>

      {err && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-5 py-1.5 text-xs text-destructive">
          {err}
        </div>
      )}

      <div className="border-b border-border bg-muted/20 px-5 py-2 text-[11px]">
        <div className="mb-1 font-medium text-muted-foreground">Auth plugins</div>
        <div className="flex flex-wrap items-center gap-1.5">
          {paths.map((p) => (
            <span key={p} className="inline-flex items-center gap-1 rounded bg-muted px-1.5 py-0.5 font-mono">
              {p}
              <button onClick={() => void removePlugin(p)} className="opacity-60 hover:opacity-100" title="Remove">
                <XIcon className="size-3" />
              </button>
            </span>
          ))}
          <input
            value={pluginDraft}
            onChange={(e) => setPluginDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                void addPlugin()
              }
            }}
            placeholder="add auth-plugin binary path…"
            className="min-w-[14rem] flex-1 rounded border border-border bg-background px-2 py-0.5 font-mono outline-none focus-visible:border-ring"
          />
          <Button variant="ghost" size="sm" className="h-6 px-2" onClick={() => void addPlugin()}>
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
        <div className="mt-1 text-muted-foreground">
          {plugins.length > 0
            ? `Discovered: ${plugins
                .map((p) => {
                  const base = (p.binary || '').split('/').pop() || '?'
                  const kinds = (p.kinds || []).join(', ')
                  const bad = p.pingFailed ? ' — ping failed' : ''
                  return kinds ? `${base} [${kinds}]${bad}` : `${base}${bad}`
                })
                .join(', ')}`
            : 'None loaded yet — add the path to your auth-plugin binary above.'}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {envs === null ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading…
          </div>
        ) : envs.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <GlobeIcon className="size-8 text-muted-foreground/50" />
            <div>
              <div className="text-sm font-medium">No environments yet</div>
              <p className="mx-auto mt-1 max-w-sm text-xs text-muted-foreground">
                An environment names a target (e.g. Staging → its base URL) so the same test can run
                against different deployments.
              </p>
            </div>
            <Button size="sm" onClick={() => setEditing('new')}>
              <PlusIcon /> New environment
            </Button>
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 z-10 bg-background">
              <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                <th className="px-5 py-2 font-medium">Name</th>
                <th className="px-3 py-2 font-medium">Base URL</th>
                <th className="px-3 py-2 font-medium">Params</th>
                <th className="px-3 py-2 font-medium" />
              </tr>
            </thead>
            <tbody>
              {envs.map((e) => (
                <tr
                  key={e.id}
                  onClick={() => setEditing(e)}
                  className="cursor-pointer border-b border-border/60 transition-colors hover:bg-muted/40"
                >
                  <td className="px-5 py-2.5">
                    <div className="font-medium text-foreground">{e.name}</div>
                    {e.description && (
                      <div className="truncate text-xs text-muted-foreground">{e.description}</div>
                    )}
                  </td>
                  <td className="px-3 py-2.5 font-mono text-[11px] text-muted-foreground">
                    {e.baseUrl || '—'}
                  </td>
                  <td className="px-3 py-2.5 text-muted-foreground">{Object.keys(e.params).length}</td>
                  <td className="px-3 py-2.5 text-right">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7"
                      onClick={(ev) => {
                        ev.stopPropagation()
                        void remove(e.id)
                      }}
                    >
                      <Trash2Icon className="size-4" />
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {editing && (
        <EnvironmentDialog
          env={editing === 'new' ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null)
            void load()
          }}
        />
      )}
    </div>
  )
}

function EnvironmentDialog({
  env,
  onClose,
  onSaved,
}: {
  env: EnvironmentRecord | null
  onClose: () => void
  onSaved: () => void
}) {
  const [name, setName] = useState(env?.name ?? '')
  const [baseUrl, setBaseUrl] = useState(env?.baseUrl ?? '')
  const [description, setDescription] = useState(env?.description ?? '')
  const [rows, setRows] = useState<{ k: string; v: string }[]>(
    env ? Object.entries(env.params).map(([k, v]) => ({ k, v })) : []
  )
  const [authPlugin, setAuthPlugin] = useState(env?.auth?.plugin ?? '')
  const [authLoginUrl, setAuthLoginUrl] = useState(env?.auth?.loginUrl ?? '')
  const [authRows, setAuthRows] = useState<{ k: string; v: string }[]>(
    env ? Object.entries(env.auth?.config ?? {}).map(([k, v]) => ({ k, v })) : []
  )
  const [credRows, setCredRows] = useState<{ k: string; v: string }[]>(
    env ? Object.entries(env.auth?.creds ?? {}).map(([k, v]) => ({ k, v })) : []
  )
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState('')

  const setRow = (i: number, patch: Partial<{ k: string; v: string }>) =>
    setRows((prev) => prev.map((r, j) => (j === i ? { ...r, ...patch } : r)))
  const setAuthRow = (i: number, patch: Partial<{ k: string; v: string }>) =>
    setAuthRows((prev) => prev.map((r, j) => (j === i ? { ...r, ...patch } : r)))
  const setCredRow = (i: number, patch: Partial<{ k: string; v: string }>) =>
    setCredRows((prev) => prev.map((r, j) => (j === i ? { ...r, ...patch } : r)))

  const save = async () => {
    const n = name.trim()
    if (!n) return
    setBusy(true)
    try {
      const id = env?.id ?? slugify(n)
      const params: Record<string, string> = {}
      for (const { k, v } of rows) if (k.trim()) params[k.trim()] = v
      const config: Record<string, string> = {}
      for (const { k, v } of authRows) if (k.trim()) config[k.trim()] = v
      const creds: Record<string, string> = {}
      for (const { k, v } of credRows) if (k.trim()) creds[k.trim()] = v
      await upsertEnvironment(id, {
        name: n,
        baseUrl: baseUrl.trim(),
        params,
        auth: { plugin: authPlugin.trim(), loginUrl: authLoginUrl.trim(), config, creds },
        description,
      })
      onSaved()
    } catch (e) {
      setErr(String((e as Error).message || e))
      setBusy(false)
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{env ? 'Edit environment' : 'New environment'}</DialogTitle>
          <DialogDescription>Name the target and the values runs should use.</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-2">
            <Label htmlFor="e-name">Name</Label>
            <Input id="e-name" autoFocus placeholder="e.g. Staging" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="space-y-2">
            <Label htmlFor="e-url">Base URL</Label>
            <Input id="e-url" placeholder="https://staging.example.com" value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
          </div>
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>Extra params</Label>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 px-2 text-xs"
                onClick={() => setRows((prev) => [...prev, { k: '', v: '' }])}
              >
                <PlusIcon /> Add
              </Button>
            </div>
            {rows.length === 0 ? (
              <p className="text-[11px] text-muted-foreground">
                Optional key/value pairs forwarded to the run as <code>--param</code>.
              </p>
            ) : (
              <div className="space-y-1.5">
                {rows.map((r, i) => (
                  <div key={i} className="flex items-center gap-1.5">
                    <Input
                      className="h-8 flex-1"
                      placeholder="key"
                      value={r.k}
                      onChange={(e) => setRow(i, { k: e.target.value })}
                    />
                    <Input
                      className="h-8 flex-1"
                      placeholder="value"
                      value={r.v}
                      onChange={(e) => setRow(i, { v: e.target.value })}
                    />
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-8 shrink-0"
                      onClick={() => setRows((prev) => prev.filter((_, j) => j !== i))}
                    >
                      <XIcon className="size-4" />
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </div>
          <div className="space-y-2 rounded-md border border-border p-3">
            <Label className="text-xs uppercase tracking-wide text-muted-foreground">
              Connection (optional)
            </Label>
            <div className="space-y-2">
              <Input
                placeholder="auth plugin name (agent-qa.toml [plugins] auth)"
                value={authPlugin}
                onChange={(e) => setAuthPlugin(e.target.value)}
              />
              <Input
                placeholder="login / SSO entry URL"
                value={authLoginUrl}
                onChange={(e) => setAuthLoginUrl(e.target.value)}
              />
              <div className="flex items-center justify-between">
                <span className="text-[11px] text-muted-foreground">Plugin config</span>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-xs"
                  onClick={() => setAuthRows((prev) => [...prev, { k: '', v: '' }])}
                >
                  <PlusIcon /> Add
                </Button>
              </div>
              {authRows.map((r, i) => (
                <div key={i} className="flex items-center gap-1.5">
                  <Input className="h-8 flex-1" placeholder="key" value={r.k} onChange={(e) => setAuthRow(i, { k: e.target.value })} />
                  <Input className="h-8 flex-1" placeholder="value" value={r.v} onChange={(e) => setAuthRow(i, { v: e.target.value })} />
                  <Button
                    variant="ghost"
                    size="icon"
                    className="size-8 shrink-0"
                    onClick={() => setAuthRows((prev) => prev.filter((_, j) => j !== i))}
                  >
                    <XIcon className="size-4" />
                  </Button>
                </div>
              ))}
              <p className="text-[11px] text-muted-foreground">
                Names a downstream auth plugin that signs in here. The plugin binary lives in your
                own repo — nothing vendor-specific is stored in the workbench.
              </p>
            </div>
            <div className="mt-2 space-y-2 border-t border-border pt-2">
              <div className="flex items-center justify-between">
                <span className="text-[11px] text-muted-foreground">Shared credentials</span>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-xs"
                  onClick={() => setCredRows((prev) => [...prev, { k: '', v: '' }])}
                >
                  <PlusIcon /> Add
                </Button>
              </div>
              {credRows.map((r, i) => (
                <div key={i} className="flex items-center gap-1.5">
                  <Input className="h-8 flex-1 font-mono text-xs" placeholder="ENV_VAR" value={r.k} onChange={(e) => setCredRow(i, { k: e.target.value })} />
                  <Input className="h-8 flex-[1.4] font-mono text-xs" placeholder="value or vault:path:key" value={r.v} onChange={(e) => setCredRow(i, { v: e.target.value })} />
                  <Button
                    variant="ghost"
                    size="icon"
                    className="size-8 shrink-0"
                    onClick={() => setCredRows((prev) => prev.filter((_, j) => j !== i))}
                  >
                    <XIcon className="size-4" />
                  </Button>
                </div>
              ))}
              <p className="text-[11px] text-muted-foreground">
                App-level creds every persona here reuses (e.g. an OAuth client id). Injected as env
                vars and merged <em>under</em> a persona's own creds — the persona wins on conflict,
                so it only carries what varies (email/password). Literal or{' '}
                <code>vault:&lt;path&gt;:&lt;key&gt;</code>.
              </p>
            </div>
          </div>
          <div className="space-y-2">
            <Label htmlFor="e-desc">Notes</Label>
            <Textarea id="e-desc" rows={2} value={description} onChange={(e) => setDescription(e.target.value)} />
          </div>
          {err && <p className="text-xs text-destructive">{err}</p>}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={() => void save()} disabled={!name.trim() || busy}>
            {busy && <Loader2Icon className="animate-spin" />} Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export default EnvironmentsPage
