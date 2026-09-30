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

type SubjectKind = 'element' | 'url' | 'shot' | 'domshot' | 'console' | 'network' | 'dialog'
type SubjectKind = 'element' | 'elementCount' | 'url' | 'shot' | 'console' | 'network' | 'dialog'

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

const COUNT_PREDICATES = ['countEquals', 'equals', 'gt', 'gte', 'lt', 'lte'] as const

// Predicates that make sense per subject — the backend re-validates anyway,
// but offering `matches` on a url claim would just bounce.
const PREDICATES_BY_KIND: Record<SubjectKind, readonly string[]> = {
  element: PREDICATES,
  elementCount: COUNT_PREDICATES,
  url: PREDICATES,
  dialog: ['exists', 'notExists', 'equals', 'contains', 'matches'],
  shot: ['matches'],
  domshot: ['matches'],
  console: ['exists', 'notExists', 'equals', 'contains', 'countEquals'],
  network: ['exists', 'notExists', 'equals', 'contains'],
}

const VALUE_PREDICATES = new Set([
  'equals',
  'contains',
  'matches',
  'startsWith',
  'endsWith',
  'countEquals',
  'gt',
  'gte',
  'lt',
  'lte',
])

const NETWORK_KINDS = ['fired', 'status', 'responseJsonPath'] as const
const METHODS = ['any', 'GET', 'POST', 'PUT', 'PATCH', 'DELETE'] as const
const CONSOLE_TYPES = ['error', 'warning', 'log', 'any'] as const

// `role=name`-style shorthand? No — keep it explicit: role + optional name
// fields produce `{role, name}`; "css selector" produces the raw escape hatch.
function elementSubject(role: string, name: string, css: string, useCss: boolean) {
  if (useCss) {
    return { element: { raw: { kind: 'css', value: css.trim() }, reason: 'assertion added from the Runs pane' } }
  }
  const loc: Record<string, unknown> = { role }
  if (name.trim()) loc.name = name.trim()
  return { element: loc }
}

// Everything the claim JSON needs, as plain strings — assembled outside the
// component so the shape is unit-testable.
export interface CheckDraft {
  subjectKind: SubjectKind
  role: string
  name: string
  css: string
  useCss: boolean
  shotStep: string
  shotTolerance: string
  consoleType: string
  consoleText: string
  netUrl: string
  netMethod: string
  netKind: string
  netPath: string
  predicate: string
  value: string
}

export function buildCheckClaim(d: CheckDraft): Record<string, unknown> {
  let subject: Record<string, unknown>
  switch (d.subjectKind) {
    case 'url':
      subject = { url: true }
      break
    case 'dialog':
      subject = { dialog: true }
      break
    case 'shot':
      subject = { shot: d.shotStep.trim() }
      break
    case 'console': {
      const m: Record<string, unknown> = {}
      if (d.consoleType && d.consoleType !== 'any') m.type = d.consoleType
      if (d.consoleText.trim()) m.text = d.consoleText.trim()
      subject = { console: Object.keys(m).length ? m : true }
      break
    }
    case 'network': {
      const m: Record<string, unknown> = {}
      if (d.netUrl.trim()) m.urlMatches = d.netUrl.trim()
      if (d.netMethod && d.netMethod !== 'any') m.method = d.netMethod
      const sub: Record<string, unknown> = { network: m }
      if (d.netKind !== 'fired') sub.ofKind = d.netKind
      if (d.netKind === 'responseJsonPath' && d.netPath.trim()) sub.path = d.netPath.trim()
      subject = sub
      break
    }
    case 'elementCount':
      // `ofKind: "count"` evaluates querySelectorAll(css).length — counting
      // semantic locators (role/text) would need a snapshot query, so the
      // count claim is raw-css only.
      subject = {
        element: { raw: { kind: 'css', value: d.css.trim() }, reason: 'assertion added from the Runs pane' },
        ofKind: 'count',
      }
      break
    default:
      subject = elementSubject(d.role, d.name, d.css, d.useCss)
  }
  const claim: Record<string, unknown> = { subject, predicate: d.predicate }
  if (d.subjectKind === 'elementCount' && d.value.trim()) {
    // count claims compare against a JSON number, not a string
    claim.value = Number(d.value.trim())
  } else if (VALUE_PREDICATES.has(d.predicate) && d.value.trim()) {
    claim.value = d.value.trim()
  }
  if (d.subjectKind === 'shot' && d.shotTolerance.trim()) {
    const px = Number(d.shotTolerance)
    if (!Number.isNaN(px)) claim.tolerance = { pixels: px }
  }
  return claim
}

export function checkDraftValid(d: CheckDraft): boolean {
  switch (d.subjectKind) {
    case 'url':
    case 'dialog':
    case 'console':
      return true
    case 'shot':
      return !!d.shotStep.trim()
    case 'network':
      return (
        (!!d.netUrl.trim() || (!!d.netMethod && d.netMethod !== 'any')) &&
        (d.netKind !== 'responseJsonPath' || !!d.netPath.trim())
      )
    case 'elementCount':
      return !!d.css.trim() && d.value.trim() !== '' && !Number.isNaN(Number(d.value.trim()))
    default:
      return d.useCss ? !!d.css.trim() : !!d.role.trim()
  }
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
  const [subjectKind, setSubjectKind] = useState<SubjectKind>('element')
  const [useCss, setUseCss] = useState(false)
  const [role, setRole] = useState('')
  const [name, setName] = useState('')
  const [css, setCss] = useState('')
  const [shotStep, setShotStep] = useState('')
  const [shotTolerance, setShotTolerance] = useState('')
  const [domshotStep, setDomshotStep] = useState('')
  const [domshotSkip, setDomshotSkip] = useState('')
  const [consoleType, setConsoleType] = useState('error')
  const [consoleText, setConsoleText] = useState('')
  const [netUrl, setNetUrl] = useState('')
  const [netMethod, setNetMethod] = useState('')
  const [netKind, setNetKind] = useState<string>('fired')
  const [netPath, setNetPath] = useState('')
  const [predicate, setPredicate] = useState<string>('isVisible')
  const [value, setValue] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (open) {
      setIntent(`check after ${afterStepId}`)
      setShotStep(afterStepId)
      setDomshotStep(afterStepId)
      setError(null)
    }
  }, [open, afterStepId])

  const pickKind = (v: SubjectKind) => {
    setSubjectKind(v)
    const defaults: Record<SubjectKind, string> = {
      element: 'isVisible',
      elementCount: 'countEquals',
      url: 'contains',
      shot: 'matches',
      domshot: 'matches',
      console: 'notExists',
      network: 'exists',
      dialog: 'exists',
    }
    setPredicate(defaults[v])
  }

  const subject = (): Record<string, unknown> => {
    switch (subjectKind) {
      case 'url':
        return { url: true }
      case 'dialog':
        return { dialog: true }
      case 'shot':
        return { shot: shotStep.trim() }
      case 'domshot': {
        const sub: Record<string, unknown> = { domshot: domshotStep.trim() }
        const skip = domshotSkip
          .split(',')
          .map((s) => s.trim())
          .filter(Boolean)
        if (skip.length) sub.skip = skip
        return sub
      }
      case 'console': {
        const m: Record<string, unknown> = {}
        if (consoleType && consoleType !== 'any') m.type = consoleType
        if (consoleText.trim()) m.text = consoleText.trim()
        return { console: Object.keys(m).length ? m : true }
      }
      case 'network': {
        const m: Record<string, unknown> = {}
        if (netUrl.trim()) m.urlMatches = netUrl.trim()
        if (netMethod && netMethod !== 'any') m.method = netMethod
        const sub: Record<string, unknown> = { network: m }
        if (netKind !== 'fired') sub.ofKind = netKind
        if (netKind === 'responseJsonPath' && netPath.trim()) sub.path = netPath.trim()
        return sub
      }
      default:
        return elementSubject(role, name, css, useCss)
    }
  const draft: CheckDraft = {
    subjectKind,
    role,
    name,
    css,
    useCss,
    shotStep,
    shotTolerance,
    consoleType,
    consoleText,
    netUrl,
    netMethod,
    netKind,
    netPath,
    predicate,
    value,
  }

  const valid =
    !!intent.trim() &&
    (subjectKind === 'url' ||
      subjectKind === 'dialog' ||
      (subjectKind === 'shot' && !!shotStep.trim()) ||
      (subjectKind === 'domshot' && !!domshotStep.trim()) ||
      subjectKind === 'console' ||
      (subjectKind === 'network' &&
        (!!netUrl.trim() || (!!netMethod && netMethod !== 'any')) &&
        (netKind !== 'responseJsonPath' || !!netPath.trim())) ||
      (subjectKind === 'element' && (useCss ? !!css.trim() : !!role.trim())))
  const valid = !!intent.trim() && checkDraftValid(draft)

  const submit = async () => {
    if (!valid || busy) return
    setBusy(true)
    setError(null)
    const claim = buildCheckClaim(draft)
    const r = await insertStep(sid, 'check', { intent: intent.trim(), claim }, { after: afterStepId })
    setBusy(false)
    if (!r.ok) {
      setError(r.error || 'insert failed')
      return
    }
    onOpenChange(false)
    onDone()
  }

  const predicates = PREDICATES_BY_KIND[subjectKind]

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
              <Select value={subjectKind} onValueChange={(v) => pickKind(v as SubjectKind)}>
                <SelectTrigger size="sm" className="h-8 text-xs" aria-label="Subject">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="element">element</SelectItem>
                  <SelectItem value="elementCount">element count</SelectItem>
                  <SelectItem value="url">url</SelectItem>
                  <SelectItem value="shot">screenshot</SelectItem>
                  <SelectItem value="domshot">dom snapshot</SelectItem>
                  <SelectItem value="console">console</SelectItem>
                  <SelectItem value="network">network</SelectItem>
                  <SelectItem value="dialog">dialog</SelectItem>
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
                  {predicates.map((p) => (
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
          {subjectKind === 'elementCount' && (
            <div className="space-y-2">
              <Label>Locator</Label>
              <Input
                value={css}
                onChange={(e) => setCss(e.target.value)}
                placeholder="css selector, e.g. .athing.submission"
              />
              <p className="text-[11px] text-muted-foreground">
                Counts every node matching the selector; expected value is a number.
              </p>
            </div>
          )}
          {subjectKind === 'shot' && (
            <div className="space-y-2">
              <Label>Screenshot step</Label>
              <div className="flex gap-2">
                <Input
                  value={shotStep}
                  onChange={(e) => setShotStep(e.target.value)}
                  placeholder="step id whose screenshot to compare"
                />
                <Input
                  className="w-28"
                  value={shotTolerance}
                  onChange={(e) => setShotTolerance(e.target.value)}
                  placeholder="tolerance 0.01"
                />
              </div>
              <p className="text-[11px] text-muted-foreground">
                Pixel-diffs that step's screenshot vs baselines/&lt;id&gt;.png (mint with shot-accept).
              </p>
            </div>
          )}
          {subjectKind === 'domshot' && (
            <div className="space-y-2">
              <Label>Snapshot step</Label>
              <div className="flex gap-2">
                <Input
                  value={domshotStep}
                  onChange={(e) => setDomshotStep(e.target.value)}
                  placeholder="step id whose ARIA snapshot to compare"
                />
                <Input
                  className="w-40"
                  value={domshotSkip}
                  onChange={(e) => setDomshotSkip(e.target.value)}
                  placeholder="skip regexes, comma-sep"
                />
              </div>
              <p className="text-[11px] text-muted-foreground">
                Text-diffs that step's ARIA snapshot vs baselines/&lt;id&gt;.snap.txt (mint with domshot-accept).
              </p>
            </div>
          )}
          {subjectKind === 'console' && (
            <div className="space-y-2">
              <Label>Matcher</Label>
              <div className="flex gap-2">
                <Select value={consoleType} onValueChange={setConsoleType}>
                  <SelectTrigger size="sm" className="h-8 w-28 text-xs" aria-label="Console level">
                    <SelectValue placeholder="any level" />
                  </SelectTrigger>
                  <SelectContent>
                    {CONSOLE_TYPES.map((t) => (
                      <SelectItem key={t} value={t}>
                        {t === 'any' ? 'any level' : t}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Input
                  value={consoleText}
                  onChange={(e) => setConsoleText(e.target.value)}
                  placeholder="text contains… (optional)"
                />
              </div>
            </div>
          )}
          {subjectKind === 'network' && (
            <div className="space-y-2">
              <Label>Matcher</Label>
              <div className="flex gap-2">
                <Input
                  value={netUrl}
                  onChange={(e) => setNetUrl(e.target.value)}
                  placeholder="url matches, e.g. /api/users"
                />
                <Select value={netMethod || 'any'} onValueChange={setNetMethod}>
                  <SelectTrigger size="sm" className="h-8 w-24 text-xs" aria-label="Method">
                    <SelectValue placeholder="method" />
                  </SelectTrigger>
                  <SelectContent>
                    {METHODS.map((m) => (
                      <SelectItem key={m} value={m}>
                        {m}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Select value={netKind} onValueChange={setNetKind}>
                  <SelectTrigger size="sm" className="h-8 w-36 text-xs" aria-label="Kind">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {NETWORK_KINDS.map((k) => (
                      <SelectItem key={k} value={k}>
                        {k}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              {netKind === 'responseJsonPath' && (
                <Input
                  value={netPath}
                  onChange={(e) => setNetPath(e.target.value)}
                  placeholder="json path, e.g. data.users[0].id"
                />
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
