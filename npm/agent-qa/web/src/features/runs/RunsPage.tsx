// web/src/features/runs/RunsPage.tsx
import { useEffect, useRef, useState } from 'react'
import { useRuns } from './useRuns'
import { ScenarioSidebar } from './components/ScenarioSidebar'
import { CenterPane } from './components/CenterPane'
import { StepDetail } from './components/StepDetail'
import { ReplayLive } from './components/ReplayLive'
import { Lightbox } from './components/Lightbox'
import { isRunLive } from './rows'
import { useMediaQuery } from '@/lib/useMediaQuery'
import { effectiveSettings } from '@/lib/settings-api'
import { Group, Panel, useDefaultLayout } from 'react-resizable-panels'
import { ResizeHandle } from '@/components/ResizeHandle'
import { getPersonas, getEnvironments } from '@/lib/run-config-api'
import { getScenarioDef } from '@/lib/runs-api'
import { replayParams, visibleInputs, type InputDecl } from './replay-params'
import type { PersonaRecord } from '@/features/personas/types'
import type { EnvironmentRecord } from '@/features/environments/types'

export function RunsPage() {
  const runs = useRuns()
  // Phone widths can't hold the scenario list beside the run. Stack them;
  // keep a separate layout id so a narrow session doesn't overwrite the
  // desktop column widths.
  const wide = useMediaQuery('(min-width: 768px)')
  const wideLayout = useDefaultLayout({ id: 'aqa-runs-cols' })
  const narrowLayout = useDefaultLayout({ id: 'aqa-runs-rows' })
  const cols = wide ? wideLayout : narrowLayout
  const [lightbox, setLightbox] = useState<{ url: string; caption: string } | null>(null)
  const [replayErr, setReplayErr] = useState('')

  // Optional run config: which persona (login) / environment (target) a replay
  // uses. The ids ride along so the server resolves the persona's credentials
  // and self-bootstraps the login — an auth-walled scenario re-authenticates.
  const [personas, setPersonas] = useState<PersonaRecord[]>([])
  const [environments, setEnvironments] = useState<EnvironmentRecord[]>([])
  const [personaId, setPersonaId] = useState('')
  const [envId, setEnvId] = useState('')
  const [headed, setHeaded] = useState(false)
  // Declared inputs per scenario, plus whatever the person typed over the default.
  const [declaredBySid, setDeclaredBySid] = useState<Record<string, Record<string, InputDecl>>>({})
  const [draftsBySid, setDraftsBySid] = useState<Record<string, Record<string, string>>>({})
  // Seed from the workbench "headed by default" setting once it loads.
  useEffect(() => {
    void effectiveSettings()
      .then((s) => setHeaded(s.effective.headedDefault))
      .catch(() => {})
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    Promise.all([getPersonas(), getEnvironments()])
      .then(([pe, en]) => {
        setPersonas(pe.personas)
        setEnvironments(en.environments)
      })
      .catch(() => {})
  }, [])

  // Default "Replay as" to the scenario's RECORDED login: an auth-walled
  // scenario records `env.open: [{useProfile, name}]`; preselect the persona
  // whose profile matches (+ the sole environment) so replay works first click.
  // Defaults once per scenario; a manual change sticks until you switch scenarios.
  const defaultedFor = useRef<string | null>(null)
  useEffect(() => {
    const sid = runs.sel.sid
    if (!sid || defaultedFor.current === sid) return
    const recorded = (runs.scenarioDef?.env?.open || []).find((o) => o.kind === 'useProfile')?.name
    if (!recorded) return // not auth-walled → leave the pickers alone
    const p = personas.find((x) => x.profile === recorded)
    if (!p) return
    defaultedFor.current = sid
    setPersonaId(p.id)
    if (environments.length === 1) setEnvId(environments[0].id)
  }, [runs.sel.sid, runs.scenarioDef, personas, environments])

  // A replay of the selected scenario is in flight — block re-firing (a second
  // run would collide on the shared `replay-<sid>` browser session and fail
  // setup). `replayingSid` bridges the gap before the run is tracked; then the
  // in-flight run itself keeps it disabled.
  const [replayingSid, setReplayingSid] = useState<string | null>(null)
  const d = runs.detail
  const viewedRunActive = !!(
    d &&
    d.sid === runs.sel.sid &&
    !d.audit?.summary &&
    d.status?.state !== 'done' &&
    // 'stale' = dead-but-flagged-running — must NOT hold the Replay button.
    d.status?.state !== 'stale'
  )
  const busy = viewedRunActive || replayingSid === runs.sel.sid
  useEffect(() => {
    if (viewedRunActive && replayingSid === runs.sel.sid) setReplayingSid(null)
  }, [viewedRunActive, replayingSid, runs.sel.sid])

  const selectedSid = runs.sel.sid
  useEffect(() => {
    if (!selectedSid || declaredBySid[selectedSid]) return
    let alive = true
    getScenarioDef(selectedSid)
      .then((d) => {
        if (!alive) return
        setDeclaredBySid((prev) => (prev[selectedSid] ? prev : { ...prev, [selectedSid]: d.scenario?.inputs || {} }))
      })
      .catch(() => {
        if (alive) setDeclaredBySid((prev) => (prev[selectedSid] ? prev : { ...prev, [selectedSid]: {} }))
      })
    return () => {
      alive = false
    }
  }, [selectedSid, declaredBySid])

  const declared = selectedSid ? declaredBySid[selectedSid] : undefined
  const drafts = (selectedSid && draftsBySid[selectedSid]) || {}
  const inputs = visibleInputs(declared).map((inp) => ({
    ...inp,
    value: Object.prototype.hasOwnProperty.call(drafts, inp.name) ? drafts[inp.name] : inp.value,
  }))

  const onReplay = async (sid: string) => {
    setReplayErr('')
    setReplayingSid(sid)
    const persona = personas.find((p) => p.id === personaId)
    const env = environments.find((e) => e.id === envId)
    const envParams: Record<string, string> = env ? { ...env.params } : {}
    if (env?.baseUrl) envParams.baseUrl = env.baseUrl
    const forSid = declaredBySid[sid]
    const shown = visibleInputs(forSid).map((inp) => ({
      ...inp,
      value:
        draftsBySid[sid] && Object.prototype.hasOwnProperty.call(draftsBySid[sid], inp.name)
          ? draftsBySid[sid][inp.name]
          : inp.value,
    }))
    const params = replayParams(
      forSid,
      envParams,
      Object.fromEntries(shown.map((inp) => [inp.name, inp.value])),
    )
    const res = await runs.replay(sid, {
      headed,
      profile: persona?.profile || undefined,
      params,
      personaId: personaId || undefined,
      environmentId: envId || undefined,
    })
    if (!res.ok) {
      setReplayErr(`Replay did not start: ${res.error}`)
      setReplayingSid(null)
    } else {
      // Backstop: drop the bridge flag if the in-flight run never gets tracked.
      window.setTimeout(() => setReplayingSid((cur) => (cur === sid ? null : cur)), 45000)
    }
  }

  // Per-replay run config, handed to the scenario's Replay controls (not a
  // global top bar) — so you pick the login/target for the scenario you're
  // about to replay, defaulted to its recorded persona.
  const runConfig = {
    personas,
    environments,
    personaId,
    envId,
    headed,
    setPersonaId,
    setEnvId,
    setHeaded,
    inputs,
    setInput: (name: string, value: string) => {
      if (!selectedSid) return
      setDraftsBySid((prev) => ({ ...prev, [selectedSid]: { ...(prev[selectedSid] || {}), [name]: value } }))
    },
  }

  // Right pane: live screencast while the run is in flight and no step pinned,
  // otherwise the static step detail. Only mounted when a run is open — with
  // nothing selected the third column would just be a second "nothing here"
  // pane next to the center empty state.
  const showLive = isRunLive(runs.detail) && runs.sel.stepIdx == null && runs.sel.sid
  const showRight = !!runs.detail

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {replayErr && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-4 py-1.5 text-xs text-destructive">{replayErr}</div>
      )}

      {/* Resizable full-bleed columns — drag the dividers to resize. The right
          column only exists once a run is open; otherwise the center empty
          state gets the room. */}
      <Group
        key={wide ? 'h' : 'v'}
        orientation={wide ? 'horizontal' : 'vertical'}
        defaultLayout={cols.defaultLayout}
        onLayoutChanged={cols.onLayoutChanged}
        className="min-h-0 flex-1"
      >
        <Panel defaultSize={showRight ? "22" : "26"} minSize="14%" className="min-h-0">
          <ScenarioSidebar runs={runs} />
        </Panel>
        <ResizeHandle />
        <Panel defaultSize={showRight ? "56" : "74"} minSize="30%" className="min-h-0">
          <CenterPane runs={runs} onReplay={(sid) => void onReplay(sid)} busy={busy} runConfig={runConfig} />
        </Panel>
        {showRight && (
          <>
            <ResizeHandle />
            <Panel defaultSize="22" minSize="16%" className="min-h-0" data-qa-volatile>
              {showLive ? (
                <ReplayLive sid={runs.sel.sid!} onLightbox={(url, caption) => setLightbox({ url, caption })} />
              ) : (
                <StepDetail runs={runs} onLightbox={(url, caption) => setLightbox({ url, caption })} />
              )}
            </Panel>
          </>
        )}
      </Group>

      {/* Bottom status stripe — scenarios root + live toggle */}
      <div data-qa-volatile className="flex h-7 shrink-0 items-center justify-between gap-3 border-t border-border px-4 text-xs text-muted-foreground">
        <div className="truncate font-mono">{runs.root}</div>
        <button
          type="button"
          onClick={() => runs.setLive(!runs.live)}
          title={runs.live ? 'Auto-refreshing — click to pause' : 'Paused — click to auto-refresh runs'}
          className="flex shrink-0 items-center gap-1.5 uppercase tracking-wide transition-colors hover:text-foreground"
        >
          <span
            className={
              'size-1.5 rounded-full ' +
              (runs.live ? 'bg-success animate-pulse' : 'bg-muted-foreground/40')
            }
          />
          {runs.live ? 'live' : 'paused'}
        </button>
      </div>

      {lightbox && <Lightbox url={lightbox.url} caption={lightbox.caption} onClose={() => setLightbox(null)} />}
    </div>
  )
}

export default RunsPage
