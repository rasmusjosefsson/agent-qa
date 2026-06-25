// web/src/features/chat/useChat.ts

import { useCallback, useEffect, useReducer, useRef, useState } from 'react'
import type { AgentEvent, SessionEvent, RootInfo } from '@/lib/types'
import {
  getRoot,
  getChatState,
  createChatEventSource,
  postPrompt,
  postAbort,
  postNew,
  postModel,
  postThinking,
  browserNavigate as apiBrowserNavigate,
} from '@/lib/api'
import { reducer, initialState } from './chatReducer'

export function useChat(cid: string | null) {
  const [state, dispatch] = useReducer(reducer, initialState)
  const [root, setRoot] = useState<RootInfo>({})
  const esRef = useRef<EventSource | null>(null)
  const streamingRef = useRef(false)
  streamingRef.current = state.streaming

  const loadState = useCallback(async () => {
    if (!cid) return null
    const s = await getChatState(cid)
    dispatch({ type: 'init_state', payload: s })
    return s
  }, [cid])

  // Boot: load root info + this chat's state, then open its live event stream.
  // Keyed on `cid` so selecting another chat re-hydrates from that chat's hub.
  useEffect(() => {
    if (!cid) return
    let mounted = true
    ;(async () => {
      const [r, s] = await Promise.all([getRoot(), getChatState(cid)])
      if (!mounted) return
      setRoot(r)
      dispatch({ type: 'init_state', payload: s })
      if (s.available === false) return

      const es = createChatEventSource(cid)
      esRef.current = es
      es.addEventListener('agent', (e) => {
        try {
          dispatch({ type: 'agent_event', event: JSON.parse((e as MessageEvent).data) as AgentEvent })
        } catch {
          /* ignore malformed frame */
        }
      })
      es.addEventListener('session', (e) => {
        try {
          dispatch({ type: 'session_event', event: JSON.parse((e as MessageEvent).data) as SessionEvent })
        } catch {
          /* ignore */
        }
      })
      es.onerror = () => {
        // EventSource auto-reconnects; nothing to do.
      }
    })()

    return () => {
      mounted = false
      try {
        esRef.current?.close()
      } catch {
        /* ignore */
      }
    }
  }, [cid])

  const sendPrompt = useCallback(async (text: string) => {
    if (!cid) return
    const t = text.trim()
    if (!t) return
    dispatch({ type: 'add_user', text: t })
    const { ok, status, body } = await postPrompt(cid, t, streamingRef.current ? 'steer' : undefined)
    if (!ok && status !== 202) {
      dispatch({ type: 'add_error', message: body?.error || `prompt failed (${status})` })
    }
  }, [cid])

  const abort = useCallback(async () => {
    if (!cid) return
    await postAbort(cid)
    dispatch({ type: 'set_streaming', on: false })
  }, [cid])

  const newChat = useCallback(async () => {
    if (!cid) return
    const res = await postNew(cid)
    if (res.ok) dispatch({ type: 'new_chat' })
  }, [cid])

  const setModel = useCallback(async (provider?: string, id?: string) => {
    if (!cid) return
    const { ok, body } = await postModel(cid, provider, id)
    if (ok) dispatch({ type: 'patch_meta', payload: body })
    else if (body?.error) dispatch({ type: 'add_error', message: body.error })
  }, [cid])

  const setThinking = useCallback(async (level?: string) => {
    if (!cid) return
    const { ok, body } = await postThinking(cid, level)
    if (ok) dispatch({ type: 'patch_meta', payload: body })
    else if (body?.error) dispatch({ type: 'add_error', message: body.error })
  }, [cid])

  const navigateBrowser = useCallback(async (payload: unknown) => {
    return apiBrowserNavigate(payload)
  }, [])

  return {
    state,
    root,
    sendPrompt,
    abort,
    newChat,
    setModel,
    setThinking,
    navigateBrowser,
    reloadState: loadState,
  }
}
