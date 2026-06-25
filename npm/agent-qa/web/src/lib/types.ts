// web/src/lib/types.ts

// Minimal TypeScript types mirroring the classic chat.js shapes. Keep permissive
// where the backend can vary (tool results, args, etc.).

export type AssistantMessageEvent =
  | { type: 'text_start' }
  | { type: 'text_delta'; delta: string }
  | { type: 'thinking_delta'; delta: string }
  | { type: 'error'; error: { errorMessage?: string } };

export type AgentEvent =
  | { type: 'agent_start' }
  | { type: 'agent_end' }
  | { type: 'turn_start' }
  | { type: 'turn_end' }
  | { type: 'message_start' }
  | { type: 'message_end' }
  | { type: 'message_update'; assistantMessageEvent?: AssistantMessageEvent }
  | { type: 'tool_execution_start'; toolName?: string; args?: any; toolCallId?: string }
  | { type: 'tool_execution_update'; toolCallId?: string; partialResult?: any }
  | { type: 'tool_execution_end'; toolCallId?: string; isError?: boolean; result?: any; toolName?: string }
  | { type: 'error'; message?: string };

export type SessionEvent = { type: 'session_reset' | 'session_idle' };

export type AssistantContentPart =
  | { type: 'text'; text: string }
  | { type: 'thinking'; thinking: string }
  | { type: 'toolCall'; id?: string; name?: string; arguments?: any };

export type ChatMessage =
  | { role: 'user'; content: string | Array<{ type: 'text'; text: string }> }
  | { role: 'assistant'; content: AssistantContentPart[] }
  | { role: 'toolResult'; toolCallId?: string; toolName?: string; isError?: boolean; content?: any };

export interface ModelInfo {
  provider?: string;
  id: string;
  label?: string;
}

// Ordered, render-ready items. Mirrors the DOM order the classic chat.js builds
// in the thread (user / assistant text / thinking / tool cards interleaved in
// event order) so live streaming and rehydration produce identical layouts.
export type ToolStatus = 'running' | 'ok' | 'err';

export type ChatItem =
  | { kind: 'user'; id: number; text: string }
  | { kind: 'assistant'; id: number; text: string }
  | { kind: 'thinking'; id: number; text: string }
  | {
      kind: 'tool';
      id: number;
      toolCallId?: string;
      name?: string;
      args?: unknown;
      out?: string;
      status: ToolStatus;
    }
  | { kind: 'error'; id: number; text: string };

export interface RootInfo {
  scenariosRoot?: string;
  editor?: boolean;
  chat?: boolean;
  liveBrowser?: boolean;
}

export interface ChatState {
  available: boolean;
  started?: boolean;
  streaming?: boolean;
  messages?: ChatMessage[];
  streamingMessage?: ChatMessage;
  model?: ModelInfo;
  models?: ModelInfo[];
  thinkingLevel?: string;
  thinkingLevels?: string[];
  sessionId?: string;
  reason?: string;
}
