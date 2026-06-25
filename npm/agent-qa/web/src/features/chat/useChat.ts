// web/src/features/chat/useChat.ts

import { use, useCallback, useEffect, useReducer, useRef } from 'react'
import type { AgentEvent, SessionEvent } from '@/lib/types'
import {
  createChatEventSource,
  postPrompt,
  postAbort,
  postNew,
  postModel,
  postThinking,
  browserNavigate as apiBrowserNavigate,
} from '@/lib/api'
import { chatStateResource, rootResource } from '@/lib/resources'
import { reducer, rehydrate } from './chatReducer'

// `cid` is non-null: ChatPage only mounts ChatConversation for an active chat.
// Boot data (root info + this chat's saved state) is read with `use()`, so the
// hook *suspends* until both resolve instead of rendering a half-built
// "Connecting…" state. ChatPage's Suspense boundary handles the wait, and a
// startTransition-driven switch keeps the previous chat visible meanwhile — so
// switching chats (and "New") never flashes a remount spinner.
export function useChat(cid: string) {
  const root = use(rootResource())
  // Seed the reducer straight from the resolved state, so the conversation is
  // hydrated on its very first render — there's no false→true `hydrated` flip.
  const initial = use(chatStateResource(cid))
  const [state, dispatch] = useReducer(reducer, initial, rehydrate)
  const esRef = useRef<EventSource | null>(null)
  const streamingRef = useRef(false)
  streamingRef.current = state.streaming

  // Open this chat's live event stream once it's known to be available. Keyed
  // on `cid`; the seeded initial state already carries the saved history.
  useEffect(() => {
    if (!initial.available) return
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
    return () => {
      try {
        es.close()
      } catch {
        /* ignore */
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cid])

  const sendPrompt = useCallback(async (text: string) => {
    const t = text.trim()
    if (!t) return
    dispatch({ type: 'add_user', text: t })
    const { ok, status, body } = await postPrompt(cid, t, streamingRef.current ? 'steer' : undefined)
    if (!ok && status !== 202) {
      dispatch({ type: 'add_error', message: body?.error || `prompt failed (${status})` })
    }
  }, [cid])

  const abort = useCallback(async () => {
    await postAbort(cid)
    dispatch({ type: 'set_streaming', on: false })
  }, [cid])

  const newChat = useCallback(async () => {
    const res = await postNew(cid)
    if (res.ok) dispatch({ type: 'new_chat' })
  }, [cid])

  const setModel = useCallback(async (provider?: string, id?: string) => {
    const { ok, body } = await postModel(cid, provider, id)
    if (ok) dispatch({ type: 'patch_meta', payload: body })
    else if (body?.error) dispatch({ type: 'add_error', message: body.error })
  }, [cid])

  const setThinking = useCallback(async (level?: string) => {
    const { ok, body } = await postThinking(cid, level)
    if (ok) dispatch({ type: 'patch_meta', payload: body })
    else if (body?.error) dispatch({ type: 'add_error', message: body.error })
  }, [cid])

  const navigateBrowser = useCallback(async (payload: unknown) => {
    return apiBrowserNavigate(payload)
  }, [])

  return { state, root, sendPrompt, abort, newChat, setModel, setThinking, navigateBrowser }
}
