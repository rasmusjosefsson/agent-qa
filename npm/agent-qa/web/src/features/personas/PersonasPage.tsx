import { useEffect, useState } from 'react'
import { PlusIcon, Loader2Icon, UsersIcon, Trash2Icon, XIcon } from 'lucide-react'
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
import { getPersonas, upsertPersona, deletePersona } from '@/lib/run-config-api'
import type { PersonaRecord } from './types'

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
  const [err, setErr] = useState('')
  const [editing, setEditing] = useState<PersonaRecord | 'new' | null>(null)

  const load = () =>
    getPersonas()
      .then((r) => setPersonas(r.personas))
      .catch((e) => setErr(String(e.message || e)))
  useEffect(() => {
    void load()
  }, [])

  const remove = async (id: string) => {
    await deletePersona(id).catch((e) => setErr(String(e.message || e)))
    void load()
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-border px-5 py-3">
        <div>
          <h1 className="text-base font-semibold tracking-tight">Personas</h1>
          <p className="text-xs text-muted-foreground">
            A login a test runs as — a profile plus the credentials it signs in with.
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
                A persona is a login: a profile + the credentials it signs in with (typed in, or
                pointed at a vault).
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
                <th className="px-3 py-2 font-medium">Credentials</th>
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
                  <td className="px-3 py-2.5 text-muted-foreground">
                    {Object.keys(p.credentials?.entries || {}).length} keys
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

function PersonaDialog({
  persona,
  onClose,
  onSaved,
}: {
  persona: PersonaRecord | null
  onClose: () => void
  onSaved: () => void
}) {
  const [name, setName] = useState(persona?.name ?? '')
  const [profile, setProfile] = useState(persona?.profile ?? '')
  const [rows, setRows] = useState<{ k: string; v: string }[]>(
    persona ? Object.entries(persona.credentials?.entries ?? {}).map(([k, v]) => ({ k, v })) : [{ k: '', v: '' }]
  )
  const [description, setDescription] = useState(persona?.description ?? '')
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState('')

  const setRow = (i: number, patch: Partial<{ k: string; v: string }>) =>
    setRows((prev) => prev.map((r, j) => (j === i ? { ...r, ...patch } : r)))

  const save = async () => {
    const n = name.trim()
    if (!n) return
    setBusy(true)
    try {
      const id = persona?.id ?? slugify(n)
      const entries: Record<string, string> = {}
      for (const { k, v } of rows) if (k.trim()) entries[k.trim()] = v
      await upsertPersona(id, {
        name: n,
        profile: profile.trim() || id,
        credentials: { entries },
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
          <DialogDescription>A login: a profile + the credentials it signs in with.</DialogDescription>
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
              placeholder={name.trim() ? slugify(name) : 'admin'}
              value={profile}
              onChange={(e) => setProfile(e.target.value)}
            />
            <p className="text-[11px] text-muted-foreground">The session id this login runs under (defaults to the name).</p>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>Credentials</Label>
              <Button variant="ghost" size="sm" className="h-7 px-2 text-xs" onClick={() => setRows((p) => [...p, { k: '', v: '' }])}>
                <PlusIcon /> Add
              </Button>
            </div>
            {rows.map((r, i) => (
              <div key={i} className="flex items-center gap-1.5">
                <Input className="h-8 flex-1 font-mono text-xs" placeholder="ENV_VAR" value={r.k} onChange={(e) => setRow(i, { k: e.target.value })} />
                <Input className="h-8 flex-[1.4] font-mono text-xs" placeholder="value or vault:path:key" value={r.v} onChange={(e) => setRow(i, { v: e.target.value })} />
                <Button variant="ghost" size="icon" className="size-8 shrink-0" onClick={() => setRows((p) => p.filter((_, j) => j !== i))}>
                  <XIcon className="size-4" />
                </Button>
              </div>
            ))}
            <p className="text-[11px] text-muted-foreground">
              Env vars the auth plugin reads. A value can be a literal or{' '}
              <code>vault:&lt;path&gt;:&lt;key&gt;</code> (resolved at run time — run <code>vault login</code> and set{' '}
              <code>VAULT_ADDR</code> first). Stored locally on this machine.
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

export default PersonasPage
