// web/src/features/runs/RunsPage.tsx
import { useState } from 'react'
import { useRuns } from './useRuns'
import { ScenarioSidebar } from './components/ScenarioSidebar'
import { CenterPane } from './components/CenterPane'
import { StepDetail } from './components/StepDetail'
import { ReplayLive } from './components/ReplayLive'
import { Lightbox } from './components/Lightbox'
import { isRunLive } from './rows'
import { Panel, PanelGroup } from 'react-resizable-panels'
import { ResizeHandle } from '@/components/ResizeHandle'

export function RunsPage() {
  const runs = useRuns()
  const [lightbox, setLightbox] = useState<{ url: string; caption: string } | null>(null)
  const [replayErr, setReplayErr] = useState('')

  const onReplay = async (sid: string) => {
    setReplayErr('')
    const res = await runs.replay(sid)
    if (!res.ok) setReplayErr(`Replay did not start: ${res.error}`)
  }

  // Right pane: live screencast while the run is in flight and no step pinned,
  // otherwise the static step detail (which renders its own empty state).
  const showLive = isRunLive(runs.detail) && runs.sel.stepIdx == null && runs.sel.sid

  return (
    <div className="flex min-h-0 flex-1 flex-col">
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
          <CenterPane runs={runs} onReplay={(sid) => void onReplay(sid)} />
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
