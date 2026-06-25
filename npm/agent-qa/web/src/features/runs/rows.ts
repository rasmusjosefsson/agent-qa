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

// Merge the full scenario step list (if known) with the run events so not-yet
// run steps appear up front in a disabled "pending" state.
export function mergeRows(events: RunEvent[], defSteps: ScenarioStep[] | null): RunEvent[] {
  if (!defSteps || !defSteps.length) return events
  const byId = new Map(events.map((e) => [e.id, e]))
  return defSteps.map((ds, i) => {
    const ev = ds.id != null ? byId.get(ds.id) : undefined
    if (ev) return ev
    return {
      idx: i + 1,
      id: ds.id,
      intent: ds.intent || stepText(ds),
      kind: ds.verb,
      status: 'pending',
      total: defSteps.length,
      pending: true,
    }
  })
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

export function defFrom(json: { scenario?: ScenarioDef } | null): ScenarioDef | null {
  return json && json.scenario ? json.scenario : null
}
