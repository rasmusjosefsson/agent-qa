import { useEffect, useMemo, useState } from 'react'
import {
  ArrowLeftIcon,
  Loader2Icon,
  SaveIcon,
  Trash2Icon,
  XIcon,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { getCases } from '@/lib/cases-api'
import { deleteSet, getSet, upsertSet } from '@/lib/sets-api'
import type { CaseWithScenario } from '@/features/cases/types'
import type { SetMode, SetRecord } from './types'

const back = () => {
  window.location.href = '/sets'
}

export function SetDetail({ id }: { id: string }) {
  const [set, setSet] = useState<SetRecord | null>(null)
  const [cases, setCases] = useState<CaseWithScenario[]>([])
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const [confirmDel, setConfirmDel] = useState(false)

  // Editable fields.
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [mode, setMode] = useState<SetMode>('manual')
  const [caseIds, setCaseIds] = useState<string[]>([])
  const [tagQuery, setTagQuery] = useState<string[]>([])
  const [tagDraft, setTagDraft] = useState('')

  useEffect(() => {
    Promise.all([getSet(id), getCases()])
      .then(([s, c]) => {
        setSet(s.set)
        setCases(c.cases)
        setName(s.set.name)
        setDescription(s.set.description)
        setMode(s.set.mode)
        setCaseIds(s.set.caseIds)
        setTagQuery(s.set.tagQuery)
      })
      .catch((e) => setErr(String(e.message || e)))
  }, [id])

  // All labels in use across cases, for quick add in tag mode.
  const allTags = useMemo(() => {
    const t = new Set<string>()
    for (const c of cases) for (const tag of c.tags) t.add(tag)
    return [...t].sort()
  }, [cases])

  // Live preview of resolved members (mirrors the server's resolveSetCaseIds).
  const members = useMemo(() => {
    if (mode === 'tag') {
      if (tagQuery.length === 0) return []
      const want = new Set(tagQuery)
      return cases.filter((c) => c.tags.some((t) => want.has(t)))
    }
    const want = new Set(caseIds)
    return cases.filter((c) => want.has(c.id))
  }, [mode, cases, caseIds, tagQuery])

  const toggleCase = (cid: string) =>
    setCaseIds((prev) => (prev.includes(cid) ? prev.filter((x) => x !== cid) : [...prev, cid]))

  const addTag = (raw: string) => {
    const t = raw.trim()
    if (t && !tagQuery.includes(t)) setTagQuery((prev) => [...prev, t])
    setTagDraft('')
  }
  const removeTag = (t: string) => setTagQuery((prev) => prev.filter((x) => x !== t))

  const save = async () => {
    setBusy(true)
    setErr('')
    try {
      await upsertSet(id, { name: name.trim() || id, description, mode, caseIds, tagQuery })
      back()
    } catch (e) {
      setErr(String((e as Error).message || e))
      setBusy(false)
    }
  }

  const remove = async () => {
    setBusy(true)
    try {
      await deleteSet(id)
      back()
    } catch (e) {
      setErr(String((e as Error).message || e))
      setBusy(false)
    }
  }

  if (err && !set) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-sm text-destructive">
        {err}
        <Button variant="ghost" size="sm" onClick={back}>
          <ArrowLeftIcon /> Back to sets
        </Button>
      </div>
    )
  }

  if (!set) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading set…
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-3 border-b border-border px-5 py-3">
        <Button variant="ghost" size="icon" className="size-7" onClick={back} title="Back to sets">
          <ArrowLeftIcon className="size-4" />
        </Button>
        <div className="min-w-0 flex-1">
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="h-8 border-transparent bg-transparent px-1 text-base font-semibold shadow-none focus-visible:border-border"
          />
          <div className="px-1 font-mono text-[11px] text-muted-foreground">
            {set.id} · {members.length} {members.length === 1 ? 'case' : 'cases'}
          </div>
        </div>
        <Button variant="ghost" size="sm" onClick={() => setConfirmDel(true)} disabled={busy}>
          <Trash2Icon /> Delete
        </Button>
        <Button size="sm" onClick={() => void save()} disabled={busy}>
          {busy ? <Loader2Icon className="animate-spin" /> : <SaveIcon />} Save
        </Button>
      </div>

      {err && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-5 py-1.5 text-xs text-destructive">
          {err}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        <div className="mx-auto max-w-3xl space-y-6 p-5">
          <div className="space-y-2">
            <Label htmlFor="set-desc">Description</Label>
            <Textarea
              id="set-desc"
              rows={2}
              placeholder="What is this set for?"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
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
                    mode === m ? 'border-primary bg-primary/10' : 'border-border hover:bg-muted/40'
                  }`}
                >
                  <div className="font-medium">{m === 'tag' ? 'By label' : 'Manual'}</div>
                  <div className="mt-0.5 text-muted-foreground">
                    {m === 'tag'
                      ? 'Any case carrying one of these labels'
                      : 'Cases you pick by hand'}
                  </div>
                </button>
              ))}
            </div>
          </div>

          {mode === 'tag' ? (
            <div className="space-y-2">
              <Label>Labels</Label>
              <div className="flex flex-wrap items-center gap-1.5 rounded-md border border-border p-2">
                {tagQuery.map((t) => (
                  <span
                    key={t}
                    className="inline-flex items-center gap-1 rounded bg-muted px-1.5 py-0.5 text-xs"
                  >
                    {t}
                    <button type="button" onClick={() => removeTag(t)} className="opacity-60 hover:opacity-100">
                      <XIcon className="size-3" />
                    </button>
                  </span>
                ))}
                <input
                  value={tagDraft}
                  onChange={(e) => setTagDraft(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ',') {
                      e.preventDefault()
                      addTag(tagDraft)
                    }
                  }}
                  placeholder={tagQuery.length ? '' : 'Type a label, Enter to add'}
                  className="min-w-[8rem] flex-1 bg-transparent text-sm outline-none"
                />
              </div>
              {allTags.length > 0 && (
                <div className="flex flex-wrap gap-1.5 pt-1">
                  {allTags
                    .filter((t) => !tagQuery.includes(t))
                    .map((t) => (
                      <button
                        key={t}
                        type="button"
                        onClick={() => addTag(t)}
                        className="rounded border border-border px-1.5 py-0.5 text-[11px] text-muted-foreground hover:bg-muted/40"
                      >
                        + {t}
                      </button>
                    ))}
                </div>
              )}
            </div>
          ) : null}

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>{mode === 'tag' ? 'Matching cases' : 'Cases'}</Label>
              <span className="text-xs text-muted-foreground">{members.length} selected</span>
            </div>
            {cases.length === 0 ? (
              <p className="text-xs text-muted-foreground">No cases yet — create some first.</p>
            ) : mode === 'tag' ? (
              <div className="divide-y divide-border/60 rounded-md border border-border">
                {members.length === 0 ? (
                  <div className="px-3 py-4 text-center text-xs text-muted-foreground">
                    No cases match these labels yet.
                  </div>
                ) : (
                  members.map((c) => <CaseRow key={c.id} c={c} />)
                )}
              </div>
            ) : (
              <div className="divide-y divide-border/60 rounded-md border border-border">
                {cases.map((c) => (
                  <label
                    key={c.id}
                    className="flex cursor-pointer items-center gap-3 px-3 py-2 hover:bg-muted/40"
                  >
                    <Checkbox
                      checked={caseIds.includes(c.id)}
                      onCheckedChange={() => toggleCase(c.id)}
                    />
                    <CaseRow c={c} bare />
                  </label>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      <Dialog open={confirmDel} onOpenChange={setConfirmDel}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete this set?</DialogTitle>
            <DialogDescription>
              This removes the set “{set.name}”. The cases it references are not deleted.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setConfirmDel(false)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={() => void remove()} disabled={busy}>
              {busy && <Loader2Icon className="animate-spin" />} Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

function CaseRow({ c, bare = false }: { c: CaseWithScenario; bare?: boolean }) {
  return (
    <div className={bare ? 'min-w-0 flex-1' : 'px-3 py-2'}>
      <div className="truncate text-sm font-medium text-foreground">{c.title}</div>
      <div className="mt-0.5 flex items-center gap-2">
        <span className="font-mono text-[11px] text-muted-foreground">{c.id}</span>
        {c.tags.slice(0, 3).map((t) => (
          <span key={t} className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
            {t}
          </span>
        ))}
      </div>
    </div>
  )
}
