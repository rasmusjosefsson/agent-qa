// web/src/features/editor/useEditor.ts
//
// Coordinates the authoring editor: recording session + step buffer, the ARIA
// element picker, and the live CDP screencast (frames + input forwarding + the
// /api/edit/stream SSE side-channel). Mirrors lib/public/editor.js behavior.

import { useCallback, useEffect, useRef, useState } from 'react'
import { getRoot } from '@/lib/api'
import * as api from '@/lib/editor-api'
import type {
  AriaNode,
  BufferState,
  EditKind,
  LiveInput,
  LiveStatus,
  PickedElement,
  RunResult,
} from './types'
import { recordLabel } from './compose'

type FrameCb = (b64: string) => void

export interface FlashMsg {
  text: string
  error: boolean
}

export function useEditor() {
  const [available, setAvailable] = useState(true)
  const [scenariosRoot, setScenariosRoot] = useState('')
  const [buffer, setBuffer] = useState<BufferState>({ sid: null, intent: null, rows: [] })
  const [ariaNodes, setAriaNodes] = useState<AriaNode[]>([])
  const [interactiveOnly, setInteractiveOnlyState] = useState(true)
  const [liveStatus, setLiveStatus] = useState<LiveStatus>({ text: 'idle', tone: 'idle' })
  const [liveHint, setLiveHint] = useState(
    'Start a recording session to stream the live page here, then type a URL above to navigate.'
  )
  const [liveUrl, setLiveUrl] = useState('')
  const [flashMsg, setFlashMsg] = useState<FlashMsg | null>(null)

  const esRef = useRef<EventSource | null>(null)
  const connectedRef = useRef(false)
  const frameSubs = useRef<Set<FrameCb>>(new Set())
  const snapTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const flashTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const recordQueue = useRef<Promise<unknown>>(Promise.resolve())
  const interactiveRef = useRef(interactiveOnly)
  interactiveRef.current = interactiveOnly

  const refreshBuffer = useCallback(async () => {
    const b = await api.getBuffer()
    setBuffer(b)
    return b
  }, [])

  const flash = useCallback((text: string, error = false) => {
    setFlashMsg({ text, error })
    if (flashTimer.current) clearTimeout(flashTimer.current)
    flashTimer.current = setTimeout(() => {
      setFlashMsg(null)
      void refreshBuffer()
    }, 2500)
  }, [refreshBuffer])

  const snapshot = useCallback(async () => {
    const r = await api.getSnapshot(interactiveRef.current)
    if (!r.ok) {
      setAriaNodes([])
      return
    }
    setAriaNodes(r.nodes)
  }, [])

  const scheduleSnapshot = useCallback(() => {
    if (snapTimer.current) clearTimeout(snapTimer.current)
    snapTimer.current = setTimeout(() => void snapshot(), 700)
  }, [snapshot])

  const enqueueRecord = useCallback((kind: EditKind, payload: Record<string, unknown>) => {
    recordQueue.current = recordQueue.current
      .then(() => api.recordStep(kind, payload))
      .then(() => refreshBuffer())
      .catch(() => {})
  }, [refreshBuffer])

  const connectLive = useCallback(() => {
    if (esRef.current) {
      try {
        esRef.current.close()
      } catch {
        /* ignore */
      }
      esRef.current = null
    }
    connectedRef.current = false
    setLiveStatus({ text: 'connecting…', tone: 'busy' })
    setLiveHint('')
    const es = new EventSource('/api/edit/stream')
    esRef.current = es

    es.onmessage = (ev) => {
      try {
        const f = JSON.parse((ev as MessageEvent).data)
        if (f.data) {
          connectedRef.current = true
          setLiveStatus({ text: 'live', tone: 'ok' })
          frameSubs.current.forEach((cb) => cb(f.data))
        }
      } catch {
        /* keep-alive comment */
      }
    }
    es.addEventListener('bridge-error', (ev) => {
      let msg = 'no live session'
      try {
        msg = JSON.parse((ev as MessageEvent).data).error || msg
      } catch {
        /* ignore */
      }
      setLiveStatus({ text: msg, tone: 'err' })
      setLiveHint(msg + ' — start a session first.')
    })
    es.addEventListener('url', (ev) => {
      try {
        const { url } = JSON.parse((ev as MessageEvent).data)
        if (url) setLiveUrl(url)
        scheduleSnapshot()
      } catch {
        /* ignore */
      }
    })
    es.addEventListener('recordable', (ev) => {
      try {
        const { kind, payload } = JSON.parse((ev as MessageEvent).data)
        if (kind && payload) {
          enqueueRecord(kind, payload)
          flash('recorded ' + recordLabel(payload))
        }
      } catch {
        /* ignore */
      }
    })
    es.addEventListener('buffer-changed', (ev) => {
      try {
        const { payload } = JSON.parse((ev as MessageEvent).data)
        if (payload) flash('recorded ' + recordLabel(payload))
      } catch {
        /* ignore */
      }
      void refreshBuffer()
    })
    es.addEventListener('record-skip', (ev) => {
      try {
        flash(JSON.parse((ev as MessageEvent).data).reason || 'not recorded', true)
      } catch {
        /* ignore */
      }
    })
    es.addEventListener('loaded', () => scheduleSnapshot())
    es.onerror = () => {
      if (!connectedRef.current) setLiveStatus({ text: 'disconnected', tone: 'err' })
    }
  }, [enqueueRecord, flash, refreshBuffer, scheduleSnapshot])

  const subscribeFrame = useCallback((cb: FrameCb) => {
    frameSubs.current.add(cb)
    return () => {
      frameSubs.current.delete(cb)
    }
  }, [])

  const startSession = useCallback(
    async (intent: string, url: string) => {
      const trimmed = intent.trim()
      if (!trimmed) {
        flash('Enter an intent first.', true)
        return false
      }
      const open = url.trim() || 'about:blank'
      const { ok, body } = await api.startSession(trimmed, open)
      if (!ok) {
        flash(body.error || 'start failed', true)
        return false
      }
      flash(`started ${body.sid || ''}`)
      // Bake the entry navigation in as step 0 so a pick-recorded click lands
      // on a real page at replay time.
      if (open && open !== 'about:blank') {
        setLiveUrl(open)
        await api.recordStep('navigation', { route: open })
      }
      await refreshBuffer()
      connectLive()
      await snapshot()
      return true
    },
    [connectLive, flash, refreshBuffer, snapshot]
  )

  const flushScenario = useCallback(async () => {
    const { ok, body } = await api.flush()
    if (!ok) {
      flash(body.error || 'flush failed', true)
      return
    }
    flash(`flushed → ${body.scenarioFile || body.sid || 'scenario.json'}`)
    await refreshBuffer()
  }, [flash, refreshBuffer])

  const cancelScenario = useCallback(async () => {
    const { ok, body } = await api.cancel()
    if (!ok) {
      flash(body.error || 'cancel failed', true)
      return
    }
    flash('recording discarded')
    await refreshBuffer()
  }, [flash, refreshBuffer])

  const moveRow = useCallback(
    async (from: number, to: number) => {
      if (to < 0) return
      await api.moveRow(from, to)
      await refreshBuffer()
    },
    [refreshBuffer]
  )

  const deleteRow = useCallback(
    async (index: number) => {
      await api.deleteRow(index)
      await refreshBuffer()
    },
    [refreshBuffer]
  )

  const runStep = useCallback(
    async (kind: EditKind, payload: Record<string, unknown>): Promise<RunResult> => {
      const { ok, body } = await api.runStep(kind, payload)
      const report = body.result || {}
      scheduleSnapshot()
      return {
        ok: ok && report.ok,
        error: report.error || (!ok ? body.error : null),
        report,
      }
    },
    [scheduleSnapshot]
  )

  const recordStep = useCallback(
    async (kind: EditKind, payload: Record<string, unknown>): Promise<RunResult> => {
      const { ok, body } = await api.recordStep(kind, payload)
      if (!ok) return { ok: false, error: body.error || 'record failed' }
      await refreshBuffer()
      return { ok: true, recorded: true }
    },
    [refreshBuffer]
  )

  const sendInput = useCallback((evt: LiveInput) => api.sendInput(evt), [])

  const pick = useCallback(async (nx: number, ny: number): Promise<PickedElement | null> => {
    const r = await api.pick(nx, ny)
    if (!r.ok || !r.element) {
      if (r.error) flash(r.error, true)
      return null
    }
    return r.element
  }, [flash])

  const setInteractiveOnly = useCallback(
    (v: boolean) => {
      setInteractiveOnlyState(v)
      // snapshot reads the ref; defer so the ref is updated first.
      setTimeout(() => void snapshot(), 0)
    },
    [snapshot]
  )

  const reload = useCallback(() => {
    if (connectedRef.current) api.sendInput({ type: 'reload' })
    else connectLive()
  }, [connectLive])

  // Boot: resolve availability + buffer, reconnect live if a session exists.
  useEffect(() => {
    let mounted = true
    ;(async () => {
      const root = await getRoot()
      if (!mounted) return
      setScenariosRoot(root.scenariosRoot || '')
      if (!root.editor) {
        setAvailable(false)
        return
      }
      setAvailable(true)
      const b = await refreshBuffer()
      if (!mounted) return
      if (b.sid) {
        connectLive()
        void snapshot()
      }
    })()
    return () => {
      mounted = false
      try {
        esRef.current?.close()
      } catch {
        /* ignore */
      }
      if (snapTimer.current) clearTimeout(snapTimer.current)
      if (flashTimer.current) clearTimeout(flashTimer.current)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return {
    available,
    scenariosRoot,
    buffer,
    ariaNodes,
    interactiveOnly,
    liveStatus,
    liveHint,
    liveUrl,
    flashMsg,
    // actions
    refreshBuffer,
    snapshot,
    setInteractiveOnly,
    startSession,
    flushScenario,
    cancelScenario,
    moveRow,
    deleteRow,
    runStep,
    recordStep,
    connectLive,
    reload,
    sendInput,
    pick,
    subscribeFrame,
    flash,
  }
}

export type EditorApi = ReturnType<typeof useEditor>
