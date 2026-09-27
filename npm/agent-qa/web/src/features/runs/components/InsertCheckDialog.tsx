// web/src/features/runs/components/InsertCheckDialog.tsx
//
// "Add check after this step" — while reviewing a run, insert an assertion
// right after the step that established the state worth checking. Writes the
// saved scenario.json via `scenario insert` (the CLI re-validates, so a bad
// draft comes back as an error rather than a broken file).

import { useEffect, useState } from 'react'
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { insertStep } from '@/lib/runs-api'

const PREDICATES = [
  'isVisible',
  'exists',
  'notExists',
  'isHidden',
  'equals',
  'contains',
  'matches',
  'startsWith',
  'endsWith',
] as const

const VALUE_PREDICATES = new Set([
  'equals',
  'contains',
  'matches',
  'startsWith',
  'endsWith',
])

// `role=name`-style shorthand? No — keep it explicit: role + optional name
// fields produce `{role, name}`; "css selector" produces the raw escape hatch.
function elementSubject(role: string, name: string, css: string, useCss: boolean) {
  if (useCss) {
    return { element: { raw: { kind: 'css', value: css }, reason: 'assertion added from the Runs pane' } }
  }
  const loc: Record<string, unknown> = { role }
  if (name.trim()) loc.name = name.trim()
  return { element: loc }
}

export function InsertCheckDialog({
  open,
  onOpenChange,
  sid,
  afterStepId,
  afterLabel,
  onDone,
}: {
  open: boolean
  onOpenChange: (v: boolean) => void
  sid: string
  afterStepId: string
  afterLabel: string
  onDone: () => void
}) {
  const [intent, setIntent] = useState('')
  const [subjectKind, setSubjectKind] = useState<'element' | 'url'>('element')
  const [useCss, setUseCss] = useState(false)
  const [role, setRole] = useState('')
  const [name, setName] = useState('')
  const [css, setCss] = useState('')
  const [predicate, setPredicate] = useState<string>('isVisible')
  const [value, setValue] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (open) {
      setIntent(`check after ${afterStepId}`)
      setError(null)
    }
  }, [open, afterStepId])

  const valid =
    !!intent.trim() &&
    (subjectKind === 'url' ||
      (useCss ? !!css.trim() : !!role.trim()))

  const submit = async () => {
    if (!valid || busy) return
    setBusy(true)
    setError(null)
    const claim: Record<string, unknown> = {
      subject: subjectKind === 'url' ? { url: true } : elementSubject(role, name, css, useCss),
      predicate,
    }
    if (VALUE_PREDICATES.has(predicate as never) && value.trim()) claim.value = value.trim()
    const r = await insertStep(sid, 'check', { intent: intent.trim(), claim }, { after: afterStepId })
    setBusy(false)
    if (!r.ok) {
      setError(r.error || 'insert failed')
      return
    }
    onOpenChange(false)
    onDone()
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add check after this step</DialogTitle>
          <DialogDescription>
            Inserts a check step into the saved scenario right after {afterLabel}.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-2">
            <Label htmlFor="ins-intent">Intent</Label>
            <Input
              id="ins-intent"
              autoFocus
              value={intent}
              onChange={(e) => setIntent(e.target.value)}
              placeholder="what this assertion proves…"
            />
          </div>
          <div className="flex gap-3">
            <div className="w-36 space-y-2">
              <Label>Subject</Label>
              <Select value={subjectKind} onValueChange={(v) => setSubjectKind(v as 'element' | 'url')}>
                <SelectTrigger size="sm" className="h-8 text-xs" aria-label="Subject">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="element">element</SelectItem>
                  <SelectItem value="url">url</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="flex-1 space-y-2">
              <Label>Predicate</Label>
              <Select value={predicate} onValueChange={setPredicate}>
                <SelectTrigger size="sm" className="h-8 text-xs" aria-label="Predicate">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PREDICATES.map((p) => (
                    <SelectItem key={p} value={p}>
                      {p}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          {subjectKind === 'element' && (
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Label>Locator</Label>
                <button
                  type="button"
                  className="text-[11px] text-muted-foreground underline-offset-2 hover:underline"
                  onClick={() => setUseCss(!useCss)}
                >
                  {useCss ? 'use role + name instead' : 'use a css selector instead'}
                </button>
              </div>
              {useCss ? (
                <Input
                  value={css}
                  onChange={(e) => setCss(e.target.value)}
                  placeholder="css selector, e.g. button[type=submit]"
                />
              ) : (
                <div className="flex gap-2">
                  <Input value={role} onChange={(e) => setRole(e.target.value)} placeholder="role (button)" />
                  <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="name (optional)" />
                </div>
              )}
            </div>
          )}
          {VALUE_PREDICATES.has(predicate as never) && (
            <div className="space-y-2">
              <Label htmlFor="ins-value">Expected value</Label>
              <Input
                id="ins-value"
                value={value}
                onChange={(e) => setValue(e.target.value)}
                placeholder="text / number / pattern"
              />
            </div>
          )}
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button disabled={!valid || busy} onClick={() => void submit()}>
            {busy ? 'Inserting…' : 'Insert check'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export default InsertCheckDialog
