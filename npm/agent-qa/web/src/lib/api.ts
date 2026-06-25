// web/src/lib/api.ts

import type { ChatState, RootInfo } from './types';

export interface ChatMeta {
  id: string;
  title: string;
  createdAt: number;
  session: string;
}

export async function getRoot(): Promise<RootInfo> {
  try {
    const r = await fetch('/api/root', { headers: { accept: 'application/json' } });
    const body = await r.json().catch(() => ({}));
    return (body as RootInfo) || {};
  } catch {
    return {};
  }
}

// ----- multi-chat lifecycle -----

export async function listChats(): Promise<ChatMeta[]> {
  try {
    const r = await fetch('/api/chat/list', { headers: { accept: 'application/json' } });
    const body = await r.json().catch(() => ({}));
    return Array.isArray((body as { chats?: ChatMeta[] }).chats)
      ? (body as { chats: ChatMeta[] }).chats
      : [];
  } catch {
    return [];
  }
}

export async function createChat(): Promise<ChatMeta | null> {
  try {
    const r = await fetch('/api/chat/create', { method: 'POST' });
    if (!r.ok) return null;
    return (await r.json()) as ChatMeta;
  } catch {
    return null;
  }
}

export async function deleteChat(cid: string): Promise<void> {
  try {
    await fetch(`/api/chat/c/${cid}/delete`, { method: 'POST' });
  } catch {
    /* best-effort */
  }
}

// ----- per-chat routes (/api/chat/c/<cid>/*) -----

const chatBase = (cid: string) => `/api/chat/c/${encodeURIComponent(cid)}`;

export async function getChatState(cid: string): Promise<ChatState> {
  try {
    const r = await fetch(`${chatBase(cid)}/state`, { headers: { accept: 'application/json' } });
    const body = await r.json().catch(() => ({}));
    return (body as ChatState) || { available: false };
  } catch {
    return { available: false } as ChatState;
  }
}

export function createChatEventSource(cid: string): EventSource {
  return new EventSource(`${chatBase(cid)}/stream`);
}

export async function postPrompt(
  cid: string,
  text: string,
  streamingBehavior?: string
): Promise<{ ok: boolean; status: number; body: { error?: string } }> {
  const r = await fetch(`${chatBase(cid)}/prompt`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ text, streamingBehavior }),
  });
  const body = await r.json().catch(() => ({}));
  return { ok: r.ok, status: r.status, body: body as { error?: string } };
}

export async function postAbort(cid: string): Promise<Response> {
  return fetch(`${chatBase(cid)}/abort`, { method: 'POST' });
}

export async function postNew(cid: string): Promise<Response> {
  return fetch(`${chatBase(cid)}/new`, { method: 'POST' });
}

export async function postModel(
  cid: string,
  provider?: string,
  id?: string
): Promise<{ ok: boolean; body: ChatState & { error?: string } }> {
  const r = await fetch(`${chatBase(cid)}/model`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ provider, id }),
  });
  const body = await r.json().catch(() => ({}));
  return { ok: r.ok, body: body as ChatState & { error?: string } };
}

export async function postThinking(
  cid: string,
  level?: string
): Promise<{ ok: boolean; body: ChatState & { error?: string } }> {
  const r = await fetch(`${chatBase(cid)}/thinking`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ level }),
  });
  const body = await r.json().catch(() => ({}));
  return { ok: r.ok, body: body as ChatState & { error?: string } };
}

export async function browserNavigate(payload: unknown): Promise<Response> {
  return fetch('/api/chat/browser-navigate', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload || {}),
  });
}

// ----- live recording view (per-chat) -----

export interface RecordingStep {
  stepIndex: number;
  stepId: string;
  kind: 'navigation' | 'action' | 'wait' | 'assert' | string;
  payload: Record<string, unknown>;
  intent?: string | null;
  recordedAt?: string | null;
}

export interface RecordingState {
  recording: boolean;
  sid: string | null;
  intent: string | null;
  session: string | null;
  startedAt: string | null;
  baseline: string | null;
  flushed: boolean;
  steps: RecordingStep[];
}

const EMPTY_RECORDING: RecordingState = {
  recording: false,
  sid: null,
  intent: null,
  session: null,
  startedAt: null,
  baseline: null,
  flushed: false,
  steps: [],
};

export async function getRecording(cid: string): Promise<RecordingState> {
  try {
    const r = await fetch(`${chatBase(cid)}/recording`, { headers: { accept: 'application/json' } });
    if (!r.ok) return EMPTY_RECORDING;
    return (await r.json()) as RecordingState;
  } catch {
    return EMPTY_RECORDING;
  }
}

export function recordingArtifactUrl(
  cid: string,
  stepId: string,
  kind: 'screenshot' | 'snapshot'
): string {
  return `${chatBase(cid)}/recording/step/${encodeURIComponent(stepId)}/${kind}`;
}
