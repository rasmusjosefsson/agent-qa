// web/src/lib/resources.ts
//
// Suspense-readable boot resources for the Chat tab. We cache the *promise*
// per key (not the resolved value) so `use(resource)` returns the same promise
// on every render: it suspends once on first read, then resolves synchronously
// from React's promise cache. Combined with a `startTransition`-driven chat
// switch, this lets React keep the previous chat on screen until the next
// chat's state resolves — no remount "Connecting…" flash.

import { getRoot, getChatState } from './api';
import type { RootInfo, ChatState } from './types';

let rootPromise: Promise<RootInfo> | null = null;

export function rootResource(): Promise<RootInfo> {
  if (!rootPromise) rootPromise = getRoot();
  return rootPromise;
}

const chatStateCache = new Map<string, Promise<ChatState>>();

export function chatStateResource(cid: string): Promise<ChatState> {
  let p = chatStateCache.get(cid);
  if (!p) {
    p = getChatState(cid);
    chatStateCache.set(cid, p);
  }
  return p;
}

// Kick off the state fetch ahead of render (e.g. right after creating a chat)
// so the promise is already in flight by the time the conversation mounts.
export function prefetchChatState(cid: string): void {
  chatStateResource(cid);
}

// Drop a chat's cached state (on delete) so the map doesn't grow unbounded.
// Never call this for a currently-mounted chat: `use()` re-reads the resource
// each render, so replacing the promise would re-suspend the live view.
export function dropChatState(cid: string): void {
  chatStateCache.delete(cid);
}
