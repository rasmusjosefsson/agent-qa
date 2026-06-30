import { useEffect, useState } from 'react'
import { PlusIcon, Loader2Icon, UsersIcon, Trash2Icon, KeyRoundIcon } from 'lucide-react'
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
  getPersonas,
  upsertPersona,
  deletePersona,
  getSecretSources,
  upsertSecretSource,
  deleteSecretSource,
} from '@/lib/run-config-api'
import type { PersonaRecord, SecretSourceRecord } from './types'

function slugify(s: string): string {
  return (
    s
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 60) || 'persona'
  )
}

export function PersonasPage() {
  const [personas, setPersonas] = useState<PersonaRecord[] | null>(null)
  const [sources, setSources] = useState<SecretSourceRecord[]>([])
  const [err, setErr] = useState('')
  const [editing, setEditing] = useState<PersonaRecord | 'new' | null>(null)
  const [editingSrc, setEditingSrc] = useState<SecretSourceRecord | 'new' | null>(null)

  const load = () =>
    getPersonas()
      .then((r) => setPersonas(r.personas))
      .catch((e) => setErr(String(e.message || e)))
  const loadSources = () =>
    getSecretSources()
      .then((r) => setSources(r.secretSources))
      .catch(() => {})
  useEffect(() => {
    void load()
    void loadSources()
  }, [])

  const remove = async (id: string) => {
    await deletePersona(id).catch((e) => setErr(String(e.message || e)))
    void load()
  }
  const removeSrc = async (id: string) => {
    await deleteSecretSource(id).catch((e) => setErr(String(e.message || e)))
    void loadSources()
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-border px-5 py-3">
        <div>
          <h1 className="text-base font-semibold tracking-tight">Personas</h1>
          <p className="text-xs text-muted-foreground">
            Who a test runs as — a saved login identity, passed to a run as its profile.
          </p>
        </div>
        <Button size="sm" onClick={() => setEditing('new')}>
          <PlusIcon /> New persona
        </Button>
      </div>

      {err && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-5 py-1.5 text-xs text-destructive">
          {err}
        </div>
      )}

      {/* Vault targets — secret sources a persona can pull credentials from. */}
      <div className="border-b border-border bg-muted/20 px-5 py-2 text-[11px]">
        <div className="mb-1 flex items-center justify-between">
          <span className="flex items-center gap-1.5 font-medium text-muted-foreground">
            <KeyRoundIcon className="size-3.5" /> Vault targets
          </span>
          <Button variant="ghost" size="sm" className="h-6 px-2" onClick={() => setEditingSrc('new')}>
            <PlusIcon /> New vault target
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          {sources.length === 0 ? (
            <span className="text-muted-foreground">
              None yet — a vault target is a command that fetches secrets (so no one exports
              passwords).
            </span>
          ) : (
            sources.map((s) => (
              <span key={s.id} className="inline-flex items-center gap-1 rounded bg-muted px-1.5 py-0.5">
                <button onClick={() => setEditingSrc(s)} className="hover:underline">
                  {s.name}
                </button>
                <button onClick={() => void removeSrc(s.id)} className="opacity-60 hover:opacity-100" title="Remove">
                  ✕
                </button>
              </span>
            ))
          )}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {personas === null ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading…
          </div>
        ) : personas.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <UsersIcon className="size-8 text-muted-foreground/50" />
            <div>
              <div className="text-sm font-medium">No personas yet</div>
              <p className="mx-auto mt-1 max-w-sm text-xs text-muted-foreground">
                A persona is a saved login the agent reuses, so a run can sign in as a specific user.
              </p>
            </div>
            <Button size="sm" onClick={() => setEditing('new')}>
              <PlusIcon /> New persona
            </Button>
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 z-10 bg-background">
              <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                <th className="px-5 py-2 font-medium">Name</th>
                <th className="px-3 py-2 font-medium">Profile</th>
                <th className="px-3 py-2 font-medium" />
              </tr>
            </thead>
            <tbody>
              {personas.map((p) => (
                <tr
                  key={p.id}
                  onClick={() => setEditing(p)}
                  className="cursor-pointer border-b border-border/60 transition-colors hover:bg-muted/40"
                >
                  <td className="px-5 py-2.5">
                    <div className="font-medium text-foreground">{p.name}</div>
                    {p.description && (
                      <div className="truncate text-xs text-muted-foreground">{p.description}</div>
                    )}
                  </td>
                  <td className="px-3 py-2.5 font-mono text-[11px] text-muted-foreground">
                    {p.profile || '—'}
                  </td>
                  <td className="px-3 py-2.5 text-right">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7"
                      onClick={(e) => {
                        e.stopPropagation()
                        void remove(p.id)
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
        <PersonaDialog
          persona={editing === 'new' ? null : editing}
          sources={sources}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null)
            void load()
          }}
        />
      )}
      {editingSrc && (
        <SecretSourceDialog
          source={editingSrc === 'new' ? null : editingSrc}
          onClose={() => setEditingSrc(null)}
          onSaved={() => {
            setEditingSrc(null)
            void loadSources()
          }}
        />
      )}
    </div>
  )
}

function PersonaDialog({
  persona,
  sources,
  onClose,
  onSaved,
}: {
  persona: PersonaRecord | null
  sources: SecretSourceRecord[]
  onClose: () => void
  onSaved: () => void
}) {
  const [name, setName] = useState(persona?.name ?? '')
  const [profile, setProfile] = useState(persona?.profile ?? '')
  const [envPrefix, setEnvPrefix] = useState(persona?.credentials?.envPrefix ?? '')
  const [secretSourceId, setSecretSourceId] = useState(persona?.secretSourceId ?? '')
  const [description, setDescription] = useState(persona?.description ?? '')
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState('')

  const save = async () => {
    const n = name.trim()
    if (!n) return
    setBusy(true)
    try {
      const id = persona?.id ?? slugify(n)
      await upsertPersona(id, {
        name: n,
        profile: profile.trim(),
        credentials: { envPrefix: envPrefix.trim() },
        secretSourceId,
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
          <DialogTitle>{persona ? 'Edit persona' : 'New persona'}</DialogTitle>
          <DialogDescription>
            Give it a name and the profile to sign in with at run time.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-2">
            <Label htmlFor="p-name">Name</Label>
            <Input id="p-name" autoFocus placeholder="e.g. Admin user" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="space-y-2">
            <Label htmlFor="p-profile">Profile</Label>
            <Input
              id="p-profile"
              placeholder="agent-qa profile name (passed as --profile)"
              value={profile}
              onChange={(e) => setProfile(e.target.value)}
            />
            <p className="text-[11px] text-muted-foreground">
              A registered agent-qa auth profile (see <code>agent-qa profile-list</code>).
            </p>
          </div>
          <div className="space-y-2">
            <Label htmlFor="p-env">Secrets env prefix</Label>
            <Input
              id="p-env"
              placeholder="e.g. AGENT_QA_PROFILE_ADMIN"
              value={envPrefix}
              onChange={(e) => setEnvPrefix(e.target.value)}
            />
            <p className="text-[11px] text-muted-foreground">
              Env-var prefix the auth plugin reads this login's secrets from — no secrets are stored
              here.
            </p>
          </div>
          <div className="space-y-2">
            <Label htmlFor="p-src">Vault target (optional)</Label>
            <select
              id="p-src"
              value={secretSourceId}
              onChange={(e) => setSecretSourceId(e.target.value)}
              className="w-full rounded-md border border-border bg-background px-2 py-1.5 text-sm outline-none focus-visible:border-ring"
            >
              <option value="">none — use exported env vars</option>
              {sources.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
            <p className="text-[11px] text-muted-foreground">
              Fetch this login's secrets from a vault target at run time, instead of exporting them.
            </p>
          </div>
          <div className="space-y-2">
            <Label htmlFor="p-desc">Notes</Label>
            <Textarea id="p-desc" rows={2} value={description} onChange={(e) => setDescription(e.target.value)} />
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

function SecretSourceDialog({
  source,
  onClose,
  onSaved,
}: {
  source: SecretSourceRecord | null
  onClose: () => void
  onSaved: () => void
}) {
  const [name, setName] = useState(source?.name ?? '')
  const [mode, setMode] = useState<'inline' | 'command'>(source?.mode ?? 'inline')
  const [rows, setRows] = useState<{ k: string; v: string }[]>(
    source ? Object.entries(source.entries ?? {}).map(([k, v]) => ({ k, v })) : [{ k: '', v: '' }]
  )
  const [command, setCommand] = useState(source?.command ?? '')
  const [description, setDescription] = useState(source?.description ?? '')
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState('')

  const setRow = (i: number, patch: Partial<{ k: string; v: string }>) =>
    setRows((prev) => prev.map((r, j) => (j === i ? { ...r, ...patch } : r)))

  const save = async () => {
    const n = name.trim()
    if (!n) return
    setBusy(true)
    try {
      const id = source?.id ?? slugify(n)
      const entries: Record<string, string> = {}
      for (const { k, v } of rows) if (k.trim()) entries[k.trim()] = v
      await upsertSecretSource(id, { name: n, mode, entries, command, description })
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
          <DialogTitle>{source ? 'Edit vault target' : 'New vault target'}</DialogTitle>
          <DialogDescription>Secrets passed to the auth plugin at run time.</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-2">
            <Label htmlFor="s-name">Name</Label>
            <Input id="s-name" autoFocus placeholder="e.g. staging-creds" value={name} onChange={(e) => setName(e.target.value)} />
          </div>

          <div className="grid grid-cols-2 gap-2">
            {(['inline', 'command'] as const).map((m) => (
              <button
                key={m}
                type="button"
                onClick={() => setMode(m)}
                className={`rounded-md border px-3 py-2 text-left text-xs transition-colors ${
                  mode === m ? 'border-primary bg-primary/10' : 'border-border hover:bg-muted/40'
                }`}
              >
                <div className="font-medium">{m === 'inline' ? 'Key / value' : 'Command'}</div>
                <div className="mt-0.5 text-muted-foreground">
                  {m === 'inline' ? 'Type the secrets in' : 'Fetch from a vault'}
                </div>
              </button>
            ))}
          </div>

          {mode === 'inline' ? (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label>Secrets</Label>
                <Button variant="ghost" size="sm" className="h-7 px-2 text-xs" onClick={() => setRows((p) => [...p, { k: '', v: '' }])}>
                  <PlusIcon /> Add
                </Button>
              </div>
              {rows.map((r, i) => (
                <div key={i} className="flex items-center gap-1.5">
                  <Input className="h-8 flex-1 font-mono text-xs" placeholder="KEY" value={r.k} onChange={(e) => setRow(i, { k: e.target.value })} />
                  <Input className="h-8 flex-1 font-mono text-xs" type="password" placeholder="value" value={r.v} onChange={(e) => setRow(i, { v: e.target.value })} />
                  <Button variant="ghost" size="icon" className="size-8 shrink-0" onClick={() => setRows((p) => p.filter((_, j) => j !== i))}>
                    <Trash2Icon className="size-4" />
                  </Button>
                </div>
              ))}
              <p className="text-[11px] text-muted-foreground">
                Stored locally on this machine. Keys are env vars the auth plugin reads — e.g.{' '}
                <code>AGENT_QA_PROFILE_ADMIN_EMAIL</code>, <code>AGENT_QA_PROFILE_ADMIN_PASSWORD</code>,
                plus any client id / token your plugin needs.
              </p>
            </div>
          ) : (
            <div className="space-y-2">
              <Label htmlFor="s-cmd">Fetch command</Label>
              <Textarea
                id="s-cmd"
                rows={3}
                className="font-mono text-xs"
                placeholder={`vault kv get -format=json secret/qa/staging | jq -r '.data.data | to_entries[] | "\\(.key)=\\(.value)"'`}
                value={command}
                onChange={(e) => setCommand(e.target.value)}
              />
              <p className="text-[11px] text-muted-foreground">
                Prints JSON <code>{'{KEY:value}'}</code> or <code>KEY=VALUE</code> lines. Runs with the
                server's env (so an ambient vault token works); secrets aren't stored.
              </p>
            </div>
          )}

          <div className="space-y-2">
            <Label htmlFor="s-desc">Notes</Label>
            <Textarea id="s-desc" rows={2} value={description} onChange={(e) => setDescription(e.target.value)} />
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

export default PersonasPage
