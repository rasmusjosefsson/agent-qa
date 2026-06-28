import { useEffect, useState } from 'react'
import { PlusIcon, Loader2Icon, FolderTreeIcon } from 'lucide-react'
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
import { getPlans, upsertPlan } from '@/lib/plans-api'
import type { PlanWithCount } from './types'

function slugify(s: string): string {
  return (
    s
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 60) || 'plan'
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

const gotoPlan = (id: string) => {
  window.location.href = `/plans?id=${encodeURIComponent(id)}`
}

export function PlanList() {
  const [plans, setPlans] = useState<PlanWithCount[] | null>(null)
  const [err, setErr] = useState('')
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    getPlans()
      .then((r) => setPlans(r.plans))
      .catch((e) => setErr(String(e.message || e)))
  }, [])

  const create = async () => {
    const n = name.trim()
    if (!n) return
    setBusy(true)
    try {
      const id = slugify(n)
      await upsertPlan(id, { name: n })
      gotoPlan(id)
    } catch (e) {
      setErr(String((e as Error).message || e))
      setBusy(false)
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-border px-5 py-3">
        <div>
          <h1 className="text-base font-semibold tracking-tight">Test Plans</h1>
          <p className="text-xs text-muted-foreground">
            Pick a scope of sets and cases, run it, and track each case's result.
          </p>
        </div>
        <Button size="sm" onClick={() => setOpen(true)}>
          <PlusIcon /> New plan
        </Button>
      </div>

      {err && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-5 py-1.5 text-xs text-destructive">
          {err}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        {plans === null ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading plans…
          </div>
        ) : plans.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <FolderTreeIcon className="size-8 text-muted-foreground/50" />
            <div>
              <div className="text-sm font-medium">No test plans yet</div>
              <p className="mx-auto mt-1 max-w-sm text-xs text-muted-foreground">
                A plan is a runnable scope — assemble it from sets and individual cases, then run
                them together.
              </p>
            </div>
            <Button size="sm" onClick={() => setOpen(true)}>
              <PlusIcon /> New plan
            </Button>
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 z-10 bg-background">
              <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                <th className="px-5 py-2 font-medium">Name</th>
                <th className="px-3 py-2 font-medium">Scope</th>
                <th className="px-3 py-2 font-medium">Cases</th>
                <th className="px-3 py-2 font-medium">Updated</th>
              </tr>
            </thead>
            <tbody>
              {plans.map((p) => (
                <tr
                  key={p.id}
                  onClick={() => gotoPlan(p.id)}
                  className="cursor-pointer border-b border-border/60 transition-colors hover:bg-muted/40"
                >
                  <td className="px-5 py-2.5">
                    <div className="font-medium text-foreground">{p.name}</div>
                    <div className="mt-0.5 font-mono text-[11px] text-muted-foreground">{p.id}</div>
                  </td>
                  <td className="px-3 py-2.5 text-muted-foreground">
                    {p.scope.setIds.length} {p.scope.setIds.length === 1 ? 'set' : 'sets'} ·{' '}
                    {p.scope.caseIds.length} direct
                  </td>
                  <td className="px-3 py-2.5 text-muted-foreground">{p.caseCount}</td>
                  <td className="px-3 py-2.5 text-muted-foreground">{fmtAgo(p.updatedAt)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>New test plan</DialogTitle>
            <DialogDescription>Name it — you'll pick its scope next.</DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="plan-name">Name</Label>
            <Input
              id="plan-name"
              autoFocus
              placeholder="e.g. Release regression"
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
