import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  Suspense,
  useTransition,
  type PointerEvent as ReactPointerEvent,
} from 'react'
import {
  Loader2Icon,
  PlusIcon,
  XIcon,
  PlugZapIcon,
  CopyIcon,
  CheckIcon,
  SparklesIcon,
  LogInIcon,
  RotateCcwIcon,
  ListIcon,
  FormInputIcon,
  ScanSearchIcon,
  BugIcon,
  MessageSquareTextIcon,
  MonitorPlayIcon,
  FileCheck2Icon,
  type LucideIcon,
} from 'lucide-react'
import { useChat } from './useChat'
import type { ChatItem, ChatUsage, ModelInfo } from '@/lib/types'
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
import {
  getPersonas,
  getEnvironments,
  getChatConnection,
  connectPersonaToChat,
  disconnectChat,
  remediateChatAuth,
  type AuthRemediation,
  type ChatConnection,
  type ConnectResult,
} from '@/lib/run-config-api'
import type { PersonaRecord } from '@/features/personas/types'
import type { EnvironmentRecord } from '@/features/environments/types'
import { prefetchChatState, dropChatState } from '@/lib/resources'
import { replaceRoute, useRoute } from '@/router'

const SPLIT_KEY = 'aqa-chat-split'

// Starter prompts that target the qaplayground demo pages we keep goldens for
// (evals/golden/*), so each one records/replays against a real, stable site.
// Vendor-neutral: qaplayground.com is a public practice site.
const SUGGESTIONS: { title: string; hint: string; icon: LucideIcon; prompt: string }[] = [
  {
    title: 'Record a login flow',
    hint: 'Sign in and assert the dashboard',
    icon: LogInIcon,
    prompt:
      'Record a scenario: open https://qaplayground.com/bank, sign in with username "admin" and password "admin123", click the login button, wait for the URL to be /bank/dashboard, assert the page title reads "SecureBank Dashboard", then save it as bank-login.json.',
  },
  {
    title: 'Replay & summarize',
    hint: 'Re-run the latest scenario',
    icon: RotateCcwIcon,
    prompt: 'Replay the most recent scenario and summarize what passed and what failed.',
  },
  {
    title: 'List scenarios',
    hint: 'See everything with its last status',
    icon: ListIcon,
    prompt: 'List the recorded scenarios in this project with their last run status.',
  },
  {
    title: 'Record a form fill',
    hint: 'Fill, submit and check success',
    icon: FormInputIcon,
    prompt:
      'Record a scenario on https://qaplayground.com/practice/forms: fill in the form fields with valid data and submit it, asserting the success state. Save it as forms.json.',
  },
  {
    title: 'Inspect a page',
    hint: 'ARIA snapshot of the controls',
    icon: ScanSearchIcon,
    prompt:
      'Open https://qaplayground.com/practice/dropdowns and give me an ARIA snapshot of the page\u2019s main landmarks and controls.',
  },
  {
    title: 'Explain a failure',
    hint: 'Find the failing step and why',
    icon: BugIcon,
    prompt: 'Look at the latest replay run and explain why it failed, with the failing step.',
  },
]

const HOW_IT_WORKS: { icon: LucideIcon; label: string }[] = [
  { icon: MessageSquareTextIcon, label: 'Describe it' },
  { icon: MonitorPlayIcon, label: 'Agent drives a browser' },
  { icon: FileCheck2Icon, label: 'Replayable scenario' },
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
  const isDesktop = useMediaQuery('(min-width: 1024px)')
  const [leftPct, setLeftPct] = useState<number>(() => {
    const value = Number(localStorage.getItem(SPLIT_KEY))
    return value >= 25 && value <= 80 ? value : 58
  })
  // `/chat?ask=…` (e.g. the Runs "Ask agent" button) opens a fresh chat seeded
  // with that prompt. Reactive to the SPA route so in-app navigations seed
  // without a document reload; consumed once per ask value.
  const route = useRoute()
  const ask = new URLSearchParams(route.search).get('ask')
  const askRef = useRef<string | null>(null)
  const [seed, setSeed] = useState<{ id: string; prompt: string } | null>(null)
  const [chatsReady, setChatsReady] = useState(false)

  useEffect(() => {
    localStorage.setItem(SPLIT_KEY, String(Math.round(leftPct)))
  }, [leftPct])

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
      const firstId = list[0]?.id ?? null
      if (firstId) prefetchChatState(firstId)
      setChats(list)
      setActiveId((cur) => cur ?? firstId)
      setChatsReady(true)
    })()
    return () => {
      mounted = false
    }
  }, [])

  // Refresh per-chat live/busy badges on a slow poll — the list endpoint only
  // reads flags off already-resolved hubs, so it never spins up an agent.
  useEffect(() => {
    if (!chatsReady || chats.length <= 1) return
    const t = setInterval(async () => {
      const list = await listChats()
      setChats((prev) => {
        if (list.length !== prev.length) return prev // structural changes come from local actions
        return prev.map((c) => {
          const next = list.find((l) => l.id === c.id)
          return next ? { ...c, busy: next.busy, live: next.live } : c
        })
      })
    }, 2500)
    return () => clearInterval(t)
  }, [chatsReady, chats.length])

  // Seed a fresh chat from ?ask= once the chat list exists. Clears the param
  // so refresh doesn't re-seed.
  useEffect(() => {
    if (!ask || !chatsReady) return
    if (askRef.current === ask) return
    askRef.current = ask
    replaceRoute('/chat')
    void (async () => {
      const c = await createChat()
      if (!c) return
      prefetchChatState(c.id)
      setSeed({ id: c.id, prompt: ask })
      setChats((prev) => [...prev, c])
      setActiveId(c.id)
    })()
  }, [ask, chatsReady])

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
      <div className="flex flex-col border-b border-border lg:flex-row">
        <div
          className="flex min-w-0 items-center gap-0.5 overflow-x-auto overflow-y-hidden px-3"
          style={isDesktop ? { flexBasis: `${leftPct}%`, flexGrow: 0, flexShrink: 0 } : undefined}
        >
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
                  title={
                    `session: ${c.session}` +
                    (c.busy ? ' · working…' : c.live ? ' · ready for input' : '')
                  }
                  className="flex max-w-[12rem] items-center gap-1.5 truncate"
                >
                  {(c.busy || c.live) && (
                    <span
                      className={cn(
                        'size-1.5 shrink-0 rounded-full',
                        c.busy ? 'animate-pulse bg-warning' : 'bg-success'
                      )}
                    />
                  )}
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
        {activeId ? <ConnectBar key={activeId} cid={activeId} /> : null}
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
            isDesktop={isDesktop}
            leftPct={leftPct}
            setLeftPct={setLeftPct}
          />
          </Suspense>
        </div>
      ) : chatsReady ? (
        // Boot finished with zero chats — either listChats returned an
        // empty list or the report-server is unreachable. A spinner would
        // sit here forever, so say what happened and offer a retry.
        <div className="flex flex-1 flex-col items-center justify-center gap-3 text-sm text-muted-foreground">
          <p>
            No chats — the report-server may be down. Start it with{' '}
            <code className="rounded bg-muted px-1 py-0.5">agent-qa web</code> and retry.
          </p>
          <button
            type="button"
            onClick={() => window.location.reload()}
            className="rounded-md border px-3 py-1.5 text-foreground transition-colors hover:bg-muted/50"
          >
            Retry
          </button>
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
  isDesktop,
  leftPct,
  setLeftPct,
}: {
  cid: string
  session: string | null
  seedPrompt?: string
  onSeeded?: () => void
  isDesktop: boolean
  leftPct: number
  setLeftPct: (value: number) => void
}) {
  const { state, root, sendPrompt, abort, setModel, setThinking, navigateBrowser } =
    useChat(cid)
  const [text, setText] = useState('')
  const [copied, setCopied] = useState(false)
  const threadRef = useRef<HTMLDivElement | null>(null)
  const atBottomRef = useRef(true)

  // Right-pane tabs (live browser / live recording) + a resizable split.
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [rec, setRec] = useState<RecordingState | null>(null)
  // Transient "Scenario saved" toast, fired when a recording flushes.
  const [savedNotice, setSavedNotice] = useState<{ sid: string; steps: number } | null>(null)
  const prevRec = useRef<RecordingState | null>(null)
  const savedTimer = useRef<number | undefined>(undefined)

  // Poll this chat's recording (cheap file-backed route) for the tab badge and
  // the RecordingView. Resets when the active chat changes.
  useEffect(() => {
    let alive = true
    let timer: number | undefined
    prevRec.current = null
    const tick = async () => {
      const r = await getRecording(cid)
      if (!alive) return
      // Flush transition (was recording this sid, now saved) → toast.
      const p = prevRec.current
      if (r?.flushed && r.sid && p && p.sid === r.sid && !p.flushed) {
        setSavedNotice({ sid: r.sid, steps: (r.steps || []).length })
        window.clearTimeout(savedTimer.current)
        savedTimer.current = window.setTimeout(() => setSavedNotice(null), 6000)
      }
      prevRec.current = r
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
      className="flex min-h-[80svh] min-w-0 flex-1 flex-col lg:min-h-0"
      style={isDesktop ? { flexBasis: `${leftPct}%`, flexGrow: 0, flexShrink: 0 } : undefined}
    >
          {state.hydrated && !state.available && (
            <ChatSetupNotice reason={state.reason} install={state.install} backend={state.backend} />
          )}
          <div className="flex items-center justify-between gap-2 border-b border-border px-2 py-1">
            {/* Which backend is actually under the hood (pi vs opencode), the
                active model, and cumulative usage — answers "am I on the
                cheap model?" at a glance. */}
            <span
              className="min-w-0 truncate px-1 text-[11px] text-muted-foreground"
              title={backendUsageTitle(state.backend, state.model, state.usage)}
            >
              {backendUsageLabel(state.backend, state.model, state.usage)}
            </span>
            <button
              type="button"
              onClick={() => void copyTranscript()}
              disabled={empty}
              title="Copy the whole conversation (markdown) to share"
              className="inline-flex shrink-0 items-center gap-1 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-40"
            >
              {copied ? <CheckIcon className="size-3.5" /> : <CopyIcon className="size-3.5" />}
              {copied ? 'Copied' : 'Copy chat'}
            </button>
          </div>
          <div
            ref={threadRef}
            onScroll={onScroll}
            className="min-h-0 flex-1 space-y-6 overflow-auto px-5 py-4"
          >
            {empty ? (
              <div className="mx-auto flex min-h-full w-full max-w-2xl flex-col items-center justify-center gap-7 py-6 text-center">
                <div className="grid size-12 place-items-center rounded-2xl bg-primary/10 text-primary">
                  <SparklesIcon className="size-6" />
                </div>
                <div>
                  <h2 className="text-foreground text-2xl font-semibold tracking-tight">
                    Chat with your agent-qa agent
                  </h2>
                  <p className="mx-auto mt-2 max-w-md text-[13px] leading-relaxed text-muted-foreground">
                    Same skills, tools, and models as the terminal. Ask it to record a scenario,
                    replay a run, run a command, or explain a failure. When it opens a browser,
                    watch it live on the right.
                  </p>
                </div>
                <ol className="flex flex-wrap items-center justify-center gap-x-2 gap-y-2 text-xs text-muted-foreground">
                  {HOW_IT_WORKS.map((h, i) => (
                    <li key={h.label} className="flex items-center gap-2">
                      <span className="inline-flex items-center gap-1.5 rounded-full border border-border bg-card px-2.5 py-1 shadow-xs">
                        <span className="grid size-4 place-items-center rounded-full bg-primary/12 text-[10px] font-semibold text-primary">
                          {i + 1}
                        </span>
                        <h.icon className="size-3.5 text-foreground/70" />
                        <span className="font-medium text-foreground/80">{h.label}</span>
                      </span>
                      {i < HOW_IT_WORKS.length - 1 && <span className="h-px w-4 bg-border" />}
                    </li>
                  ))}
                </ol>
                <div className="grid w-full grid-cols-1 gap-2.5 text-left sm:grid-cols-2 xl:grid-cols-3">
                  {SUGGESTIONS.map((s) => (
                    <button
                      key={s.title}
                      type="button"
                      title={s.prompt}
                      disabled={!state.available}
                      onClick={() => sendSuggestion(s.prompt)}
                      className="group flex items-start gap-3 rounded-xl border text-left border-border bg-card p-3 shadow-xs transition-all hover:border-primary/35 disabled:pointer-events-none disabled:opacity-50"
                    >
                      <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-accent text-accent-foreground transition-colors group-hover:bg-primary group-hover:text-primary-foreground">
                        <s.icon className="size-4" />
                      </span>
                      <span className="min-w-0">
                        <span className="block text-[13px] font-medium text-foreground">{s.title}</span>
                        <span className="block truncate text-xs text-muted-foreground">{s.hint}</span>
                      </span>
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
    />
  )
  const hasRecording = !!(rec && rec.sid)

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div ref={containerRef} className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto lg:flex-row lg:gap-0 lg:overflow-visible">
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
        <div className="flex h-[60vh] min-h-0 min-w-0 shrink-0 flex-col lg:h-auto lg:flex-1 lg:shrink">
          <div className={cn('min-h-0', hasRecording ? 'flex-[3]' : 'flex-1')}>{liveBrowserPane}</div>
          {hasRecording && (
            <div className="flex min-h-0 flex-[2] flex-col overflow-hidden border-t border-border">
              <RecordingView cid={cid} rec={rec} onChanged={() => void getRecording(cid).then(setRec)} />
            </div>
          )}
        </div>
      </div>

      {savedNotice && (
        <div className="fixed bottom-4 right-4 z-50 flex items-center gap-2 rounded-lg border border-success/30 bg-success/10 px-3 py-2 text-sm text-success shadow-lg backdrop-blur">
          <CheckIcon className="size-4 shrink-0" />
          <span>
            Scenario saved — {savedNotice.steps} step{savedNotice.steps === 1 ? '' : 's'}
          </span>
          <button
            type="button"
            onClick={() => setSavedNotice(null)}
            className="ml-1 opacity-60 hover:opacity-100"
            title="Dismiss"
          >
            <XIcon className="size-3.5" />
          </button>
        </div>
      )}
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
  const [msg, setMsg] = useState<{ tone: 'busy' | 'ok' | 'err'; text: string; remediation?: AuthRemediation } | null>(null)

  const showConnectResult = (result: ConnectResult, afterPreparing = false) => {
    setMsg(
      result.authenticated
        ? { tone: 'ok', text: `Signed in as ${result.profile} — this chat's browser is authenticated.` }
        : {
            tone: 'err',
            // A prepare that ran and still left us signed out must not repeat
            // "needs preparation" — that reads as if nothing happened and
            // invites pressing the same button forever.
            text: !result.remediation
              ? "Connect ran but the profile isn't authenticated yet."
              : afterPreparing
                ? 'Preparation ran, but sign-in still failed.'
                : 'Sign-in needs preparation.',
            remediation: result.remediation,
          }
    )
  }

  // "No sign-in" picked — drop any persona binding and stop the background
  // auto-connect for this chat; the poll loop then shows the guest state.
  const optOut = async () => {
    setPersonaId('')
    setEnvId('')
    setMsg({ tone: 'ok', text: 'No sign-in — browsing anonymously.' })
    try {
      await disconnectChat(cid)
    } catch {
      /* best-effort; the connection poll re-shows whatever state remains */
    }
  }

  useEffect(() => {
    let alive = true
    let timer: number | undefined
    let shownPersonaId: string | null = null
    let shownEnvironmentId: string | null = null
    // Guest is folded into the state key so a disconnect→connect cycle that
    // returns to the same state still re-renders its message.
    let shownKey: string | null = null

    const showConnection = (connection: ChatConnection) => {
      if (
        connection.personaId &&
        (connection.personaId !== shownPersonaId || connection.environmentId !== shownEnvironmentId)
      ) {
        shownPersonaId = connection.personaId
        shownEnvironmentId = connection.environmentId
        setPersonaId(connection.personaId)
        if (connection.environmentId) setEnvId(connection.environmentId)
      }
      const key = `${connection.state}:${connection.guest ? 'guest' : ''}`
      if (key === shownKey) return
      shownKey = key
      if (connection.guest) {
        setBusy(false)
        setMsg({ tone: 'ok', text: 'No sign-in — browsing anonymously.' })
      } else if (connection.state === 'connecting') {
        setBusy(true)
        setMsg({ tone: 'busy', text: 'Signing in…' })
      } else if (connection.state === 'connected') {
        setBusy(false)
        setMsg({ tone: 'ok', text: `Signed in as ${connection.profile}.` })
      } else if (connection.state === 'failed') {
        setBusy(false)
        setMsg({
          tone: 'err',
          text: connection.remediation ? 'Sign-in needs preparation.' : 'Automatic sign-in failed. Press Connect to retry.',
          remediation: connection.remediation,
        })
      }
    }

    const refreshConnection = async () => {
      try {
        const connection = await getChatConnection(cid)
        if (alive) showConnection(connection)
      } catch {
        /* connection status is optional */
      }
    }

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
    void refreshConnection()
    timer = window.setInterval(refreshConnection, 1000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [cid])

  if (personas.length === 0) return null

  const connect = async () => {
    if (!personaId || busy) return
    setBusy(true)
    setMsg(null)
    try {
      showConnectResult(await connectPersonaToChat(cid, personaId, envId || undefined))
    } catch (e) {
      setMsg({ tone: 'err', text: e instanceof Error ? e.message : String(e) })
    } finally {
      setBusy(false)
    }
  }

  const remediate = async () => {
    if (!personaId || busy) return
    setBusy(true)
    setMsg({ tone: 'busy', text: 'Preparing sign-in…' })
    try {
      showConnectResult(await remediateChatAuth(cid, personaId, envId || undefined), true)
    } catch (e) {
      setMsg({ tone: 'err', text: e instanceof Error ? e.message : String(e) })
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex max-w-full flex-1 flex-wrap items-center gap-x-2 gap-y-1 border-t border-border bg-background px-3 py-1 text-xs text-muted-foreground lg:border-l lg:border-t-0">
      <span>Sign in as</span>
      <ChatSelect
        value={personaId}
        onChange={(v) => {
          if (v) {
            setPersonaId(v)
          } else {
            void optOut()
          }
        }}
        placeholder="No sign-in"
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
      {msg?.remediation && (
        <Button variant="ghost" size="sm" className="h-7 px-2 text-xs" onClick={() => void remediate()} disabled={busy || !personaId}>
          {msg.remediation.label}
        </Button>
      )}
      {msg && (
        <span
          role="status"
          className={cn(
            'text-[11px]',
            msg.tone === 'busy' && 'text-warning',
            msg.tone === 'ok' && 'text-success',
            msg.tone === 'err' && 'text-destructive'
          )}
        >
          {msg.text}
        </span>
      )}
    </div>
  )
}

// Chat agent isn't set up — a calm setup nudge with a copy-paste install
// CTA, not an error. The server names the backend + install command; the
// pi command stays as fallback for older servers.
const PI_INSTALL_CMD = 'npm i -g @earendil-works/pi-coding-agent'
const BACKEND_LABELS: Record<string, string> = { pi: 'pi', opencode: 'opencode' }

function fmtTokens(n?: number | null): string {
  if (n == null || !Number.isFinite(n)) return ''
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return String(n)
}

function fmtCost(c?: number | null): string {
  if (c == null || !Number.isFinite(c)) return ''
  return `$${c < 0.01 ? c.toFixed(4) : c.toFixed(2)}`
}

// One-line "who's under the hood" for the chat header: backend · model · usage.
function backendUsageLabel(backend?: string, model?: ModelInfo, usage?: ChatUsage | null): string {
  const parts: string[] = []
  if (backend) parts.push(BACKEND_LABELS[backend] || backend)
  if (model) parts.push(model.label || model.id)
  const cost = fmtCost(usage?.cost)
  const total = usage?.tokens?.total ?? ((usage?.tokens?.input || 0) + (usage?.tokens?.output || 0))
  if (cost) parts.push(cost)
  else if (total) parts.push(`${fmtTokens(total)} tok`)
  return parts.join(' · ')
}

function backendUsageTitle(backend?: string, model?: ModelInfo, usage?: ChatUsage | null): string {
  const bits = [
    backend ? `backend: ${BACKEND_LABELS[backend] || backend}` : null,
    model ? `model: ${model.provider ? `${model.provider}/` : ''}${model.id}` : null,
    usage?.tokens?.total != null ? `tokens: ${usage.tokens.total.toLocaleString()}` : null,
    usage?.cost != null ? `cost: ${fmtCost(usage.cost)}` : null,
  ].filter(Boolean)
  return bits.length ? bits.join('\n') : 'chat backend'
}

function ChatSetupNotice({
  reason,
  install,
  backend,
}: {
  reason?: string
  install?: string
  backend?: string
}) {
  const [copied, setCopied] = useState(false)
  const installCmd = install || PI_INSTALL_CMD
  const backendLabel = (backend && BACKEND_LABELS[backend]) || 'the chat agent'

  const copyInstall = async () => {
    try {
      await navigator.clipboard.writeText(installCmd)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    } catch {
      /* clipboard blocked — user can still read the command */
    }
  }

  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 border-b border-border bg-muted/30 px-4 py-2.5">
      <PlugZapIcon className="size-4 shrink-0 text-muted-foreground" />
      <div className="min-w-0 flex-1 basis-56">
        <div className="text-[13px] font-semibold tracking-tight">Finish chat setup</div>
        <div
          className="truncate text-xs text-muted-foreground"
          title={reason || `${backendLabel} needs its agent runtime installed.`}
        >
          {reason || `${backendLabel} needs its agent runtime installed.`}
        </div>
      </div>
      <button
        type="button"
        onClick={() => void copyInstall()}
        title="Copy the install command"
        className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-card px-2.5 py-1.5 font-mono text-[11px] text-muted-foreground shadow-sm transition-colors hover:text-foreground"
      >
        {copied ? <CheckIcon className="size-3.5" /> : <CopyIcon className="size-3.5" />}
        {copied ? 'Copied' : installCmd}
      </button>
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
      className="h-7 rounded-lg border border-border bg-card px-2 text-xs font-medium text-foreground shadow-xs outline-none transition-colors hover:border-primary/30 focus-visible:border-primary/50 focus-visible:ring-3 focus-visible:ring-primary/10"
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
