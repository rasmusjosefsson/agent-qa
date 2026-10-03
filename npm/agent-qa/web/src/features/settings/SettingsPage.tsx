// web/src/features/settings/SettingsPage.tsx
// Workbench preferences: chat backend, replay defaults, and version/path info.
// Values persist to <root>/_config/settings.json via /api/config/settings;
// env vars override stored values and are flagged inline.
import { useEffect, useState, type ChangeEvent } from 'react'
import {
  ArrowDownToLineIcon,
  ArrowUpToLineIcon,
  Loader2Icon,
  RefreshCwIcon,
  Settings2Icon,
} from 'lucide-react'

import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  getBaselinesConfig,
  getBaselinesStatus,
  syncBaselines,
  updateBaselinesConfig,
  getSettings,
  updateSettings,
  type BaselinesConfig,
  type BaselinesStatus,
  type BaselineStore,
  type ChatBackend,
  type SettingsResponse,
} from '@/lib/settings-api'
import { PageHeader, PageError } from '@/components/page-header'
import { LoadingState } from '@/components/empty-state'

export function SettingsPage() {
  const [data, setData] = useState<SettingsResponse | null>(null)
  const [err, setErr] = useState('')
  const [saving, setSaving] = useState(false)
  const [savedTick, setSavedTick] = useState(0)

  // Golden storage — [baselines] in agent-qa.toml + the CLI sync verbs.
  const [bForm, setBForm] = useState<BaselinesConfig | null>(null)
  const [bStatus, setBStatus] = useState<BaselinesStatus | null>(null)
  const [bBusy, setBBusy] = useState(false)
  const [bOut, setBOut] = useState('')

  const loadBaselines = () => {
    getBaselinesConfig()
      .then(setBForm)
      .catch(() => setBForm(null))
    getBaselinesStatus()
      .then(setBStatus)
      .catch(() => setBStatus(null))
  }

  const load = () => {
    getSettings()
      .then(setData)
      .catch((e) => setErr(String(e.message || e)))
    loadBaselines()
  }

  useEffect(() => {
    void load()
  }, [])

  const patch = async (p: { chatBackend?: ChatBackend; headedDefault?: boolean }) => {
    setSaving(true)
    setErr('')
    try {
      const r = await updateSettings(p)
      setData(r)
      setSavedTick((n) => n + 1)
    } catch (e) {
      setErr(String((e as Error).message || e))
    } finally {
      setSaving(false)
    }
  }

  const envForced = !!data?.env.chatBackend
  const backendDiffers =
    !!data &&
    data.effective.chatBackend !== 'auto' &&
    // A stored backend only takes effect on the next workbench start — the
    // chat hub is resolved once at boot.
    true

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader
        icon={Settings2Icon}
        title="Settings"
        description={
          <>
            Workbench preferences, stored in <span className="font-mono">_config/settings.json</span>{' '}
            next to your scenarios. Environment variables override stored values.
          </>
        }
        actions={
          <>
            {saving && <Loader2Icon className="size-3.5 animate-spin text-muted-foreground" />}
            {savedTick > 0 && !saving && !err && (
              <span className="text-xs text-muted-foreground">Saved</span>
            )}
            <Button variant="ghost" size="sm" onClick={() => void load()}>
              <RefreshCwIcon /> Refresh
            </Button>
          </>
        }
      />

      {err && (
        <PageError>{err}</PageError>
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        {!data ? (
          <LoadingState>Loading settings…</LoadingState>
        ) : (
          <div className="mx-auto w-full max-w-2xl space-y-6 p-6">
            {/* Chat */}
            <section className="space-y-3">
              <div className="px-1 text-[10.5px] font-semibold uppercase tracking-[0.07em] text-muted-foreground">
                Chat
              </div>
              <div className="aqa-elevated rounded-xl border border-border bg-card p-4">
                <div className="flex items-center justify-between gap-4">
                  <div className="space-y-1">
                    <Label htmlFor="chat-backend">Agent backend</Label>
                    <p className="text-xs text-muted-foreground">
                      Which agent the Chat tab talks to. <span className="font-mono">auto</span>{' '}
                      picks whichever backend is installed.
                    </p>
                  </div>
                  <Select
                    value={data.effective.chatBackend}
                    disabled={saving || envForced}
                    onValueChange={(v) => void patch({ chatBackend: v as ChatBackend })}
                  >
                    <SelectTrigger size="sm" className="w-36" aria-label="Agent backend">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="auto">auto</SelectItem>
                      <SelectItem value="pi">pi</SelectItem>
                      <SelectItem value="opencode">opencode</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                {envForced && (
                  <p className="mt-2 text-xs text-muted-foreground">
                    Locked by <span className="font-mono">AGENT_QA_CHAT_BACKEND</span>=
                    {data.env.chatBackend} — unset it to change this here.
                  </p>
                )}
                {backendDiffers && !envForced && (
                  <p className="mt-2 text-xs text-muted-foreground">
                    Applies to the next workbench start — the chat backend is resolved once at boot.
                  </p>
                )}
              </div>
            </section>

            {/* Replay */}
            <section className="space-y-3">
              <div className="px-1 text-[10.5px] font-semibold uppercase tracking-[0.07em] text-muted-foreground">
                Replay
              </div>
              <div className="aqa-elevated rounded-xl border border-border bg-card p-4">
                <div className="flex items-center justify-between gap-4">
                  <div className="space-y-1">
                    <Label htmlFor="headed-default">Headed browser by default</Label>
                    <p className="text-xs text-muted-foreground">
                      New replays and persona connects start with a visible browser window instead of
                      headless. Per-run toggles still win.
                    </p>
                  </div>
                  <Checkbox
                    id="headed-default"
                    checked={data.effective.headedDefault}
                    disabled={saving}
                    onCheckedChange={(v) => void patch({ headedDefault: v === true })}
                  />
                </div>
              </div>
            </section>

            {/* Golden storage */}
            <section className="space-y-3">
              <div className="px-1 text-[10.5px] font-semibold uppercase tracking-[0.07em] text-muted-foreground">
                Golden storage
              </div>
              <div className="aqa-elevated rounded-xl border border-border bg-card p-4">
                {!bForm ? (
                  <p className="text-xs text-muted-foreground">
                    Store backend unavailable — start the workbench with the agent-qa CLI
                    resolved.
                  </p>
                ) : (
                  <GoldenStorageCard
                    form={bForm}
                    status={bStatus}
                    busy={bBusy}
                    out={bOut}
                    onChange={setBForm}
                    onSave={async () => {
                      setBBusy(true)
                      setErr('')
                      setBOut('')
                      try {
                        await updateBaselinesConfig({
                          store: bForm.store,
                          repo: bForm.repo || undefined,
                          branch: bForm.branch || undefined,
                          prefix: bForm.prefix || undefined,
                          url: bForm.url || undefined,
                          tokenEnv: bForm.tokenEnv || undefined,
                        })
                        loadBaselines()
                        setBOut('Saved to agent-qa.toml')
                      } catch (e) {
                        setErr(String((e as Error).message || e))
                      } finally {
                        setBBusy(false)
                      }
                    }}
                    onSync={async (direction) => {
                      setBBusy(true)
                      setBOut('')
                      try {
                        const r = await syncBaselines(direction)
                        setBOut(
                          (r.stdout + (r.stderr ? `\n${r.stderr}` : '')).trim() ||
                            `exit ${r.code}`,
                        )
                        loadBaselines()
                      } catch (e) {
                        setErr(String((e as Error).message || e))
                      } finally {
                        setBBusy(false)
                      }
                    }}
                  />
                )}
              </div>
            </section>

            {/* Paths */}
            <section className="space-y-3">
              <div className="px-1 text-[10.5px] font-semibold uppercase tracking-[0.07em] text-muted-foreground">
                Paths
              </div>
              <div className="aqa-elevated rounded-xl border border-border bg-card p-4">
                <dl className="space-y-2 text-xs">
                  <div className="flex justify-between gap-4">
                    <dt className="w-24 shrink-0 text-muted-foreground">Scenario root</dt>
                    <dd data-qa-volatile className="min-w-0 flex-1 truncate font-mono" title={data.root}>{data.root}</dd>
                  </div>
                  <div className="flex justify-between gap-4">
                    <dt className="w-24 shrink-0 text-muted-foreground">Settings file</dt>
                    <dd data-qa-volatile className="min-w-0 flex-1 truncate font-mono" title={`${data.root}/_config/settings.json`}>{data.root}/_config/settings.json</dd>
                  </div>
                </dl>
              </div>
            </section>
          </div>
        )}
      </div>
    </div>
  )
}

// The [baselines] table editor + sync actions. Config is written to the
// agent-qa.toml the CLI discovers — CLI and workbench share one source.
function GoldenStorageCard({
  form,
  status,
  busy,
  out,
  onChange,
  onSave,
  onSync,
}: {
  form: BaselinesConfig
  status: BaselinesStatus | null
  busy: boolean
  out: string
  onChange: (f: BaselinesConfig) => void
  onSave: () => void
  onSync: (d: 'pull' | 'push') => void
}) {
  const set = (k: keyof BaselinesConfig) => (e: ChangeEvent<HTMLInputElement>) =>
    onChange({ ...form, [k]: e.target.value })

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-4">
        <div className="space-y-1">
          <Label htmlFor="baselines-store">Store backend</Label>
          <p className="text-xs text-muted-foreground">
            Where shot/domshot goldens live. <span className="font-mono">local</span> keeps them
            in the scenario repo (default); <span className="font-mono">github</span> and{' '}
            <span className="font-mono">turso</span> sync them to a remote store.
          </p>
        </div>
        <Select
          value={form.store}
          disabled={busy}
          onValueChange={(v) => onChange({ ...form, store: v as BaselineStore })}
        >
          <SelectTrigger size="sm" className="w-36" aria-label="Golden store backend">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="local">local (repo)</SelectItem>
            <SelectItem value="github">github repo</SelectItem>
            <SelectItem value="turso">turso db</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {form.store === 'github' && (
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label htmlFor="bs-repo">Repository</Label>
            <Input
              id="bs-repo"
              placeholder="owner/agent-qa-goldens"
              value={form.repo}
              onChange={set('repo')}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="bs-branch">Branch</Label>
            <Input
              id="bs-branch"
              placeholder="main"
              value={form.branch}
              onChange={set('branch')}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="bs-prefix">Path prefix</Label>
            <Input
              id="bs-prefix"
              placeholder="baselines"
              value={form.prefix}
              onChange={set('prefix')}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="bs-token">Token env var</Label>
            <Input
              id="bs-token"
              placeholder="GITHUB_TOKEN"
              value={form.tokenEnv}
              onChange={set('tokenEnv')}
            />
          </div>
        </div>
      )}

      {form.store === 'turso' && (
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label htmlFor="bs-url">Database URL</Label>
            <Input
              id="bs-url"
              placeholder="libsql://db-org.turso.io"
              value={form.url}
              onChange={set('url')}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="bs-ttoken">Token env var</Label>
            <Input
              id="bs-ttoken"
              placeholder="TURSO_AUTH_TOKEN"
              value={form.tokenEnv}
              onChange={set('tokenEnv')}
            />
          </div>
        </div>
      )}

      {form.store !== 'local' && (
        <p className="text-xs text-muted-foreground">
          Replay pulls goldens before the step loop; accept and{' '}
          <span className="font-mono">--update-baselines</span> push after minting. Sync is
          additive — files missing on one side are never deleted on the other.
        </p>
      )}

      {status && (
        <p className="text-xs text-muted-foreground">
          Store: <span className="font-mono">{status.store}</span>
        </p>
      )}

      <div className="flex items-center gap-2">
        <Button size="sm" disabled={busy} onClick={onSave}>
          {busy ? <Loader2Icon className="animate-spin" /> : null}
          Save
        </Button>
        {form.store !== 'local' && (
          <>
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => onSync('pull')}
            >
              <ArrowDownToLineIcon /> Pull
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => onSync('push')}
            >
              <ArrowUpToLineIcon /> Push
            </Button>
          </>
        )}
      </div>

      {out && (
        <pre className="max-h-40 overflow-auto rounded-md bg-muted/60 p-3 text-[11px]">
          {out}
        </pre>
      )}
    </div>
  )
}

export default SettingsPage
