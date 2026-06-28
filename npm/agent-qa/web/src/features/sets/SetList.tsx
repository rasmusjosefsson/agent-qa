import { useEffect, useState } from 'react'
import { PlusIcon, Loader2Icon, LayersIcon } from 'lucide-react'
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
import { getSets, upsertSet } from '@/lib/sets-api'
import type { SetMode, SetWithCount } from './types'

function slugify(s: string): string {
  return (
    s
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 60) || 'set'
  )
}

function fmtAgo(ts: number | null | undefined): string {
  if (!ts) return '—'
  const s = Math.max(0, Math.round((Date.now() - ts) / 1000))
  if (s < 60) return `${s}s ago`
  const m = Math.round(s / 60)
  if (m < 60) return `${m}m ago`
  const h = Math.round(m / 60)
  if (h < 24) return `${h}h ago`
  return `${Math.round(h / 24)}d ago`
}

const gotoSet = (id: string) => {
  window.location.href = `/sets?id=${encodeURIComponent(id)}`
}

export function SetList() {
  const [sets, setSets] = useState<SetWithCount[] | null>(null)
  const [err, setErr] = useState('')
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  const [mode, setMode] = useState<SetMode>('manual')
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    getSets()
      .then((r) => setSets(r.sets))
      .catch((e) => setErr(String(e.message || e)))
  }, [])

  const create = async () => {
    const n = name.trim()
    if (!n) return
    setBusy(true)
    try {
      const id = slugify(n)
      await upsertSet(id, { name: n, mode })
      gotoSet(id)
    } catch (e) {
      setErr(String((e as Error).message || e))
      setBusy(false)
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-border px-5 py-3">
        <div>
          <h1 className="text-base font-semibold tracking-tight">Test Sets</h1>
          <p className="text-xs text-muted-foreground">
            Curate a reusable collection of cases — by hand, or by matching a label.
          </p>
        </div>
        <Button size="sm" onClick={() => setOpen(true)}>
          <PlusIcon /> New set
        </Button>
      </div>

      {err && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-5 py-1.5 text-xs text-destructive">
          {err}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        {sets === null ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading sets…
          </div>
        ) : sets.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <LayersIcon className="size-8 text-muted-foreground/50" />
            <div>
              <div className="text-sm font-medium">No test sets yet</div>
              <p className="mx-auto mt-1 max-w-sm text-xs text-muted-foreground">
                Group cases into a reusable set — pick them by hand, or match a label so it stays
                up to date as cases are added.
              </p>
            </div>
            <Button size="sm" onClick={() => setOpen(true)}>
              <PlusIcon /> New set
            </Button>
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 z-10 bg-background">
              <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                <th className="px-5 py-2 font-medium">Name</th>
                <th className="px-3 py-2 font-medium">Membership</th>
                <th className="px-3 py-2 font-medium">Cases</th>
                <th className="px-3 py-2 font-medium">Updated</th>
              </tr>
            </thead>
            <tbody>
              {sets.map((s) => (
                <tr
                  key={s.id}
                  onClick={() => gotoSet(s.id)}
                  className="cursor-pointer border-b border-border/60 transition-colors hover:bg-muted/40"
                >
                  <td className="px-5 py-2.5">
                    <div className="font-medium text-foreground">{s.name}</div>
                    <div className="mt-0.5 font-mono text-[11px] text-muted-foreground">{s.id}</div>
                  </td>
                  <td className="px-3 py-2.5 text-muted-foreground">
                    {s.mode === 'tag' ? (
                      <span>
                        label:{' '}
                        {s.tagQuery.length ? s.tagQuery.join(', ') : <span className="italic">none</span>}
                      </span>
                    ) : (
                      'manual'
                    )}
                  </td>
                  <td className="px-3 py-2.5 text-muted-foreground">{s.caseCount}</td>
                  <td className="px-3 py-2.5 text-muted-foreground">{fmtAgo(s.updatedAt)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>New test set</DialogTitle>
            <DialogDescription>
              Name it and choose how members are picked — you can change this later.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-2">
              <Label htmlFor="set-name">Name</Label>
              <Input
                id="set-name"
                autoFocus
                placeholder="e.g. Smoke tests"
                value={name}
                onChange={(e) => setName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') void create()
                }}
              />
              {name.trim() && (
                <p className="font-mono text-[11px] text-muted-foreground">id: {slugify(name)}</p>
              )}
            </div>
            <div className="space-y-2">
              <Label>Membership</Label>
              <div className="grid grid-cols-2 gap-2">
                {(['manual', 'tag'] as const).map((m) => (
                  <button
                    key={m}
                    type="button"
                    onClick={() => setMode(m)}
                    className={`rounded-md border px-3 py-2 text-left text-xs transition-colors ${
                      mode === m
                        ? 'border-primary bg-primary/10'
                        : 'border-border hover:bg-muted/40'
                    }`}
                  >
                    <div className="font-medium capitalize">{m === 'tag' ? 'By label' : 'Manual'}</div>
                    <div className="mt-0.5 text-muted-foreground">
                      {m === 'tag' ? 'Auto-match cases by label' : 'Pick cases by hand'}
                    </div>
                  </button>
                ))}
              </div>
            </div>
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button onClick={() => void create()} disabled={!name.trim() || busy}>
              {busy && <Loader2Icon className="animate-spin" />} Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
