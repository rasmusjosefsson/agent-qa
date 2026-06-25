import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from 'react'
import { Loader2Icon, PlusIcon, XIcon } from 'lucide-react'
import { useChat } from './useChat'
import BrowserPane from './BrowserPane'
import { Message } from './components/Message'
import { PromptInput } from './components/PromptInput'
import RecordingView from './components/RecordingView'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { useMediaQuery } from '@/lib/useMediaQuery'
import {
  listChats,
  createChat,
  deleteChat,
  getRecording,
  type ChatMeta,
  type RecordingState,
} from '@/lib/api'

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

// Top-level Chat tab: owns the open chats + which one is active, renders the
// switcher, and mounts one ChatConversation keyed by the active id (so React
// remounts — and useChat re-hydrates — on every switch). Each chat has its own
// conversation history AND its own agent-browser session (see report-server's
// chat manager), so chats run side-by-side without sharing a browser.
export function ChatPage() {
  const [chats, setChats] = useState<ChatMeta[]>([])
  const [activeId, setActiveId] = useState<string | null>(null)

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
      setChats(list)
      setActiveId((cur) => cur ?? list[0]?.id ?? null)
    })()
    return () => {
      mounted = false
    }
  }, [])

  const onNew = async () => {
    const c = await createChat()
    if (!c) return
    setChats((cs) => [...cs, c])
    setActiveId(c.id)
  }

  const onDelete = async (id: string) => {
    if (chats.length <= 1) return // always keep at least one chat
    await deleteChat(id)
    const remaining = chats.filter((c) => c.id !== id)
    setChats(remaining)
    if (id === activeId) setActiveId(remaining[0]?.id ?? null)
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      {/* Chat switcher — tabs */}
      <div className="flex items-center border-b border-border">
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
                  onClick={() => setActiveId(c.id)}
                  title={`session: ${c.session}`}
                  className="max-w-[12rem] truncate"
                >
                  {c.title && c.title !== 'New chat' ? c.title : `Chat ${i + 1}`}
                </button>
                {chats.length > 1 && (
                  <button
                    type="button"
                    onClick={() => void onDelete(c.id)}
                    aria-label="Close chat"
                    className={cn(
                      'rounded p-0.5 text-muted-foreground/50 transition-opacity hover:bg-muted hover:text-foreground',
                      isActive ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'
                    )}
                  >
                    <XIcon className="size-3.5" />
                  </button>
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
        <ChatConversation key={activeId} cid={activeId} />
      ) : (
        <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
          <Loader2Icon className="mr-2 size-4 animate-spin" /> Loading chats…
        </div>
      )}
    </div>
  )
}

// One conversation: bound to a single chat id. Re-mounted on switch via `key`.
function ChatConversation({ cid }: { cid: string }) {
  const { state, root, sendPrompt, abort, newChat, setModel, setThinking, navigateBrowser } =
    useChat(cid)
  const [text, setText] = useState('')
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

  const metaParts: string[] = []
  if (state.model?.label || state.model?.id) metaParts.push(state.model.label || state.model.id!)
  if (state.thinkingLevel && state.thinkingLevel !== 'off')
    metaParts.push('thinking: ' + state.thinkingLevel)

  const conversationColumn = (
    <div
      className="flex min-h-0 min-w-0 flex-1 flex-col gap-2"
      style={isDesktop ? { flexBasis: `${leftPct}%`, flexGrow: 0, flexShrink: 0 } : undefined}
    >
          <div className="flex items-center justify-between gap-3 px-1">
            {state.hydrated ? (
              <div className="truncate text-xs text-muted-foreground">{metaParts.join(' · ')}</div>
            ) : (
              <div className="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
                <Loader2Icon className="size-3.5 animate-spin" /> Connecting…
              </div>
            )}
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void newChat()}
              disabled={!state.available || empty}
              title="Clear this conversation and start fresh in a new browser"
            >
              Reset
            </Button>
          </div>

          <div
            ref={threadRef}
            onScroll={onScroll}
            className="min-h-0 flex-1 space-y-6 overflow-auto rounded-xl border border-border bg-card/20 p-5"
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
                      className="rounded-full border border-border bg-card px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-50"
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
                    <Loader2Icon className="size-4 animate-spin" />
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
    <BrowserPane available={!!root.liveBrowser} chatId={cid} navigate={navigateBrowser} />
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
          className="group hidden shrink-0 cursor-col-resize px-1.5 lg:flex lg:items-stretch"
          title="Drag to resize"
        >
          <div className="w-px bg-border transition-colors group-hover:bg-primary" />
        </div>

        {/* Right column: live browser, with the recording steps stacked
            underneath once a recording exists (the browser letterboxes, so the
            leftover vertical space goes to the steps). */}
        <div className="flex h-[60vh] min-h-0 min-w-0 flex-col gap-3 lg:h-auto lg:flex-1">
          <div className={cn('min-h-0', hasRecording ? 'flex-[3]' : 'flex-1')}>{liveBrowserPane}</div>
          {hasRecording && (
            <div className="flex min-h-0 flex-[2] flex-col overflow-hidden rounded-xl border border-border bg-card/20">
              <RecordingView cid={cid} rec={rec} />
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

export default ChatPage
