import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  ArrowLeftIcon,
  Loader2Icon,
  PlayIcon,
  RefreshCwIcon,
  SaveIcon,
  Trash2Icon,
  UploadIcon,
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
import { getSets } from '@/lib/sets-api'
import { deletePlan, getPlan, runPlan, upsertPlan } from '@/lib/plans-api'
import { getPersonas, getEnvironments } from '@/lib/run-config-api'
import type { CaseWithScenario } from '@/features/cases/types'
import type { SetWithCount } from '@/features/sets/types'
import type { PersonaRecord } from '@/features/personas/types'
import type { EnvironmentRecord } from '@/features/environments/types'
import { StatusBadge, caseStatus } from '@/features/cases/status'
import { ReplayLive } from '@/features/runs/components/ReplayLive'
import { Lightbox } from '@/features/runs/components/Lightbox'
import { buildXrayExportPrompt, type XrayExportItem } from '@/features/knowledge/exportPrompt'

const back = () => {
  window.location.href = '/plans'
}
const gotoCase = (id: string) => {
  window.location.href = `/cases?id=${encodeURIComponent(id)}`
}
// Open the full run view (live browser stream + per-step screenshots/pass-fail)
// for a member case's scenario.
const gotoRun = (sid: string) => {
  window.location.href = `/?sid=${encodeURIComponent(sid)}`
}

// "step 3/5" while a case is mid-replay, from its scenario summary. currentIdx
// is 0-based and can tick one past the last step at the end, so clamp to total.
function stepProgress(c: CaseWithScenario): string | null {
  const r = c.scenario?.latestRun
  if (!r || r.state !== 'running') return null
  const total = typeof r.total === 'number' ? r.total : null
  let cur = typeof r.currentIdx === 'number' ? r.currentIdx + 1 : null
  if (cur != null && total != null) cur = Math.min(cur, total)
  return cur && total ? `step ${cur}/${total}` : 'running'
}

// Is this member's replay in flight right now?
function isRunning(c: CaseWithScenario): boolean {
  return !!(c.scenario?.sid && (c.scenario.activeRunId || c.scenario.latestRun?.state === 'running'))
}

// Client mirror of the server's resolveSetCaseIds, so the dashboard reflects
// scope edits before they're saved.
function resolveSet(set: SetWithCount, allCases: CaseWithScenario[]): string[] {
  if (set.mode === 'tag') {
    if (set.tagQuery.length === 0) return []
    const want = new Set(set.tagQuery)
    return allCases.filter((c) => c.tags.some((t) => want.has(t))).map((c) => c.id)
  }
  const byId = new Set(allCases.map((c) => c.id))
  return set.caseIds.filter((cid) => byId.has(cid))
}

export function PlanDetail({ id }: { id: string }) {
  const [loaded, setLoaded] = useState(false)
  const [allCases, setAllCases] = useState<CaseWithScenario[]>([])
  const [allSets, setAllSets] = useState<SetWithCount[]>([])
  const [personas, setPersonas] = useState<PersonaRecord[]>([])
  const [environments, setEnvironments] = useState<EnvironmentRecord[]>([])
  const [personaId, setPersonaId] = useState('')
  const [envId, setEnvId] = useState('')
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const [live, setLive] = useState(false)
  const [runMsg, setRunMsg] = useState('')
  const [confirmDel, setConfirmDel] = useState(false)
  const [lightbox, setLightbox] = useState<{ url: string; caption: string } | null>(null)

  // Editable fields.
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [setIds, setSetIds] = useState<string[]>([])
  const [caseIds, setCaseIds] = useState<string[]>([])

  const refreshCases = useCallback(() => {
    return getCases()
      .then((r) => setAllCases(r.cases))
      .catch(() => {})
  }, [])

  useEffect(() => {
    Promise.all([getPlan(id), getSets(), getCases(), getPersonas(), getEnvironments()])
      .then(([p, s, c, pe, en]) => {
        setName(p.plan.name)
        setDescription(p.plan.description)
        setSetIds(p.plan.scope.setIds)
        setCaseIds(p.plan.scope.caseIds)
        setAllSets(s.sets)
        setAllCases(c.cases)
        setPersonas(pe.personas)
        setEnvironments(en.environments)
        setLoaded(true)
      })
      .catch((e) => setErr(String(e.message || e)))
  }, [id])

  // Resolve the chosen persona + environment into the run payload:
  // persona.profile → --profile, environment.baseUrl + params → --param.
  const buildRunOpts = () => {
    const persona = personas.find((p) => p.id === personaId)
    const env = environments.find((e) => e.id === envId)
    const params: Record<string, string> = env ? { ...env.params } : {}
    if (env?.baseUrl) params.baseUrl = env.baseUrl
    return {
      profile: persona?.profile || undefined,
      params: Object.keys(params).length ? params : undefined,
    }
  }

  // Live status polling after a run (and on demand).
  useEffect(() => {
    if (!live) return
    const t = setInterval(() => void refreshCases(), 3000)
    return () => clearInterval(t)
  }, [live, refreshCases])

  const caseById = useMemo(() => new Map(allCases.map((c) => [c.id, c])), [allCases])

  // Resolved members: union of cases from selected sets, then direct caseIds.
  const members = useMemo(() => {
    const order: string[] = []
    const seen = new Set<string>()
    const add = (cid: string) => {
      if (!seen.has(cid)) {
        seen.add(cid)
        order.push(cid)
      }
    }
    const setById = new Map(allSets.map((s) => [s.id, s]))
    for (const sid of setIds) {
      const s = setById.get(sid)
      if (s) for (const cid of resolveSet(s, allCases)) add(cid)
    }
    const byId = new Set(allCases.map((c) => c.id))
    for (const cid of caseIds) if (byId.has(cid)) add(cid)
    return order.map((cid) => caseById.get(cid)!).filter(Boolean)
  }, [setIds, caseIds, allSets, allCases, caseById])

  const rollup = useMemo(() => {
    const acc = { pass: 0, fail: 0, running: 0, recorded: 0, none: 0 }
    for (const c of members) acc[caseStatus(c.scenario).tone] += 1
    return acc
  }, [members])

  // Count of members mid-replay — keep polling whenever something is running
  // (covers opening a plan while a run is already going).
  const runningCount = useMemo(() => members.filter(isRunning).length, [members])
  useEffect(() => {
    if (runningCount > 0) setLive(true)
  }, [runningCount])

  // Members that map to an Xray test, with their latest verdict + run link —
  // the payload an export pushes back as a Test Execution.
  const xrayItems = useMemo<XrayExportItem[]>(() => {
    const out: XrayExportItem[] = []
    for (const c of members) {
      const ref = c.externalRefs.find((r) => r.provider === 'xray')
      if (!ref) continue
      const tone = caseStatus(c.scenario).tone
      const result = tone === 'pass' ? 'pass' : tone === 'fail' ? 'fail' : 'todo'
      const run = c.scenario?.latestRun
      const runUrl = run
        ? `${window.location.origin}/api/scenarios/${c.scenario!.sid}/runs/${run.runId}`
        : null
      out.push({ xrayKey: ref.key, title: c.title, result, runUrl })
    }
    return out
  }, [members])

  const anyRunning = rollup.running > 0
  // Stop auto-polling once nothing is in flight.
  useEffect(() => {
    if (live && !anyRunning && runMsg) {
      const t = setTimeout(() => setLive(false), 6000)
      return () => clearTimeout(t)
    }
  }, [live, anyRunning, runMsg])

  const toggleSet = (sid: string) =>
    setSetIds((p) => (p.includes(sid) ? p.filter((x) => x !== sid) : [...p, sid]))
  const toggleCase = (cid: string) =>
    setCaseIds((p) => (p.includes(cid) ? p.filter((x) => x !== cid) : [...p, cid]))

  const save = async () => {
    setBusy(true)
    setErr('')
    try {
      await upsertPlan(id, { name: name.trim() || id, description, scope: { setIds, caseIds } })
      back()
    } catch (e) {
      setErr(String((e as Error).message || e))
      setBusy(false)
    }
  }

  const run = async () => {
    setBusy(true)
    setErr('')
    setRunMsg('')
    try {
      // Persist scope first so the server runs exactly what's shown.
      await upsertPlan(id, { name: name.trim() || id, description, scope: { setIds, caseIds } })
      const r = await runPlan(id, buildRunOpts())
      setRunMsg(
        `Started ${r.started.length} ${r.started.length === 1 ? 'replay' : 'replays'}` +
          (r.skipped.length ? ` · skipped ${r.skipped.length} (no recording)` : '')
      )
      setLive(true)
      void refreshCases()
    } catch (e) {
      setErr(String((e as Error).message || e))
    } finally {
      setBusy(false)
    }
  }

  const exportXray = () => {
    if (xrayItems.length === 0) return
    const prompt = buildXrayExportPrompt(name.trim() || id, xrayItems, window.location.origin)
    window.location.href = `/chat?ask=${encodeURIComponent(prompt)}`
  }

  const remove = async () => {
    setBusy(true)
    try {
      await deletePlan(id)
      back()
    } catch (e) {
      setErr(String((e as Error).message || e))
      setBusy(false)
    }
  }

  if (err && !loaded) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-sm text-destructive">
        {err}
        <Button variant="ghost" size="sm" onClick={back}>
          <ArrowLeftIcon /> Back to plans
        </Button>
      </div>
    )
  }
  if (!loaded) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading plan…
      </div>
    )
  }

  const runnable = members.some((c) => c.scenario?.hasScenario)

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-3 border-b border-border px-5 py-3">
        <Button variant="ghost" size="icon" className="size-7" onClick={back} title="Back to plans">
          <ArrowLeftIcon className="size-4" />
        </Button>
        <div className="min-w-0 flex-1">
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="h-8 border-transparent bg-transparent px-1 text-base font-semibold shadow-none focus-visible:border-border"
          />
          <div className="px-1 font-mono text-[11px] text-muted-foreground">
            {id} · {members.length} {members.length === 1 ? 'case' : 'cases'}
          </div>
        </div>
        <Button variant="ghost" size="sm" onClick={() => setConfirmDel(true)} disabled={busy}>
          <Trash2Icon /> Delete
        </Button>
        <Button variant="ghost" size="sm" onClick={() => void save()} disabled={busy}>
          {busy ? <Loader2Icon className="animate-spin" /> : <SaveIcon />} Save
        </Button>
        <Button
          variant="secondary"
          size="sm"
          onClick={exportXray}
          disabled={busy || xrayItems.length === 0}
          title={
            xrayItems.length
              ? `Push ${xrayItems.length} result${xrayItems.length === 1 ? '' : 's'} to Xray`
              : 'No member cases are linked to Xray'
          }
        >
          <UploadIcon /> Export to Xray
        </Button>
        <Button size="sm" onClick={() => void run()} disabled={busy || !runnable}>
          <PlayIcon /> Run plan
        </Button>
      </div>

      {/* Run config — who/where the next run uses. */}
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1 border-b border-border px-5 py-2 text-xs text-muted-foreground">
        <span>Run as</span>
        <RunSelect
          value={personaId}
          onChange={setPersonaId}
          placeholder="default login"
          options={personas.map((p) => ({ value: p.id, label: p.name }))}
        />
        <span>on</span>
        <RunSelect
          value={envId}
          onChange={setEnvId}
          placeholder="default environment"
          options={environments.map((e) => ({ value: e.id, label: e.name }))}
        />
        {personas.length === 0 && environments.length === 0 && (
          <span className="text-[11px] opacity-70">
            — add Personas / Environments to pick a login or target
          </span>
        )}
      </div>

      {err && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-5 py-1.5 text-xs text-destructive">
          {err}
        </div>
      )}
      {runMsg && (
        <div className="border-b border-border bg-muted/30 px-5 py-1.5 text-xs text-muted-foreground">
          {runMsg}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        <div className="mx-auto grid max-w-5xl grid-cols-1 gap-6 p-5 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)]">
          {/* Scope */}
          <div className="space-y-5">
            <div className="space-y-2">
              <Label htmlFor="plan-desc">Description</Label>
              <Textarea
                id="plan-desc"
                rows={2}
                placeholder="What does this plan cover?"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>

            <div className="space-y-2">
              <Label>Sets</Label>
              {allSets.length === 0 ? (
                <p className="text-xs text-muted-foreground">No sets yet.</p>
              ) : (
                <div className="divide-y divide-border/60 rounded-md border border-border">
                  {allSets.map((s) => (
                    <label
                      key={s.id}
                      className="flex cursor-pointer items-center gap-3 px-3 py-2 hover:bg-muted/40"
                    >
                      <Checkbox checked={setIds.includes(s.id)} onCheckedChange={() => toggleSet(s.id)} />
                      <span className="min-w-0 flex-1 truncate text-sm">{s.name}</span>
                      <span className="text-[11px] text-muted-foreground">{s.caseCount}</span>
                    </label>
                  ))}
                </div>
              )}
            </div>

            <div className="space-y-2">
              <Label>Individual cases</Label>
              {allCases.length === 0 ? (
                <p className="text-xs text-muted-foreground">No cases yet.</p>
              ) : (
                <div className="max-h-72 divide-y divide-border/60 overflow-auto rounded-md border border-border">
                  {allCases.map((c) => (
                    <label
                      key={c.id}
                      className="flex cursor-pointer items-center gap-3 px-3 py-2 hover:bg-muted/40"
                    >
                      <Checkbox checked={caseIds.includes(c.id)} onCheckedChange={() => toggleCase(c.id)} />
                      <span className="min-w-0 flex-1 truncate text-sm">{c.title}</span>
                    </label>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Dashboard */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <Label>Results</Label>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 px-2 text-xs"
                onClick={() => void refreshCases()}
              >
                <RefreshCwIcon className={live ? 'animate-spin' : ''} /> Refresh
              </Button>
            </div>

            <div className="flex flex-wrap gap-2 text-xs">
              <Pill label="Passed" n={rollup.pass} tone="text-emerald-400" />
              <Pill label="Failed" n={rollup.fail} tone="text-destructive" />
              <Pill label="Running" n={rollup.running} tone="text-amber-300" />
              <Pill label="Not recorded" n={rollup.none} tone="text-muted-foreground" />
            </div>

            {members.length === 0 ? (
              <div className="rounded-md border border-dashed border-border px-3 py-8 text-center text-xs text-muted-foreground">
                Select sets or cases on the left to build this plan.
              </div>
            ) : (
              <div className="divide-y divide-border/60 rounded-md border border-border">
                {members.map((c) => {
                  const recorded = !!c.scenario?.hasScenario
                  const progress = stepProgress(c)
                  const running = isRunning(c)
                  return (
                    <div key={c.id}>
                      <button
                        onClick={() => (recorded ? gotoRun(c.scenario!.sid) : gotoCase(c.id))}
                        title={recorded ? 'Open the live run + step details' : 'Open the case'}
                        className="flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-muted/40"
                      >
                        <div className="min-w-0 flex-1">
                          <div className="truncate text-sm font-medium text-foreground">{c.title}</div>
                          <div className="font-mono text-[11px] text-muted-foreground">{c.id}</div>
                        </div>
                        {progress && <span className="text-[11px] text-amber-400">{progress}</span>}
                        <StatusBadge scenario={c.scenario} />
                      </button>
                      {/* Live browser, inline under the case it belongs to. */}
                      {running && (
                        <div className="h-52 border-t border-amber-500/30 bg-black/40 px-2 pb-2 pt-1">
                          <ReplayLive
                            sid={c.scenario!.sid}
                            onLightbox={(url, caption) => setLightbox({ url, caption })}
                          />
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        </div>
      </div>

      <Dialog open={confirmDel} onOpenChange={setConfirmDel}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete this plan?</DialogTitle>
            <DialogDescription>
              This removes the plan “{name}”. Its sets and cases are not deleted.
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

      {lightbox && (
        <Lightbox url={lightbox.url} caption={lightbox.caption} onClose={() => setLightbox(null)} />
      )}
    </div>
  )
}

function RunSelect({
  value,
  onChange,
  placeholder,
  options,
}: {
  value: string
  onChange: (v: string) => void
  placeholder: string
  options: { value: string; label: string }[]
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="rounded-md border border-border bg-background px-2 py-1 text-xs text-foreground outline-none focus-visible:border-ring"
    >
      <option value="">{placeholder}</option>
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  )
}

function Pill({ label, n, tone }: { label: string; n: number; tone: string }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border border-border bg-muted/30 px-2.5 py-1">
      <span className={`font-semibold ${tone}`}>{n}</span>
      <span className="text-muted-foreground">{label}</span>
    </span>
  )
}
