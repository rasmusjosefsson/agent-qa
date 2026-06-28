import { useEffect, useMemo, useRef, useState } from 'react'
import {
  ArrowLeftIcon,
  Loader2Icon,
  SparklesIcon,
  RotateCwIcon,
  ExternalLinkIcon,
  Trash2Icon,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import { Checkbox } from '@/components/ui/checkbox'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { getCase, upsertCase, deleteCase, linkCase } from '@/lib/cases-api'
import { getScenarios, startReplay } from '@/lib/runs-api'
import type { ScenarioSummary } from '@/features/runs/types'
import type { CaseRecord, InputDecl } from './types'
import { extractTokens, reconcileInputs } from './tokens'
import { buildRunPrompt, runIntent } from './prompt'
import { StatusBadge } from './status'

const runMarkerKey = (id: string) => `aqa-case-run:${id}`

function emptyCase(id: string): CaseRecord {
  return {
    schema: 'case/1',
    id,
    title: id,
    startUrl: '',
    preconditions: '',
    steps: [],
    expected: '',
    inputs: {},
    tags: [],
    scenarioSid: null,
    source: 'manual',
    sourceRef: null,
    createdAt: 0,
    updatedAt: 0,
  }
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

export function CaseDetail({ id }: { id: string }) {
  const [loaded, setLoaded] = useState<CaseRecord | null>(null)
  const [scenario, setScenario] = useState<ScenarioSummary | null>(null)
  const [notFound, setNotFound] = useState(false)
  const [err, setErr] = useState('')
  const [flash, setFlash] = useState('')
  const [saving, setSaving] = useState(false)

  // form fields
  const [title, setTitle] = useState('')
  const [startUrl, setStartUrl] = useState('')
  const [preconditions, setPreconditions] = useState('')
  const [stepsText, setStepsText] = useState('')
  const [expected, setExpected] = useState('')
  const [tagsText, setTagsText] = useState('')
  const [inputs, setInputs] = useState<Record<string, InputDecl>>({})

  const hydrate = (c: CaseRecord, s: ScenarioSummary | null) => {
    setLoaded(c)
    setScenario(s)
    setTitle(c.title)
    setStartUrl(c.startUrl)
    setPreconditions(c.preconditions)
    setStepsText(c.steps.join('\n'))
    setExpected(c.expected)
    setTagsText(c.tags.join(', '))
    setInputs(c.inputs || {})
  }

  useEffect(() => {
    getCase(id)
      .then((r) => hydrate(r.case, r.case.scenario))
      .catch((e) => {
        // 404 → fresh draft for this id; anything else is a real error.
        if (String(e.message || e).includes('404')) {
          setNotFound(true)
          hydrate(emptyCase(id), null)
        } else {
          setErr(String(e.message || e))
        }
      })
  }, [id])

  // Keep the inputs map in sync with the [TOKEN]s used in steps + expected.
  const tokens = useMemo(
    () => extractTokens(stepsText.split('\n'), expected),
    [stepsText, expected]
  )
  const tokenKey = tokens.join('|')
  useEffect(() => {
    setInputs((prev) => {
      const next = reconcileInputs(tokens, prev)
      const same =
        Object.keys(next).length === Object.keys(prev).length &&
        Object.keys(next).every((k) => k in prev)
      return same ? prev : next
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tokenKey])

  const current: CaseRecord | null = loaded && {
    ...loaded,
    title: title.trim() || id,
    startUrl: startUrl.trim(),
    preconditions: preconditions.trim(),
    steps: stepsText.split('\n').map((s) => s.trim()).filter(Boolean),
    expected: expected.trim(),
    tags: tagsText.split(',').map((t) => t.trim()).filter(Boolean),
    inputs,
  }

  const dirty =
    !!current &&
    !!loaded &&
    (current.title !== loaded.title ||
      current.startUrl !== loaded.startUrl ||
      current.preconditions !== loaded.preconditions ||
      current.expected !== loaded.expected ||
      current.steps.join('\n') !== loaded.steps.join('\n') ||
      current.tags.join(',') !== loaded.tags.join(',') ||
      JSON.stringify(current.inputs) !== JSON.stringify(loaded.inputs) ||
      notFound)

  const save = async (): Promise<CaseRecord | null> => {
    if (!current) return null
    setSaving(true)
    setErr('')
    try {
      const r = await upsertCase(id, current)
      setLoaded(r.case)
      setNotFound(false)
      setFlash('Saved')
      setTimeout(() => setFlash(''), 1500)
      return r.case
    } catch (e) {
      setErr(String((e as Error).message || e))
      return null
    } finally {
      setSaving(false)
    }
  }

  const runWithAgent = async () => {
    const saved = await save()
    if (!saved) return
    // Mark a pending run so we can adopt the resulting scenario on return.
    localStorage.setItem(runMarkerKey(id), String(Date.now()))
    const prompt = buildRunPrompt(saved, window.location.origin)
    window.location.href = `/chat?ask=${encodeURIComponent(prompt)}`
  }

  const replay = async () => {
    if (!loaded?.scenarioSid) return
    setErr('')
    const r = await startReplay(loaded.scenarioSid)
    if (!r.ok) setErr(`Replay did not start: ${r.error}`)
    else window.location.href = '/'
  }

  // Link-back poller: after a "Run with agent", watch for the recorded scenario
  // (intent carries the case id) and adopt its sid. Belt-and-suspenders with
  // the agent's own curl link-back in the seed prompt.
  const pollRef = useRef<number | undefined>(undefined)
  useEffect(() => {
    if (!loaded) return
    const marker = localStorage.getItem(runMarkerKey(id))
    if (!marker) return
    const startedAt = Number(marker)
    if (Date.now() - startedAt > 10 * 60 * 1000) {
      localStorage.removeItem(runMarkerKey(id))
      return
    }
    const wantIntent = runIntent({ title: loaded.title, id })
    let alive = true
    const tick = async () => {
      try {
        const { scenarios } = await getScenarios()
        const matches = scenarios.filter((s) => s.intent === wantIntent && s.sid)
        const newest = matches[matches.length - 1] // sids sort ~chronologically
        if (newest && newest.sid !== loaded.scenarioSid) {
          const r = await linkCase(id, newest.sid)
          localStorage.removeItem(runMarkerKey(id))
          if (alive) hydrate(r.case, r.case.scenario)
          return
        }
      } catch {
        /* keep polling */
      }
      if (alive) pollRef.current = window.setTimeout(tick, 3000)
    }
    void tick()
    return () => {
      alive = false
      if (pollRef.current) window.clearTimeout(pollRef.current)
    }
  }, [loaded, id])

  const setInput = (token: string, patch: Partial<InputDecl>) =>
    setInputs((prev) => ({ ...prev, [token]: { ...prev[token], ...patch } }))

  if (err && !loaded) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center p-8 text-sm text-destructive">
        {err}
      </div>
    )
  }
  if (!loaded || !current) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center text-sm text-muted-foreground">
        <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading case…
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* header */}
      <div className="flex items-center gap-3 border-b border-border px-4 py-2.5">
        <a
          href="/cases"
          className="flex size-7 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
          aria-label="Back to cases"
        >
          <ArrowLeftIcon className="size-4" />
        </a>
        <Input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Untitled case"
          className="h-8 max-w-xl border-transparent bg-transparent px-1 text-base font-semibold shadow-none focus-visible:border-border"
        />
        <div className="ml-auto flex items-center gap-2">
          {flash && <span className="text-xs text-emerald-400">{flash}</span>}
          {loaded.scenarioSid && (
            <>
              <Button size="sm" variant="ghost" onClick={() => (window.location.href = '/')}>
                <ExternalLinkIcon /> Open in Runs
              </Button>
              <Button size="sm" variant="outline" onClick={() => void replay()}>
                <RotateCwIcon /> Replay
              </Button>
            </>
          )}
          <Button size="sm" variant="outline" onClick={() => void save()} disabled={!dirty || saving}>
            {saving && <Loader2Icon className="animate-spin" />}
            Save
          </Button>
          <Button size="sm" onClick={() => void runWithAgent()}>
            <SparklesIcon /> Run with agent
          </Button>
        </div>
      </div>

      {err && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-4 py-1.5 text-xs text-destructive">
          {err}
        </div>
      )}

      <div className="flex min-h-0 flex-1 overflow-hidden">
        {/* main form */}
        <div className="min-h-0 flex-1 space-y-5 overflow-auto px-5 py-4">
          <Field label="Start URL" hint="Where the agent opens the browser before step 1.">
            <Input
              value={startUrl}
              onChange={(e) => setStartUrl(e.target.value)}
              placeholder="https://app.example.com/login"
            />
          </Field>

          <Field label="Preconditions" hint="Optional — state the world must be in first.">
            <Textarea
              value={preconditions}
              onChange={(e) => setPreconditions(e.target.value)}
              rows={2}
              placeholder="e.g. A user account already exists."
            />
          </Field>

          <Field
            label="Steps"
            hint="One step per line, in plain English. Use [TOKEN] for test data (e.g. [EMAIL])."
          >
            <Textarea
              value={stepsText}
              onChange={(e) => setStepsText(e.target.value)}
              rows={8}
              className="font-mono text-[13px] leading-relaxed"
              placeholder={'Go to Settings > Plugins\nClick Add in the top right\nEnter [EMAIL] in the email field\n…'}
            />
          </Field>

          <Field label="Expected result" hint="What proves the case passed.">
            <Textarea
              value={expected}
              onChange={(e) => setExpected(e.target.value)}
              rows={3}
              placeholder='e.g. A banner reads "Changes have been saved" and the field persists after refresh.'
            />
          </Field>

          {tokens.length > 0 && (
            <Field
              label="Test data"
              hint="Detected from [TOKEN]s above. Mark secrets sensitive — their values stay out of the recording."
            >
              <div className="overflow-hidden rounded-md border border-border">
                {tokens.map((t) => {
                  const d = inputs[t] || {}
                  return (
                    <div
                      key={t}
                      className="flex items-center gap-3 border-b border-border/60 px-3 py-2 last:border-0"
                    >
                      <span className="w-40 shrink-0 font-mono text-xs text-pink-400">[{t}]</span>
                      <Input
                        value={d.sensitive ? '' : String(d.default ?? '')}
                        onChange={(e) => setInput(t, { default: e.target.value })}
                        disabled={d.sensitive}
                        placeholder={d.sensitive ? 'kept secret' : 'default value'}
                        className="h-7 flex-1"
                      />
                      <label className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
                        <Checkbox
                          checked={!!d.sensitive}
                          onCheckedChange={(v) => setInput(t, { sensitive: !!v })}
                        />
                        sensitive
                      </label>
                    </div>
                  )
                })}
              </div>
            </Field>
          )}

          <Field label="Tags" hint="Comma-separated.">
            <Input
              value={tagsText}
              onChange={(e) => setTagsText(e.target.value)}
              placeholder="smoke, auth"
            />
          </Field>
        </div>

        {/* properties */}
        <aside className="hidden w-72 shrink-0 flex-col gap-4 overflow-auto border-l border-border px-4 py-4 lg:flex">
          <Prop label="Status">
            <StatusBadge scenario={scenario} />
          </Prop>
          <Prop label="Scenario">
            {loaded.scenarioSid ? (
              <span className="font-mono text-xs break-all text-foreground">{loaded.scenarioSid}</span>
            ) : (
              <span className="text-xs text-muted-foreground">Not recorded yet</span>
            )}
          </Prop>
          {scenario?.latestRun && (
            <Prop label="Last run">
              <div className="text-xs text-muted-foreground">
                {scenario.latestRun.summary || scenario.latestRun.state || '—'}
                <div>{fmtAgo(scenario.latestRun.finishedAt)}</div>
              </div>
            </Prop>
          )}
          <Prop label="Source">
            <span className="text-xs capitalize text-muted-foreground">
              {loaded.source}
              {loaded.sourceRef ? ` · ${loaded.sourceRef}` : ''}
            </span>
          </Prop>
          <Prop label="Last edit">
            <span className="text-xs text-muted-foreground">{fmtAgo(loaded.updatedAt)}</span>
          </Prop>

          <div className="mt-auto">
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button
                  size="sm"
                  variant="ghost"
                  className="w-full text-destructive hover:bg-destructive/10 hover:text-destructive"
                >
                  <Trash2Icon /> Delete case
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Delete this test case?</AlertDialogTitle>
                  <AlertDialogDescription>
                    Removes the case definition. The recorded scenario and its runs are kept in Runs.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction
                    onClick={() => {
                      void deleteCase(id).then(() => (window.location.href = '/cases'))
                    }}
                  >
                    Delete
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </div>
        </aside>
      </div>
    </div>
  )
}

function Field({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs font-medium text-foreground">{label}</Label>
      {hint && <p className="text-[11px] text-muted-foreground">{hint}</p>}
      {children}
    </div>
  )
}

function Prop({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="space-y-1">
      <div className="text-[11px] uppercase tracking-wide text-muted-foreground">{label}</div>
      {children}
    </div>
  )
}
