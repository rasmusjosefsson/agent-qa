// web/src/features/runs/RunsPage.tsx
import { useState } from 'react'
import { useRuns } from './useRuns'
import { ScenarioSidebar } from './components/ScenarioSidebar'
import { CenterPane } from './components/CenterPane'
import { StepDetail } from './components/StepDetail'
import { ReplayLive } from './components/ReplayLive'
import { Lightbox } from './components/Lightbox'
import { isRunLive } from './rows'

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
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex h-5 items-center justify-between">
        <div className="truncate text-xs text-muted-foreground">{runs.root}</div>
        <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <input type="checkbox" checked={runs.live} onChange={(e) => runs.setLive(e.target.checked)} />
          live
        </label>
      </div>

      {replayErr && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-1.5 text-xs text-destructive">{replayErr}</div>
      )}

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 lg:grid-cols-[18rem_minmax(0,1fr)_22rem]">
        <div className="min-h-0">
          <ScenarioSidebar runs={runs} />
        </div>
        <div className="min-h-0">
          <CenterPane runs={runs} onReplay={(sid) => void onReplay(sid)} />
        </div>
        <div className="min-h-0">
          {showLive ? (
            <ReplayLive sid={runs.sel.sid!} onLightbox={(url, caption) => setLightbox({ url, caption })} />
          ) : (
            <StepDetail runs={runs} onLightbox={(url, caption) => setLightbox({ url, caption })} />
          )}
        </div>
      </div>

      {lightbox && <Lightbox url={lightbox.url} caption={lightbox.caption} onClose={() => setLightbox(null)} />}
    </div>
  )
}

export default RunsPage
