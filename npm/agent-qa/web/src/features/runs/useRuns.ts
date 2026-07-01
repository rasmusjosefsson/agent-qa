// web/src/features/runs/useRuns.ts
import { useCallback, useEffect, useRef, useState } from 'react'
import {
  deleteRun as apiDeleteRun,
  deleteScenario as apiDeleteScenario,
  getRunDetail,
  getRuns,
  getScenarioDef,
  getScenarios,
  startReplay,
  type ReplayOpts,
} from '@/lib/runs-api'
import { isRunLive } from './rows'
import type { RunDetail, RunSummary, ScenarioDef, ScenarioStep, ScenarioSummary, Selection } from './types'

const EMPTY_SEL: Selection = { sid: null, runId: null, stepIdx: null, tab: 'step' }

export interface RunsApi {
  root: string
  scenarios: ScenarioSummary[]
  runsBySid: Record<string, RunSummary[]>
  expanded: Set<string>
  sel: Selection
  detail: RunDetail | null
  scenarioDef: ScenarioDef | null
  runDefSteps: { sid: string | null; steps: ScenarioStep[] }
  live: boolean
  setLive: (v: boolean) => void
  toggleScenario: (sid: string) => Promise<void>
  deleteScenario: (sid: string) => Promise<{ ok: boolean; error?: string }>
  deleteRun: (sid: string, runId: string) => Promise<{ ok: boolean; error?: string }>
  selectRun: (sid: string, runId: string, manual?: boolean) => Promise<void>
  selectStep: (idx: number) => void
  selectTab: (tab: Selection['tab']) => void
  replay: (sid: string, opts?: ReplayOpts) => Promise<{ ok: boolean; error?: string }>
  refresh: () => void
}

export function useRuns(): RunsApi {
  const [root, setRoot] = useState('')
  const [scenarios, setScenarios] = useState<ScenarioSummary[]>([])
  const [runsBySid, setRunsBySid] = useState<Record<string, RunSummary[]>>({})
  const [expanded, setExpanded] = useState<Set<string>>(new Set())
  const [sel, setSel] = useState<Selection>(EMPTY_SEL)
  const [detail, setDetail] = useState<RunDetail | null>(null)
  const [scenarioDef, setScenarioDef] = useState<ScenarioDef | null>(null)
  const [runDefSteps, setRunDefSteps] = useState<{ sid: string | null; steps: ScenarioStep[] }>({ sid: null, steps: [] })
  const [live, setLive] = useState(true)

  // Refs that mirror state so the poll loop / async actions read fresh values.
  const autoFollow = useRef(true)
  const selRef = useRef(sel)
  const detailRef = useRef(detail)
  const expandedRef = useRef(expanded)
  const liveRef = useRef(live)
  const runsRef = useRef(runsBySid)
  const runDefRef = useRef(runDefSteps)
  selRef.current = sel
  detailRef.current = detail
  expandedRef.current = expanded
  liveRef.current = live
  runsRef.current = runsBySid
  runDefRef.current = runDefSteps

  const loadRuns = useCallback(async (sid: string) => {
    try {
      const data = await getRuns(sid)
      setRunsBySid((prev) => ({ ...prev, [sid]: data.replays || [] }))
    } catch {
      /* best-effort */
    }
  }, [])

  const refreshRun = useCallback(async (sidArg?: string, runIdArg?: string) => {
    const s = sidArg ?? selRef.current.sid
    const r = runIdArg ?? selRef.current.runId
    if (!s || !r) return
    if (runDefRef.current.sid !== s) {
      setRunDefSteps({ sid: s, steps: [] })
      try {
        const d = await getScenarioDef(s)
        setRunDefSteps({ sid: s, steps: d.scenario?.steps || [] })
      } catch {
        /* events-only fallback */
      }
    }
    try {
      setDetail(await getRunDetail(s, r))
    } catch {
      /* best-effort */
    }
  }, [])

  const selectRun = useCallback(
    async (sid: string, runId: string, manual = true) => {
      if (manual) autoFollow.current = false
      setScenarioDef(null)
      setSel({ sid, runId, stepIdx: null, tab: 'step' })
      await refreshRun(sid, runId)
    },
    [refreshRun]
  )

  const maybeAutoFollow = useCallback(
    async (list: ScenarioSummary[]) => {
      if (!autoFollow.current || !liveRef.current) return
      const sc = list.find((s) => s.activeRunId)
      if (!sc || !sc.activeRunId) return
      const cur = selRef.current
      if (cur.sid === sc.sid && cur.runId === sc.activeRunId) return
      setExpanded((prev) => new Set(prev).add(sc.sid))
      await loadRuns(sc.sid)
      await selectRun(sc.sid, sc.activeRunId, false)
    },
    [loadRuns, selectRun]
  )

  const loadScenarios = useCallback(async () => {
    const data = await getScenarios()
    setRoot(data.scenariosRoot)
    const list = data.scenarios || []
    setScenarios(list)
    await maybeAutoFollow(list)
    return list
  }, [maybeAutoFollow])

  const selectScenario = useCallback(async (sid: string) => {
    autoFollow.current = false
    setSel({ sid, runId: null, stepIdx: null, tab: 'step' })
    setDetail(null)
    try {
      const r = await getScenarioDef(sid)
      setScenarioDef(r.scenario || null)
    } catch {
      setScenarioDef(null)
    }
  }, [])

  const toggleScenario = useCallback(
    async (sid: string) => {
      // Always select the scenario (so the center pane + Replay button reflect
      // it on the FIRST click), then toggle its run list. Previously clicking an
      // already-expanded row only collapsed it and selected nothing — which is
      // why it took a second click to show the Replay button.
      const wasExpanded = expandedRef.current.has(sid)
      setExpanded((prev) => {
        const n = new Set(prev)
        if (wasExpanded) n.delete(sid)
        else n.add(sid)
        return n
      })
      const tasks: Promise<unknown>[] = [selectScenario(sid)]
      if (!wasExpanded) tasks.push(loadRuns(sid))
      await Promise.all(tasks)
    },
    [loadRuns, selectScenario]
  )

  const deleteScenario = useCallback(
    async (sid: string): Promise<{ ok: boolean; error?: string }> => {
      const res = await apiDeleteScenario(sid)
      if (!res.ok) return res
      setExpanded((prev) => {
        const n = new Set(prev)
        n.delete(sid)
        return n
      })
      setRunsBySid((prev) => {
        const n = { ...prev }
        delete n[sid]
        return n
      })
      if (selRef.current.sid === sid) {
        setSel(EMPTY_SEL)
        setDetail(null)
        setScenarioDef(null)
      }
      await loadScenarios().catch(() => {})
      return { ok: true }
    },
    [loadScenarios]
  )

  const deleteRun = useCallback(
    async (sid: string, runId: string): Promise<{ ok: boolean; error?: string }> => {
      const res = await apiDeleteRun(sid, runId)
      if (!res.ok) return res
      await loadRuns(sid)
      if (selRef.current.sid === sid && selRef.current.runId === runId) {
        setSel((prev) => ({ ...prev, runId: null, stepIdx: null }))
        setDetail(null)
      }
      return { ok: true }
    },
    [loadRuns]
  )

  const selectStep = useCallback((idx: number) => {
    setSel((prev) => ({ ...prev, stepIdx: idx }))
  }, [])

  const selectTab = useCallback((tab: Selection['tab']) => {
    setSel((prev) => ({ ...prev, tab }))
  }, [])

  const replay = useCallback(
    async (sid: string, opts?: ReplayOpts): Promise<{ ok: boolean; error?: string }> => {
      const before = new Set((runsRef.current[sid] || []).map((r) => r.runId))
      const res = await startReplay(sid, opts)
      if (!res.ok) return res
      autoFollow.current = true
      setExpanded((prev) => new Set(prev).add(sid))
      const deadline = Date.now() + 30000
      const tick = async () => {
        await loadRuns(sid)
        const fresh = (runsRef.current[sid] || []).filter((r) => !before.has(r.runId))
        if (fresh.length) {
          fresh.sort((a, b) => (a.runId < b.runId ? 1 : -1))
          await selectRun(sid, fresh[0].runId, false)
          return
        }
        if (Date.now() < deadline) setTimeout(tick, 700)
      }
      setTimeout(tick, 500)
      return { ok: true }
    },
    [loadRuns, selectRun]
  )

  const refresh = useCallback(() => {
    loadScenarios().catch(() => {})
    if (selRef.current.sid && selRef.current.runId) refreshRun().catch(() => {})
  }, [loadScenarios, refreshRun])

  // Boot + 1.5s live poll (mirrors app.js pollTick).
  useEffect(() => {
    let cancelled = false
    // Deep-link: `/?sid=<sid>[&run=<runId>]` (used by the plan dashboard to
    // open a member case's live/last run). Honors the explicit sid over the
    // generic active-run auto-follow.
    loadScenarios()
      .then(async (list) => {
        if (cancelled) return
        const params = new URLSearchParams(window.location.search)
        const wantSid = params.get('sid')
        const sc = wantSid ? (list || []).find((s) => s.sid === wantSid) : undefined
        if (!wantSid || !sc) return
        autoFollow.current = false
        setExpanded((prev) => new Set(prev).add(wantSid))
        await loadRuns(wantSid)
        const runId = params.get('run') || sc.activeRunId || sc.latestRunId
        if (runId) await selectRun(wantSid, runId, false)
        else await selectScenario(wantSid)
      })
      .catch(() => {})
    const id = setInterval(() => {
      void (async () => {
        if (!liveRef.current) return
        const cur = selRef.current
        if (detailRef.current && isRunLive(detailRef.current)) await refreshRun()
        else if (cur.sid && cur.runId) await refreshRun()
        await loadScenarios().catch(() => {})
        for (const sid of expandedRef.current) await loadRuns(sid)
      })().catch(() => {})
    }, 1500)
    return () => {
      cancelled = true
      clearInterval(id)
    }
  }, [loadScenarios, refreshRun, loadRuns, selectRun, selectScenario])

  return {
    root,
    scenarios,
    runsBySid,
    expanded,
    sel,
    detail,
    scenarioDef,
    runDefSteps,
    live,
    setLive,
    toggleScenario,
    deleteScenario,
    deleteRun,
    selectRun,
    selectStep,
    selectTab,
    replay,
    refresh,
  }
}
