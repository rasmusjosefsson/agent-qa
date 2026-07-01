// web/src/features/runs/RunsPage.tsx
import { useEffect, useRef, useState } from 'react'
import { useRuns } from './useRuns'
import { ScenarioSidebar } from './components/ScenarioSidebar'
import { CenterPane } from './components/CenterPane'
import { StepDetail } from './components/StepDetail'
import { ReplayLive } from './components/ReplayLive'
import { Lightbox } from './components/Lightbox'
import { isRunLive } from './rows'
import { Panel, PanelGroup } from 'react-resizable-panels'
import { ResizeHandle } from '@/components/ResizeHandle'
import { getPersonas, getEnvironments } from '@/lib/run-config-api'
import type { PersonaRecord } from '@/features/personas/types'
import type { EnvironmentRecord } from '@/features/environments/types'

export function RunsPage() {
  const runs = useRuns()
  const [lightbox, setLightbox] = useState<{ url: string; caption: string } | null>(null)
  const [replayErr, setReplayErr] = useState('')

  // Optional run config: which persona (login) / environment (target) a replay
  // uses. The ids ride along so the server resolves the persona's credentials
  // and self-bootstraps the login — an auth-walled scenario re-authenticates.
  const [personas, setPersonas] = useState<PersonaRecord[]>([])
  const [environments, setEnvironments] = useState<EnvironmentRecord[]>([])
  const [personaId, setPersonaId] = useState('')
  const [envId, setEnvId] = useState('')

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
    d.status?.state !== 'done'
  )
  const busy = viewedRunActive || replayingSid === runs.sel.sid
  useEffect(() => {
    if (viewedRunActive && replayingSid === runs.sel.sid) setReplayingSid(null)
  }, [viewedRunActive, replayingSid, runs.sel.sid])

  const onReplay = async (sid: string) => {
    setReplayErr('')
    setReplayingSid(sid)
    const persona = personas.find((p) => p.id === personaId)
    const env = environments.find((e) => e.id === envId)
    const params: Record<string, string> = env ? { ...env.params } : {}
    if (env?.baseUrl) params.baseUrl = env.baseUrl
    const res = await runs.replay(sid, {
      profile: persona?.profile || undefined,
      params: Object.keys(params).length ? params : undefined,
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

  const hasRunConfig = personas.length > 0 || environments.length > 0

  // Right pane: live screencast while the run is in flight and no step pinned,
  // otherwise the static step detail (which renders its own empty state).
  const showLive = isRunLive(runs.detail) && runs.sel.stepIdx == null && runs.sel.sid

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {hasRunConfig && (
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 border-b border-border px-4 py-2 text-xs text-muted-foreground">
          <span>Replay as</span>
          <select
            value={personaId}
            onChange={(e) => setPersonaId(e.target.value)}
            className="rounded-md border border-border bg-background px-2 py-1 text-xs text-foreground outline-none focus-visible:border-ring"
          >
            <option value="">default login</option>
            {personas.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          <span>on</span>
          <select
            value={envId}
            onChange={(e) => setEnvId(e.target.value)}
            className="rounded-md border border-border bg-background px-2 py-1 text-xs text-foreground outline-none focus-visible:border-ring"
          >
            <option value="">default environment</option>
            {environments.map((en) => (
              <option key={en.id} value={en.id}>
                {en.name}
              </option>
            ))}
          </select>
        </div>
      )}
      {replayErr && (
        <div className="border-b border-destructive/30 bg-destructive/10 px-4 py-1.5 text-xs text-destructive">{replayErr}</div>
      )}

      {/* Resizable full-bleed columns — drag the dividers to resize. */}
      <PanelGroup direction="horizontal" autoSaveId="aqa-runs-cols" className="min-h-0 flex-1">
        <Panel defaultSize={22} minSize={14} className="min-h-0">
          <ScenarioSidebar runs={runs} />
        </Panel>
        <ResizeHandle />
        <Panel defaultSize={56} minSize={30} className="min-h-0">
          <CenterPane runs={runs} onReplay={(sid) => void onReplay(sid)} busy={busy} />
        </Panel>
        <ResizeHandle />
        <Panel defaultSize={22} minSize={16} className="min-h-0">
          {showLive ? (
            <ReplayLive sid={runs.sel.sid!} onLightbox={(url, caption) => setLightbox({ url, caption })} />
          ) : (
            <StepDetail runs={runs} onLightbox={(url, caption) => setLightbox({ url, caption })} />
          )}
        </Panel>
      </PanelGroup>

      {/* Bottom status stripe — scenarios root + live toggle */}
      <div className="flex h-7 shrink-0 items-center justify-between gap-3 border-t border-border px-4 text-xs text-muted-foreground">
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
              (runs.live ? 'bg-emerald-400 animate-pulse' : 'bg-muted-foreground/40')
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
