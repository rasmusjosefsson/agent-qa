import { useEffect, useState } from 'react'
import { PlusIcon, Loader2Icon, UsersIcon, Trash2Icon } from 'lucide-react'
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
  const [envPrefix, setEnvPrefix] = useState(persona?.credentials?.envPrefix ?? '')
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
