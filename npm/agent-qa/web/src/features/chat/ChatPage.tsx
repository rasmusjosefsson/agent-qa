import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  Suspense,
  useTransition,
  type PointerEvent as ReactPointerEvent,
} from 'react'
import { Loader2Icon, PlusIcon, XIcon, PlugZapIcon, CopyIcon, CheckIcon } from 'lucide-react'
import { useChat } from './useChat'
import type { ChatItem } from '@/lib/types'
import { WorkingIndicator } from '@/components/working-indicator'
import BrowserPane from './BrowserPane'
import { Message } from './components/Message'
import { PromptInput } from './components/PromptInput'
import RecordingView from './components/RecordingView'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { cn } from '@/lib/utils'
import { useMediaQuery } from '@/lib/useMediaQuery'
import { Button } from '@/components/ui/button'
import {
  listChats,
  createChat,
  deleteChat,
  getRecording,
  type ChatMeta,
  type RecordingState,
} from '@/lib/api'
import { getPersonas, getEnvironments, connectPersonaToChat } from '@/lib/run-config-api'
import type { PersonaRecord } from '@/features/personas/types'
import type { EnvironmentRecord } from '@/features/environments/types'
import { prefetchChatState, dropChatState } from '@/lib/resources'

const SPLIT_KEY = 'aqa-chat-split'

// Starter prompts that target the qaplayground demo pages we keep goldens for
// (evals/golden/*), so each one records/replays against a real, stable site.
// Vendor-neutral: qaplayground.com is a public practice site.
const SUGGESTIONS: { title: string; prompt: string }[] = [
  {
    title: 'Record a login flow',
    prompt:
      'Record a scenario: open https://qaplayground.com/bank, sign in with username "admin" and password "admin123", click the login button, wait for the URL to be /bank/dashboard, assert the page title reads "SecureBank Dashboard", then save it as bank-login.json.',
  },
  {
    title: 'Replay & summarize',
    prompt: 'Replay the most recent scenario and summarize what passed and what failed.',
  },
  {
    title: 'List scenarios',
    prompt: 'List the recorded scenarios in this project with their last run status.',
  },
  {
    title: 'Record a form fill',
    prompt:
      'Record a scenario on https://qaplayground.com/practice/forms: fill in the form fields with valid data and submit it, asserting the success state. Save it as forms.json.',
  },
  {
    title: 'Inspect a page',
    prompt:
      'Open https://qaplayground.com/practice/dropdowns and give me an ARIA snapshot of the page\u2019s main landmarks and controls.',
  },
  {
    title: 'Explain a failure',
    prompt: 'Look at the latest replay run and explain why it failed, with the failing step.',
  },
]

// Serialize the conversation to markdown for the clipboard, so the user can
// paste a chat back to us when something goes wrong. Thinking bubbles are
// dropped (noise); tool calls + their output are kept (they're the useful part
// when diagnosing what the agent did).
function transcriptToText(items: ChatItem[]): string {
  const blocks: string[] = []
  for (const it of items) {
    if (it.kind === 'user') {
      blocks.push(`## User\n\n${it.text}`)
    } else if (it.kind === 'assistant') {
      if (it.text.trim()) blocks.push(`## Assistant\n\n${it.text}`)
    } else if (it.kind === 'tool') {
      const args =
        it.args == null ? '' : typeof it.args === 'string' ? it.args : JSON.stringify(it.args)
      const out = (it.out || '').trim()
      blocks.push(
        `### ${it.name || 'tool'} (${it.status})` +
          (args ? `\nargs: ${args}` : '') +
          (out ? `\n\n\`\`\`\n${out}\n\`\`\`` : '')
      )
    } else if (it.kind === 'error') {
      blocks.push(`### error\n\n${it.text}`)
    }
  }
  return blocks.join('\n\n')
}

// Top-level Chat tab: owns the open chats + which one is active, renders the
// switcher, and mounts one ChatConversation keyed by the active id (so React
// remounts — and useChat re-hydrates — on every switch). Each chat has its own
// conversation history AND its own agent-browser session (see report-server's
// chat manager), so chats run side-by-side without sharing a browser.
export function ChatPage() {
  const [chats, setChats] = useState<ChatMeta[]>([])
  const [activeId, setActiveId] = useState<string | null>(null)
  const [isPending, startTransition] = useTransition()
  // `/chat?ask=…` (e.g. the Runs "Ask agent" button) opens a fresh chat seeded
  // with that prompt. Consumed once on mount.
  const askRef = useRef<string | null>(
    typeof window !== 'undefined' ? new URLSearchParams(window.location.search).get('ask') : null
  )
  const [seed, setSeed] = useState<{ id: string; prompt: string } | null>(null)

  useEffect(() => {
    let mounted = true
    ;(async () => {
      let list = await listChats()
      if (!mounted) return
      if (list.length === 0) {
        const c = await createChat()
        if (!mounted) return
        list = c ? [c] : []
      }
      const ask = askRef.current
      if (ask) {
        askRef.current = null
        window.history.replaceState({}, '', window.location.pathname)
        const c = await createChat()
        if (!mounted) return
        if (c) {
          prefetchChatState(c.id)
          setSeed({ id: c.id, prompt: ask })
          setChats([...list, c])
          setActiveId(c.id)
          return
        }
      }
      const firstId = list[0]?.id ?? null
      if (firstId) prefetchChatState(firstId)
      setChats(list)
      setActiveId((cur) => cur ?? firstId)
    })()
    return () => {
      mounted = false
    }
  }, [])

  const onNew = async () => {
    const c = await createChat()
    if (!c) return
    prefetchChatState(c.id)
    setChats((cs) => [...cs, c])
    startTransition(() => setActiveId(c.id))
  }

  const onDelete = async (id: string) => {
    if (chats.length <= 1) return // always keep at least one chat
    await deleteChat(id)
    const remaining = chats.filter((c) => c.id !== id)
    setChats(remaining)
    if (id === activeId) {
      // Still mounted until the transition commits — let the switch drop it;
      // evicting its resource here would re-suspend the outgoing view.
      startTransition(() => setActiveId(remaining[0]?.id ?? null))
    } else {
      dropChatState(id) // not mounted — safe to evict now
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* Chat switcher — tabs */}
      <div className="flex items-center border-b border-border px-3">
        <div className="flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto">
          {chats.map((c, i) => {
            const isActive = c.id === activeId
            return (
              <div
                key={c.id}
                className={cn(
                  'group relative -mb-px flex shrink-0 items-center gap-1 border-b-2 pl-3 pr-2 py-2 text-sm transition-colors',
                  isActive
                    ? 'border-primary text-foreground'
                    : 'border-transparent text-muted-foreground hover:text-foreground'
                )}
              >
                <button
                  type="button"
                  onClick={() => startTransition(() => setActiveId(c.id))}
                  title={`session: ${c.session}`}
                  className="max-w-[12rem] truncate"
                >
                  {c.title && c.title !== 'New chat' ? c.title : `Chat ${i + 1}`}
                </button>
                {chats.length > 1 && (
                  <AlertDialog>
                    <AlertDialogTrigger asChild>
                      <button
                        type="button"
                        aria-label="Close chat"
                        className={cn(
                          'rounded p-0.5 text-muted-foreground/50 transition-opacity hover:bg-muted hover:text-foreground',
                          isActive ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'
                        )}
                      >
                        <XIcon className="size-3.5" />
                      </button>
                    </AlertDialogTrigger>
                    <AlertDialogContent>
                      <AlertDialogHeader>
                        <AlertDialogTitle>Close this chat?</AlertDialogTitle>
                        <AlertDialogDescription>
                          This permanently deletes the conversation and closes its browser session.
                        </AlertDialogDescription>
                      </AlertDialogHeader>
                      <AlertDialogFooter>
                        <AlertDialogCancel>Cancel</AlertDialogCancel>
                        <AlertDialogAction onClick={() => void onDelete(c.id)}>Delete</AlertDialogAction>
                      </AlertDialogFooter>
                    </AlertDialogContent>
                  </AlertDialog>
                )}
              </div>
            )
          })}
          <button
            type="button"
            onClick={() => void onNew()}
            title="New chat"
            className="ml-1 inline-flex shrink-0 items-center gap-1 rounded-md px-2 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground"
          >
            <PlusIcon className="size-4" />
            New
          </button>
        </div>
      </div>

      {activeId ? (
        <div
          className={cn(
            'flex min-h-0 flex-1 flex-col transition-opacity',
            isPending && 'pointer-events-none opacity-60'
          )}
        >
          <Suspense
            fallback={
              <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
                <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading chat…
              </div>
            }
          >
            <ChatConversation
            key={activeId}
            cid={activeId}
            session={chats.find((c) => c.id === activeId)?.session ?? null}
            seedPrompt={seed && seed.id === activeId ? seed.prompt : undefined}
            onSeeded={() => setSeed(null)}
          />
          </Suspense>
        </div>
      ) : (
        <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
          <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading chats…
        </div>
      )}
    </div>
  )
}

// One conversation: bound to a single chat id. Re-mounted on switch via `key`.
function ChatConversation({
  cid,
  session,
  seedPrompt,
  onSeeded,
}: {
  cid: string
  session: string | null
  seedPrompt?: string
  onSeeded?: () => void
}) {
  const { state, root, sendPrompt, abort, setModel, setThinking, navigateBrowser } =
    useChat(cid)
  const [text, setText] = useState('')
  const [copied, setCopied] = useState(false)
  const threadRef = useRef<HTMLDivElement | null>(null)
  const atBottomRef = useRef(true)

  // Right-pane tabs (live browser / live recording) + a resizable split.
  const containerRef = useRef<HTMLDivElement | null>(null)
  const isDesktop = useMediaQuery('(min-width: 1024px)')
  const [leftPct, setLeftPct] = useState<number>(() => {
    const v = Number(localStorage.getItem(SPLIT_KEY))
    return v >= 25 && v <= 80 ? v : 58
  })
  const [rec, setRec] = useState<RecordingState | null>(null)

  useEffect(() => {
    localStorage.setItem(SPLIT_KEY, String(Math.round(leftPct)))
  }, [leftPct])

  // Poll this chat's recording (cheap file-backed route) for the tab badge and
  // the RecordingView. Resets when the active chat changes.
  useEffect(() => {
    let alive = true
    let timer: number | undefined
    const tick = async () => {
      const r = await getRecording(cid)
      if (!alive) return
      setRec(r)
      timer = window.setTimeout(tick, 1500)
    }
    void tick()
    return () => {
      alive = false
      if (timer) window.clearTimeout(timer)
    }
  }, [cid])

  const onDividerDown = (e: ReactPointerEvent) => {
    e.preventDefault()
    const container = containerRef.current
    if (!container) return
    const rect = container.getBoundingClientRect()
    const onMove = (ev: PointerEvent) => {
      const pct = ((ev.clientX - rect.left) / rect.width) * 100
      setLeftPct(Math.min(80, Math.max(25, pct)))
    }
    const onUp = () => {
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
      document.body.style.userSelect = ''
      document.body.style.cursor = ''
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'col-resize'
  }

  const onScroll = () => {
    const el = threadRef.current
    if (!el) return
    atBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 120
  }

  useLayoutEffect(() => {
    const el = threadRef.current
    if (el && atBottomRef.current) el.scrollTop = el.scrollHeight
  }, [state.items, state.streaming])

  // Seed prompt (from /chat?ask=…) — send once, as soon as the agent is available.
  const seededRef = useRef(false)
  useEffect(() => {
    if (seededRef.current || !seedPrompt || !state.available) return
    seededRef.current = true
    atBottomRef.current = true
    void sendPrompt(seedPrompt)
    onSeeded?.()
  }, [seedPrompt, state.available, sendPrompt])

  const empty = state.items.length === 0

  const activeThinkingId =
    state.curThinking != null && state.items[state.curThinking]?.kind === 'thinking'
      ? state.items[state.curThinking].id
      : null

  const tail = state.items[state.items.length - 1]
  const liveText = tail?.kind === 'assistant' && tail.text.length > 0
  const liveThinking = tail?.kind === 'thinking' && tail.id === activeThinkingId
  const showWorking = state.streaming && !liveText && !liveThinking

  const submit = () => {
    const t = text.trim()
    if (!t || !state.available) return
    setText('')
    atBottomRef.current = true
    void sendPrompt(t)
  }

  const sendSuggestion = (prompt: string) => {
    if (!state.available) return
    atBottomRef.current = true
    void sendPrompt(prompt)
  }

  const copyTranscript = async () => {
    if (empty) return
    try {
      await navigator.clipboard.writeText(transcriptToText(state.items))
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    } catch {
      /* clipboard blocked — nothing we can do */
    }
  }

  const conversationColumn = (
    <div
      className="flex min-h-0 min-w-0 flex-1 flex-col"
      style={isDesktop ? { flexBasis: `${leftPct}%`, flexGrow: 0, flexShrink: 0 } : undefined}
    >
          <div className="flex items-center justify-end border-b border-border px-2 py-1">
            <button
              type="button"
              onClick={() => void copyTranscript()}
              disabled={empty}
              title="Copy the whole conversation (markdown) to share"
              className="inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-40"
            >
              {copied ? <CheckIcon className="size-3.5" /> : <CopyIcon className="size-3.5" />}
              {copied ? 'Copied' : 'Copy chat'}
            </button>
          </div>
          <div
            ref={threadRef}
            onScroll={onScroll}
            className="min-h-0 flex-1 space-y-6 overflow-auto px-4 py-3"
          >
            {empty ? (
              <div className="flex h-full flex-col items-center justify-center gap-4 text-center">
                <div>
                  <div className="text-base font-semibold">Chat with your agent-qa agent</div>
                  <p className="mx-auto mt-1 max-w-md text-sm text-muted-foreground">
                    Same skills, tools, and models as the terminal. Ask it to record a scenario,
                    replay a run, run a command, or explain a failure. When it opens a browser,
                    watch it live on the right.
                  </p>
                </div>
                <div className="flex flex-wrap justify-center gap-2">
                  {SUGGESTIONS.map((s) => (
                    <button
                      key={s.title}
                      type="button"
                      title={s.prompt}
                      disabled={!state.available}
                      onClick={() => sendSuggestion(s.prompt)}
                      className="rounded-sm border border-border bg-transparent px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-50"
                    >
                      {s.title}
                    </button>
                  ))}
                </div>
              </div>
            ) : (
              <>
                {state.items.map((item) => (
                  <Message
                    key={item.id}
                    item={item}
                    thinkingStreaming={item.kind === 'thinking' && item.id === activeThinkingId}
                  />
                ))}
                {showWorking && (
                  <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <WorkingIndicator />
                    <span className="aqa-shimmer">Working…</span>
                  </div>
                )}
              </>
            )}
          </div>

          <div className="space-y-1">
            <PromptInput
              value={text}
              onChange={setText}
              onSubmit={submit}
              onAbort={() => void abort()}
              available={state.available}
              streaming={state.streaming}
              models={state.models}
              model={state.model}
              onModel={(p, id) => void setModel(p, id)}
              thinkingLevel={state.thinkingLevel}
              thinkingLevels={state.thinkingLevels}
              onThinking={(lvl) => void setThinking(lvl)}
            />
            {state.sessionNote ? (
              <div className="px-1 pt-1 text-xs text-muted-foreground">{state.sessionNote}</div>
            ) : null}
          </div>
        </div>
  )

  const liveBrowserPane = (
    <BrowserPane
      available={!!root.liveBrowser}
      chatId={cid}
      navigate={navigateBrowser}
      initialSession={session ?? undefined}
      footer={<ConnectBar cid={cid} />}
    />
  )
  const hasRecording = !!(rec && rec.sid)

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      {state.hydrated && !state.available && (
        <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-300">
          Chat unavailable{state.reason ? ` — ${state.reason}` : ''}. Install the pi SDK
          (<code className="font-mono">@earendil-works/pi-coding-agent</code>) or set{' '}
          <code className="font-mono">AGENT_QA_PI_SDK</code>.
        </div>
      )}

      <div ref={containerRef} className="flex min-h-0 flex-1 flex-col gap-4 lg:flex-row lg:gap-0">
        {conversationColumn}

        {/* Resizer (desktop only) */}
        <div
          onPointerDown={onDividerDown}
          role="separator"
          aria-orientation="vertical"
          className="group relative hidden w-px shrink-0 cursor-col-resize bg-border transition-colors hover:bg-primary lg:block"
          title="Drag to resize"
        >
          {/* invisible wider grab zone over the flush 1px line */}
          <span className="absolute inset-y-0 -left-1.5 -right-1.5" />
        </div>

        {/* Right column: live browser, with the recording steps stacked
            underneath once a recording exists (the browser letterboxes, so the
            leftover vertical space goes to the steps). */}
        <div className="flex h-[60vh] min-h-0 min-w-0 flex-col lg:h-auto lg:flex-1">
          <div className={cn('min-h-0', hasRecording ? 'flex-[3]' : 'flex-1')}>{liveBrowserPane}</div>
          {hasRecording && (
            <div className="flex min-h-0 flex-[2] flex-col overflow-hidden border-t border-border">
              <RecordingView cid={cid} rec={rec} />
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

// Sign a persona into THIS chat's own browser session, so the agent operates an
// already-authenticated page (no credentials in the agent's hands). Hidden when
// no personas exist — keeps the chat clean for users who don't need auth.
function ConnectBar({ cid }: { cid: string }) {
  const [personas, setPersonas] = useState<PersonaRecord[]>([])
  const [environments, setEnvironments] = useState<EnvironmentRecord[]>([])
  const [personaId, setPersonaId] = useState('')
  const [envId, setEnvId] = useState('')
  const [busy, setBusy] = useState(false)
  const [msg, setMsg] = useState<{ tone: 'ok' | 'err'; text: string } | null>(null)

  useEffect(() => {
    let alive = true
    ;(async () => {
      try {
        const [pe, en] = await Promise.all([getPersonas(), getEnvironments()])
        if (!alive) return
        setPersonas(pe.personas)
        setEnvironments(en.environments)
        if (pe.personas.length === 1) setPersonaId(pe.personas[0].id)
      } catch {
        /* personas optional — bar stays hidden */
      }
    })()
    return () => {
      alive = false
    }
  }, [])

  if (personas.length === 0) return null

  const connect = async () => {
    if (!personaId || busy) return
    setBusy(true)
    setMsg(null)
    try {
      const r = await connectPersonaToChat(cid, personaId, envId || undefined)
      setMsg(
        r.authenticated
          ? { tone: 'ok', text: `Signed in as ${r.profile} — this chat's browser is authenticated.` }
          : {
              tone: 'err',
              text: `Connect ran but ${r.profile} isn't authenticated yet (check the auth plugin / \`vault login\`).`,
            }
      )
    } catch (e) {
      setMsg({ tone: 'err', text: e instanceof Error ? e.message : String(e) })
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex flex-wrap items-center gap-x-2 gap-y-1 border-t border-border bg-background px-2 py-1.5 text-xs text-muted-foreground">
      <span>Sign in as</span>
      <ChatSelect
        value={personaId}
        onChange={setPersonaId}
        placeholder="persona"
        options={personas.map((p) => ({ value: p.id, label: p.name }))}
      />
      {environments.length > 0 && (
        <>
          <span>on</span>
          <ChatSelect
            value={envId}
            onChange={setEnvId}
            placeholder="default environment"
            options={environments.map((e) => ({ value: e.id, label: e.name }))}
          />
        </>
      )}
      <Button
        variant="ghost"
        size="sm"
        className="h-7 px-2 text-xs"
        onClick={() => void connect()}
        disabled={busy || !personaId}
        title="Sign this persona into this chat's browser session (resolve credentials → profile-bootstrap)"
      >
        {busy ? <Loader2Icon className="size-3.5 animate-spin" /> : <PlugZapIcon className="size-3.5" />} Connect
      </Button>
      {msg && (
        <span className={cn('text-[11px]', msg.tone === 'ok' ? 'text-emerald-400' : 'text-destructive')}>
          {msg.text}
        </span>
      )}
    </div>
  )
}

function ChatSelect({
  value,
  onChange,
  placeholder,
  options,
}: {
  value: string
  onChange: (v: string) => void
  placeholder: string
  options: { value: string; label: string }[]
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="rounded-md border border-border bg-background px-2 py-1 text-xs text-foreground outline-none focus-visible:border-ring"
    >
      <option value="">{placeholder}</option>
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  )
}

export default ChatPage
