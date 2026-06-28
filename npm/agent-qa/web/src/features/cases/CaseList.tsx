import { useEffect, useState } from 'react'
import { PlusIcon, Loader2Icon, ClipboardListIcon } from 'lucide-react'
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
import { getCases, upsertCase } from '@/lib/cases-api'
import type { CaseWithScenario } from './types'
import { StatusBadge } from './status'

function slugify(s: string): string {
  return (
    s
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 60) || 'case'
  )
}

function fmtAgo(ts: number | string | null | undefined): string {
  if (!ts) return '—'
  const t = typeof ts === 'string' ? Date.parse(ts) : ts
  if (!Number.isFinite(t)) return '—'
  const s = Math.max(0, Math.round((Date.now() - t) / 1000))
  if (s < 60) return `${s}s ago`
  const m = Math.round(s / 60)
  if (m < 60) return `${m}m ago`
  const h = Math.round(m / 60)
  if (h < 24) return `${h}h ago`
  return `${Math.round(h / 24)}d ago`
}

const gotoCase = (id: string) => {
  window.location.href = `/cases?id=${encodeURIComponent(id)}`
}

export function CaseList() {
  const [cases, setCases] = useState<CaseWithScenario[] | null>(null)
  const [err, setErr] = useState('')
  const [open, setOpen] = useState(false)
  const [title, setTitle] = useState('')
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    getCases()
      .then((r) => setCases(r.cases))
      .catch((e) => setErr(String(e.message || e)))
  }, [])

  const create = async () => {
    const t = title.trim()
    if (!t) return
    setBusy(true)
    try {
      const id = slugify(t)
      await upsertCase(id, { title: t, source: 'manual' })
      gotoCase(id)
    } catch (e) {
      setErr(String((e as Error).message || e))
      setBusy(false)
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-border px-5 py-3">
        <div>
          <h1 className="text-base font-semibold tracking-tight">Test Cases</h1>
          <p className="text-xs text-muted-foreground">
            Author a test in plain English, then let the agent record it into a replayable scenario.
          </p>
        </div>
        <Button size="sm" onClick={() => setOpen(true)}>
          <PlusIcon /> New case
        </Button>
      </div>

      {err && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-5 py-1.5 text-xs text-destructive">
          {err}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        {cases === null ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading cases…
          </div>
        ) : cases.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <ClipboardListIcon className="size-8 text-muted-foreground/50" />
            <div>
              <div className="text-sm font-medium">No test cases yet</div>
              <p className="mx-auto mt-1 max-w-sm text-xs text-muted-foreground">
                Create one from a plain-text test plan (numbered steps + an expected result), or
                import from a source.
              </p>
            </div>
            <Button size="sm" onClick={() => setOpen(true)}>
              <PlusIcon /> New case
            </Button>
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 z-10 bg-background">
              <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                <th className="px-5 py-2 font-medium">Title</th>
                <th className="px-3 py-2 font-medium">Steps</th>
                <th className="px-3 py-2 font-medium">Status</th>
                <th className="px-3 py-2 font-medium">Last run</th>
                <th className="px-3 py-2 font-medium">Source</th>
              </tr>
            </thead>
            <tbody>
              {cases.map((c) => (
                <tr
                  key={c.id}
                  onClick={() => gotoCase(c.id)}
                  className="cursor-pointer border-b border-border/60 transition-colors hover:bg-muted/40"
                >
                  <td className="px-5 py-2.5">
                    <div className="font-medium text-foreground">{c.title}</div>
                    <div className="mt-0.5 flex items-center gap-2">
                      <span className="font-mono text-[11px] text-muted-foreground">{c.id}</span>
                      {c.tags.slice(0, 3).map((t) => (
                        <span
                          key={t}
                          className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground"
                        >
                          {t}
                        </span>
                      ))}
                    </div>
                  </td>
                  <td className="px-3 py-2.5 text-muted-foreground">{c.steps.length}</td>
                  <td className="px-3 py-2.5">
                    <StatusBadge scenario={c.scenario} />
                  </td>
                  <td className="px-3 py-2.5 text-muted-foreground">
                    {fmtAgo(c.scenario?.latestRun?.finishedAt ?? null)}
                  </td>
                  <td className="px-3 py-2.5 text-muted-foreground capitalize">{c.source}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>New test case</DialogTitle>
            <DialogDescription>
              Give it a title — you'll add the steps and expected result next.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="case-title">Title</Label>
            <Input
              id="case-title"
              autoFocus
              placeholder="e.g. User can log in with valid credentials"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void create()
              }}
            />
            {title.trim() && (
              <p className="font-mono text-[11px] text-muted-foreground">id: {slugify(title)}</p>
            )}
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button onClick={() => void create()} disabled={!title.trim() || busy}>
              {busy && <Loader2Icon className="animate-spin" />} Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
