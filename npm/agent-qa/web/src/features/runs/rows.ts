// web/src/features/runs/rows.ts
// Pure, unit-tested helpers ported 1:1 from lib/public/app.js so the Runs UI
// renders identical step rows / badges / labels to the classic viewer.
import type { RunEvent, ScenarioDef, ScenarioStep } from './types'

export function fmtMs(ms: unknown): string {
  if (typeof ms !== 'number') return ''
  return ms >= 1000 ? `${(ms / 1000).toFixed(1)}s` : `${ms}ms`
}

export function icon(status: string | undefined): string {
  if (status === 'pass') return '✓'
  if (status === 'fail') return '✗'
  if (status === 'running') return '…'
  if (status === 'pending') return '○'
  if (status === 'skip') return '–'
  return '·'
}

// Verb → badge category (mirrors the editor's step colours).
export type VerbCat = 'nav' | 'click' | 'fill' | 'press' | 'assert' | 'wait' | 'action'
export function verbCat(verb: unknown): VerbCat {
  const v = String(verb || '')
  if (/^(goto|navigate)/.test(v)) return 'nav'
  if (/^click/.test(v)) return 'click'
  if (/^(fill|type)/.test(v)) return 'fill'
  if (/^press/.test(v)) return 'press'
  if (/^assert/.test(v)) return 'assert'
  if (/^wait/.test(v)) return 'wait'
  return 'action'
}

export function verbBadge(verb: unknown): string {
  const map: Record<string, string> = {
    goto: 'GO TO',
    navigate: 'GO TO',
    click: 'CLICK',
    fill: 'FILL',
    type: 'FILL',
    press: 'PRESS',
  }
  return map[String(verb || '')] || String(verb || 'STEP').toUpperCase()
}

// Human fallback when a step has no authored intent.
export function stepText(st: ScenarioStep | RunEvent): string {
  const s = st as ScenarioStep
  if (s.on && (s.on.role || s.on.name)) return `${s.on.role || ''} “${s.on.name || ''}”`.trim()
  if (s.value && s.value.literal != null) return String(s.value.literal)
  return (s.verb as string) || (s.id as string) || ''
}

// Collapse running+terminal rows into one entry per idx (terminal wins).
export function collapseEvents(events: RunEvent[]): RunEvent[] {
  const byIdx = new Map<number, RunEvent>()
  for (const e of events) {
    if (typeof e.idx !== 'number') continue
    byIdx.set(e.idx, { ...(byIdx.get(e.idx) || {}), ...e })
  }
  return [...byIdx.values()].sort((a, b) => a.idx - b.idx)
}

// `when` and `group` never emit their own event — the runner flattens them
// and dispatches the children. Surface those children (with a branch note)
// so a flag's old and new paths both show up before and after a run.
export function flattenAuthored(steps: ScenarioStep[]): ScenarioStep[] {
  const out: ScenarioStep[] = []
  const walk = (list: ScenarioStep[], branch?: string) => {
    for (const step of list) {
      const kids = containerSteps(step)
      if (kids) {
        walk(kids, branchNote(step) || branch)
        continue
      }
      out.push(branch ? { ...step, branch } : step)
    }
  }
  walk(steps)
  return out
}

function containerSteps(step: ScenarioStep): ScenarioStep[] | null {
  if (step.verb !== 'when' && step.verb !== 'group') return null
  const nested = step.params?.steps
  return Array.isArray(nested) ? nested : null
}

function branchNote(step: ScenarioStep): string | undefined {
  if (step.verb === 'group') return step.intent || undefined
  if (step.verb !== 'when' || !step.params) return undefined
  if (step.params.present != null) return `only when ${locatorPhrase(step.params.present)} is on the page`
  if (step.params.absent != null) return `only when ${locatorPhrase(step.params.absent)} is not on the page`
  return step.intent || undefined
}

function locatorPhrase(loc: unknown): string {
  if (typeof loc === 'string') return loc
  if (!loc || typeof loc !== 'object') return 'that control'
  const o = loc as { role?: string; name?: unknown; raw?: { value?: string } }
  const name = typeof o.name === 'string' ? o.name : null
  if (o.role && name) return `${o.role} “${name}”`
  if (o.role) return o.role
  if (o.raw?.value) return o.raw.value
  return 'that control'
}

// Merge the full scenario step list (if known) with the run events so not-yet
// run steps appear up front in a disabled "pending" state. Container steps
// expand to the children the runner actually dispatches. Events whose ids
// are not in that list (an included file, for example) stay on the end.
export function mergeRows(events: RunEvent[], defSteps: ScenarioStep[] | null): RunEvent[] {
  if (!defSteps || !defSteps.length) return events
  const flat = flattenAuthored(defSteps)
  const byId = new Map(events.map((e) => [e.id, e]))
  const seen = new Set<string>()
  const rows = flat.map((ds, i) => {
    if (ds.id) seen.add(ds.id)
    const ev = ds.id != null ? byId.get(ds.id) : undefined
    if (ev) return ev
    return {
      idx: i + 1,
      id: ds.id,
      intent: ds.intent || stepText(ds),
      kind: ds.verb,
      status: 'pending',
      total: flat.length,
      pending: true,
    }
  })
  const extra = events
    .filter((e) => e.id && !seen.has(e.id))
    .sort((a, b) => a.idx - b.idx)
  return extra.length ? [...rows, ...extra] : rows
}

export function isRunLive(detail: { status?: { state?: string } | null } | null): boolean {
  return !!(detail && detail.status && detail.status.state === 'running')
}

// Badge tone for a run summary / verdict.
export function verdictTone(summary: string | null | undefined, state?: string | null): 'pass' | 'fail' | 'running' {
  if (state === 'running') return 'running'
  if (summary && /PASS/.test(summary)) return 'pass'
  if (summary && /FAIL/.test(summary)) return 'fail'
  return 'running'
}

export function scenarioVerdict(run: { state?: string | null; exitCode?: number | null; ok?: boolean | null } | null) {
  if (!run) return null
  if (run.state === 'running') return 'running' as const
  if (run.exitCode === 0 || run.ok === true) return 'pass' as const
  if (typeof run.exitCode === 'number' || run.ok === false) return 'fail' as const
  return null
}

export function cleanSummary(s: string | null | undefined): string {
  return String(s || '').replace(/^SUMMARY:\s*/, '')
}

// Run ids are `<iso-with-dashes>__<hash>`; scenario ids add an `s-` prefix
// (e.g. 2026-06-26T23-25-13-841Z__26048c2f, s-2026-06-15T21-01-40-384Z__0fb96b1f).
function runDate(id: string | null | undefined): Date | null {
  if (!id) return null
  const m = String(id)
    .replace(/^s-/, '')
    .split('__')[0]
    .match(/^(\d{4})-(\d{2})-(\d{2})T(\d{2})-(\d{2})-(\d{2})-(\d{3})Z$/)
  if (!m) return null
  const d = new Date(`${m[1]}-${m[2]}-${m[3]}T${m[4]}:${m[5]}:${m[6]}.${m[7]}Z`)
  return Number.isNaN(d.getTime()) ? null : d
}

const ABS_FMT = new Intl.DateTimeFormat(undefined, {
  month: 'short',
  day: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
})
const REL_FMT = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })

// Absolute local date/time (for the detail header + hover titles).
export function fmtRunTime(runId: string | null | undefined): string {
  const d = runDate(runId)
  return d ? ABS_FMT.format(d) : String(runId || '')
}

// Relative "x ago" for run lists; ≥1 week falls back to the absolute date.
export function relRunTime(runId: string | null | undefined): string {
  const d = runDate(runId)
  if (!d) return String(runId || '')
  const sec = Math.round((d.getTime() - Date.now()) / 1000) // negative = past
  const abs = Math.abs(sec)
  if (abs < 60) return REL_FMT.format(Math.round(sec), 'second')
  if (abs < 3600) return REL_FMT.format(Math.round(sec / 60), 'minute')
  if (abs < 86400) return REL_FMT.format(Math.round(sec / 3600), 'hour')
  if (abs < 604800) return REL_FMT.format(Math.round(sec / 86400), 'day')
  return ABS_FMT.format(d)
}

export function defFrom(json: { scenario?: ScenarioDef } | null): ScenarioDef | null {
  return json && json.scenario ? json.scenario : null
}
