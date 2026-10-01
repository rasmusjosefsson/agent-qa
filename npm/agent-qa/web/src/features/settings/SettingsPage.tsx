// web/src/features/settings/SettingsPage.tsx
// Workbench preferences: chat backend, replay defaults, and version/path info.
// Values persist to <root>/_config/settings.json via /api/config/settings;
// env vars override stored values and are flagged inline.
import { useEffect, useState } from 'react'
import { Loader2Icon, RefreshCwIcon, Settings2Icon } from 'lucide-react'

import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  getSettings,
  updateSettings,
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

  const load = () =>
    getSettings()
      .then(setData)
      .catch((e) => setErr(String(e.message || e)))

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

export default SettingsPage
