import { useState } from 'react'
import { useEditor } from './useEditor'
import { SessionBox } from './components/SessionBox'
import { StepList } from './components/StepList'
import { Composer } from './components/Composer'
import { LiveCanvas } from './components/LiveCanvas'
import { ElementPicker } from './components/ElementPicker'
import { composePayload } from './compose'
import { EMPTY_FORM, type AriaNode, type ClickMode, type ComposeForm, type PickedElement, type RunResult } from './types'
import { cn } from '@/lib/utils'

const PICK_VERBS = ['click', 'type', 'assertPresent', 'assertAbsent']

export function EditorPage() {
  const ed = useEditor()
  const [form, setForm] = useState<ComposeForm>(EMPTY_FORM)
  const [pickedHint, setPickedHint] = useState('')
  const [clickMode, setClickMode] = useState<ClickMode>('interact')
  const [runResult, setRunResult] = useState<RunResult | null>(null)

  const onField = (field: keyof ComposeForm, value: string) =>
    setForm((f) => ({ ...f, [field]: value }))

  // Pick from the ARIA tree: keep an assert verb if the user is mid-assert.
  const applyTreePick = (n: AriaNode) => {
    setForm((f) => ({
      ...f,
      verb: PICK_VERBS.includes(f.verb) ? f.verb : 'click',
      role: n.role || '',
      name: n.name || '',
    }))
    setPickedHint(`picked ${n.role} “${n.name}”${n.ref ? ` (ref ${n.ref})` : ''}`)
  }

  // Pick from the live canvas: always a click step, ready to Record.
  const applyCanvasPick = (el: PickedElement) => {
    setForm((f) => ({ ...f, verb: 'click', role: el.role || '', name: el.name || '' }))
    setPickedHint(`picked ${el.role || '?'} “${el.name || ''}” — Record step to keep it`)
    ed.flash(`picked ${el.role} “${el.name}”`)
  }

  const onRun = async () => {
    const c = composePayload(form)
    if ('error' in c) return setRunResult({ ok: false, error: c.error })
    setRunResult(await ed.runStep(c.kind, c.payload))
  }

  const onRecord = async () => {
    const c = composePayload(form)
    if ('error' in c) return setRunResult({ ok: false, error: c.error })
    const res = await ed.recordStep(c.kind, c.payload)
    setRunResult(res)
    if (res.recorded) setPickedHint('')
  }

  if (!ed.available) {
    return (
      <div className="flex min-h-0 flex-1 flex-col gap-3">
        <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-300">
          Editor unavailable — the agent-qa CLI binary could not be resolved. Launch via{' '}
          <code className="font-mono">agent-qa report view</code> with the platform package installed, or set{' '}
          <code className="font-mono">AGENT_QA_BINARY_PATH</code>.
        </div>
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex h-5 items-center justify-between">
        <div className="truncate text-xs text-muted-foreground">{ed.scenariosRoot}</div>
        {ed.flashMsg && (
          <div className={cn('text-xs', ed.flashMsg.error ? 'text-destructive' : 'text-emerald-400')}>
            {ed.flashMsg.text}
          </div>
        )}
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 lg:grid-cols-[20rem_minmax(0,1fr)_18rem]">
        {/* left: session + steps + composer */}
        <div className="flex min-h-0 flex-col gap-3 overflow-auto">
          <SessionBox
            buffer={ed.buffer}
            onStart={(intent, url) => void ed.startSession(intent, url)}
            onFlush={() => void ed.flushScenario()}
            onCancel={() => void ed.cancelScenario()}
          />
          <div className="rounded-lg border border-border bg-card p-2">
            <div className="mb-1 flex items-center justify-between px-1">
              <span className="text-xs font-medium text-muted-foreground">Steps</span>
              <button
                type="button"
                title="Refresh"
                onClick={() => void ed.refreshBuffer()}
                className="grid h-5 w-5 place-items-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
              >
                ⟳
              </button>
            </div>
            <StepList rows={ed.buffer.rows} onMove={ed.moveRow} onDelete={ed.deleteRow} />
          </div>
          <Composer
            form={form}
            onField={onField}
            pickedHint={pickedHint}
            onRun={() => void onRun()}
            onRecord={() => void onRecord()}
            runResult={runResult}
          />
        </div>

        {/* center: live browser */}
        <div className="min-h-0">
          <LiveCanvas
            subscribeFrame={ed.subscribeFrame}
            sendInput={ed.sendInput}
            pick={ed.pick}
            reload={ed.reload}
            connectLive={ed.connectLive}
            liveUrl={ed.liveUrl}
            liveStatus={ed.liveStatus}
            liveHint={ed.liveHint}
            clickMode={clickMode}
            onClickModeChange={setClickMode}
            onCanvasPick={applyCanvasPick}
          />
        </div>

        {/* right: element picker */}
        <div className="min-h-0">
          <ElementPicker
            nodes={ed.ariaNodes}
            interactiveOnly={ed.interactiveOnly}
            onInteractiveChange={ed.setInteractiveOnly}
            onSnapshot={() => void ed.snapshot()}
            onPick={applyTreePick}
          />
        </div>
      </div>
    </div>
  )
}

export default EditorPage
