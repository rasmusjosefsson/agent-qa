// web/src/lib/editor-api.ts
//
// Typed wrappers for /api/edit/*. The Rust CLI (via report-server) is the only
// write path; these never hand-edit the scenario tree.

import type { AriaNode, BufferState, EditKind, LiveInput, PickedElement } from '@/features/editor/types';

type Json = Record<string, unknown>;

async function getJson(url: string): Promise<{ ok: boolean; status: number; body: any }> {
  const r = await fetch(url, { headers: { accept: 'application/json' } });
  const body = await r.json().catch(() => ({}));
  return { ok: r.ok, status: r.status, body };
}

async function postJson(url: string, payload?: Json): Promise<{ ok: boolean; status: number; body: any }> {
  const r = await fetch(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload || {}),
  });
  const body = await r.json().catch(() => ({}));
  return { ok: r.ok, status: r.status, body };
}

export async function getBuffer(): Promise<BufferState> {
  const { body } = await getJson('/api/edit/buffer');
  return {
    sid: body.sid ?? null,
    intent: body.intent ?? null,
    session: body.session ?? null,
    baseline: body.baseline ?? null,
    rows: Array.isArray(body.rows) ? body.rows : [],
  };
}

export async function getSnapshot(interactive: boolean): Promise<{ ok: boolean; nodes: AriaNode[]; error?: string }> {
  const { ok, body } = await getJson(`/api/edit/snapshot?interactive=${interactive ? '1' : '0'}`);
  if (!ok) return { ok: false, nodes: [], error: body.error || 'snapshot failed (is a page open?)' };
  return { ok: true, nodes: (body.result && body.result.nodes) || [] };
}

export function startSession(intent: string, url: string) {
  return postJson('/api/edit/start', { intent, url });
}

export function recordStep(kind: EditKind, payload: Json) {
  return postJson('/api/edit/record', { kind, payload });
}

export function runStep(kind: EditKind, payload: Json) {
  return postJson('/api/edit/run-step', { kind, payload });
}

export function deleteRow(index: number) {
  return postJson('/api/edit/delete', { index });
}

export function moveRow(from: number, to: number) {
  return postJson('/api/edit/move', { from, to });
}

export function flush() {
  return postJson('/api/edit/flush', {});
}

export function cancel() {
  return postJson('/api/edit/cancel', {});
}

export function sendInput(evt: LiveInput): void {
  // Fire-and-forget; input forwarding must not block the pointer loop.
  fetch('/api/edit/input', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(evt),
  }).catch(() => {});
}

export async function pick(nx: number, ny: number): Promise<{ ok: boolean; element?: PickedElement; error?: string }> {
  const { ok, body } = await postJson('/api/edit/pick', { nx, ny });
  return { ok: ok && !!body.element, element: body.element, error: body.error };
}
