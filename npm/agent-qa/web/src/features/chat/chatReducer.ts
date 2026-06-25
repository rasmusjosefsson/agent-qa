// web/src/features/chat/chatReducer.ts
//
// Pure reducer that mirrors lib/public/chat.js:onAgentEvent. It folds the live
// AgentEvent stream (and rehydrated saved messages) into a single ordered
// `items` array so live streaming and rehydration produce the same interleaved
// layout (user / assistant text / thinking / tool cards in event order).
//
// Kept free of React so it can be unit-tested directly.

import type {
  AgentEvent,
  ChatItem,
  ChatMessage,
  ChatState,
  ModelInfo,
  SessionEvent,
} from '@/lib/types';

export interface ChatUIState {
  available: boolean;
  // False until the first state load resolves; gates the "unavailable" banner
  // so it never flashes while a freshly-mounted chat is still hydrating.
  hydrated: boolean;
  reason?: string;
  streaming: boolean;
  items: ChatItem[];
  model?: ModelInfo;
  models: ModelInfo[];
  thinkingLevel?: string;
  thinkingLevels: string[];
  sessionId?: string;
  sessionNote?: string;
  // Streaming cursors: indices into `items` for the currently-open assistant
  // text / thinking bubbles (null = closed, next delta opens a fresh bubble).
  curAssistant: number | null;
  curThinking: number | null;
  // toolCallId -> index into `items`.
  toolIndex: Record<string, number>;
  nextId: number;
}

export const initialState: ChatUIState = {
  available: false,
  hydrated: false,
  streaming: false,
  items: [],
  models: [],
  thinkingLevels: [],
  curAssistant: null,
  curThinking: null,
  toolIndex: {},
  nextId: 1,
};

export type Action =
  | { type: 'init_state'; payload: ChatState }
  | { type: 'patch_meta'; payload: Partial<ChatState> }
  | { type: 'agent_event'; event: AgentEvent }
  | { type: 'session_event'; event: SessionEvent }
  | { type: 'add_user'; text: string }
  | { type: 'add_error'; message: string }
  | { type: 'set_streaming'; on: boolean }
  | { type: 'new_chat' };

// ---------- formatting helpers (shared with the view) ----------

export function fmtArgs(args: unknown): string {
  if (args == null) return '';
  if (typeof args === 'string') return args;
  try {
    return JSON.stringify(args, null, 2);
  } catch {
    return String(args);
  }
}

export function resultText(result: unknown): string {
  if (result == null) return '';
  const r = result as { content?: unknown };
  if (Array.isArray(r.content)) {
    return (r.content as Array<{ type?: string; text?: string }>)
      .filter((c) => c && (c.type === 'text' || typeof c.text === 'string'))
      .map((c) => c.text || '')
      .join('');
  }
  if (typeof result === 'string') return result;
  try {
    return JSON.stringify(result, null, 2);
  } catch {
    return String(result);
  }
}

// ---------- immutable item helpers ----------

function closeBubbles(state: ChatUIState): ChatUIState {
  if (state.curAssistant == null && state.curThinking == null) return state;
  return { ...state, curAssistant: null, curThinking: null };
}

function ensureAssistant(state: ChatUIState): ChatUIState {
  if (state.curAssistant != null) return state;
  const id = state.nextId;
  const items = [...state.items, { kind: 'assistant', id, text: '' } as ChatItem];
  return { ...state, items, curAssistant: items.length - 1, nextId: id + 1 };
}

function appendAssistantText(state: ChatUIState, delta?: string): ChatUIState {
  if (!delta) return state;
  const s = ensureAssistant(state);
  const idx = s.curAssistant!;
  const items = s.items.slice();
  const cur = items[idx];
  if (cur.kind === 'assistant') items[idx] = { ...cur, text: cur.text + delta };
  return { ...s, items };
}

function ensureThinking(state: ChatUIState): ChatUIState {
  if (state.curThinking != null) return state;
  const id = state.nextId;
  const items = [...state.items, { kind: 'thinking', id, text: '' } as ChatItem];
  return { ...state, items, curThinking: items.length - 1, nextId: id + 1 };
}

function appendThinking(state: ChatUIState, delta?: string): ChatUIState {
  if (!delta) return state;
  const s = ensureThinking(state);
  const idx = s.curThinking!;
  const items = s.items.slice();
  const cur = items[idx];
  if (cur.kind === 'thinking') items[idx] = { ...cur, text: cur.text + delta };
  return { ...s, items };
}

function makeToolCard(
  state: ChatUIState,
  name?: string,
  args?: unknown,
  toolCallId?: string
): ChatUIState {
  // classic closes streaming bubbles when a tool card opens, so subsequent
  // assistant text starts a fresh bubble below the card.
  const id = state.nextId;
  const items = [
    ...state.items,
    { kind: 'tool', id, toolCallId, name, args, out: '', status: 'running' } as ChatItem,
  ];
  const toolIndex = { ...state.toolIndex };
  if (toolCallId) toolIndex[toolCallId] = items.length - 1;
  return {
    ...state,
    items,
    toolIndex,
    nextId: id + 1,
    curAssistant: null,
    curThinking: null,
  };
}

function updateToolOut(state: ChatUIState, toolCallId?: string, text?: string): ChatUIState {
  if (!toolCallId || !text) return state;
  const idx = state.toolIndex[toolCallId];
  if (idx == null) return state;
  const items = state.items.slice();
  const cur = items[idx];
  if (cur.kind === 'tool') items[idx] = { ...cur, out: text };
  return { ...state, items };
}

function endToolCard(
  state: ChatUIState,
  toolCallId?: string,
  isError?: boolean,
  result?: unknown,
  name?: string
): ChatUIState {
  let s = state;
  let idx = toolCallId != null ? s.toolIndex[toolCallId] : undefined;
  if (idx == null) {
    s = makeToolCard(s, name, null, toolCallId);
    idx = toolCallId != null ? s.toolIndex[toolCallId] : s.items.length - 1;
  }
  const items = s.items.slice();
  const cur = items[idx];
  if (cur.kind === 'tool') {
    const text = resultText(result);
    items[idx] = {
      ...cur,
      status: isError ? 'err' : 'ok',
      out: text || cur.out,
      name: cur.name || name,
    };
  }
  return { ...s, items };
}

function addError(state: ChatUIState, message: string): ChatUIState {
  const id = state.nextId;
  const s = closeBubbles(state);
  return {
    ...s,
    items: [...s.items, { kind: 'error', id, text: String(message) } as ChatItem],
    nextId: id + 1,
  };
}

function addUser(state: ChatUIState, text: string): ChatUIState {
  const id = state.nextId;
  return {
    ...state,
    items: [...state.items, { kind: 'user', id, text } as ChatItem],
    nextId: id + 1,
  };
}

// ---------- rehydration ----------

function userText(content: unknown): string {
  if (typeof content === 'string') return content;
  if (Array.isArray(content)) {
    return (content as Array<{ type?: string; text?: string }>)
      .filter((c) => c && c.type === 'text')
      .map((c) => c.text || '')
      .join('');
  }
  return '';
}

function renderSaved(state: ChatUIState, m: ChatMessage): ChatUIState {
  if (!m || typeof m !== 'object') return state;
  if (m.role === 'user') {
    const text = userText(m.content);
    return text ? addUser(state, text) : state;
  }
  if (m.role === 'assistant') {
    let s = state;
    for (const part of m.content || []) {
      if (!part) continue;
      if (part.type === 'text' && part.text) {
        s = appendAssistantText(ensureAssistant(s), part.text);
        s = closeBubbles(s);
      } else if (part.type === 'thinking' && part.thinking) {
        s = appendThinking(ensureThinking(s), part.thinking);
        s = closeBubbles(s);
      } else if (part.type === 'toolCall') {
        s = makeToolCard(s, part.name, part.arguments, part.id);
      }
    }
    return s;
  }
  if (m.role === 'toolResult') {
    return endToolCard(state, m.toolCallId, !!m.isError, m, m.toolName);
  }
  return state;
}

export function rehydrate(payload: ChatState): ChatUIState {
  let s: ChatUIState = {
    ...initialState,
    available: !!payload.available,
    hydrated: true,
    reason: payload.reason,
    model: payload.model,
    models: payload.models || [],
    thinkingLevel: payload.thinkingLevel,
    thinkingLevels: payload.thinkingLevels || [],
    sessionId: payload.sessionId,
    streaming: !!payload.streaming,
  };
  for (const m of payload.messages || []) s = renderSaved(s, m);
  if (payload.streamingMessage) s = renderSaved(s, payload.streamingMessage);
  return closeBubbles(s);
}

// ---------- reducer ----------

export function reducer(state: ChatUIState, action: Action): ChatUIState {
  switch (action.type) {
    case 'init_state':
      return rehydrate(action.payload);

    case 'patch_meta': {
      const p = action.payload;
      return {
        ...state,
        available: p.available != null ? !!p.available : state.available,
        model: p.model ?? state.model,
        models: p.models ?? state.models,
        thinkingLevel: p.thinkingLevel ?? state.thinkingLevel,
        thinkingLevels: p.thinkingLevels ?? state.thinkingLevels,
        reason: p.reason ?? state.reason,
      };
    }

    case 'add_user':
      return addUser(state, action.text);

    case 'add_error':
      return addError(state, action.message);

    case 'set_streaming':
      return { ...state, streaming: action.on };

    case 'new_chat':
      return {
        ...state,
        items: [],
        toolIndex: {},
        curAssistant: null,
        curThinking: null,
        streaming: false,
        sessionNote: undefined,
      };

    case 'session_event': {
      const note =
        action.event.type === 'session_idle'
          ? 'Session idled out — your next message starts a fresh one.'
          : undefined;
      return { ...closeBubbles(state), streaming: false, sessionNote: note };
    }

    case 'agent_event': {
      const ev = action.event;
      switch (ev.type) {
        case 'agent_start':
          return { ...state, streaming: true, sessionNote: undefined };
        case 'agent_end':
          return closeBubbles({ ...state, streaming: false });
        case 'turn_start':
          return state;
        case 'turn_end':
          return closeBubbles(state);
        case 'message_start':
          return closeBubbles(state);
        case 'message_end':
          return closeBubbles(state);
        case 'message_update': {
          const a = ev.assistantMessageEvent;
          if (!a) return state;
          if (a.type === 'text_start') return ensureAssistant(state);
          if (a.type === 'text_delta') return appendAssistantText(state, a.delta || '');
          if (a.type === 'thinking_delta') return appendThinking(state, a.delta || '');
          if (a.type === 'error' && a.error && a.error.errorMessage) {
            return addError(state, a.error.errorMessage);
          }
          return state;
        }
        case 'tool_execution_start':
          return makeToolCard(state, ev.toolName, ev.args, ev.toolCallId);
        case 'tool_execution_update':
          return updateToolOut(state, ev.toolCallId, resultText(ev.partialResult));
        case 'tool_execution_end':
          return endToolCard(state, ev.toolCallId, !!ev.isError, ev.result, ev.toolName);
        case 'error':
          return { ...addError(state, ev.message || 'agent error'), streaming: false };
        default:
          return state;
      }
    }

    default:
      return state;
  }
}
