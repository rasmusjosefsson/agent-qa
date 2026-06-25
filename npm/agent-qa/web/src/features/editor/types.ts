// web/src/features/editor/types.ts

export type EditKind = 'navigation' | 'action' | 'wait' | 'assert';

export interface BufferRow {
  kind: string;
  payload?: {
    route?: string;
    method?: string;
    args?: unknown[];
    condition?: { kind?: string; ms?: number };
    kind?: string;
    intent?: string;
    [k: string]: unknown;
  };
}

export interface BufferState {
  sid: string | null;
  intent: string | null;
  session?: string | null;
  baseline?: string | null;
  rows: BufferRow[];
}

export interface AriaNode {
  role?: string;
  name?: string;
  depth?: number;
  pickable?: boolean;
  ref?: string;
}

export interface PickedBox {
  nx: number;
  ny: number;
  nw: number;
  nh: number;
}

export interface PickedElement {
  role?: string;
  name?: string;
  ref?: string;
  box?: PickedBox;
  interactive?: boolean;
}

export type ComposeResult =
  | { error: string }
  | { kind: EditKind; payload: Record<string, unknown> };

export interface RunResult {
  ok: boolean;
  error?: string | null;
  recorded?: boolean;
  report?: Record<string, unknown>;
}

export type ClickMode = 'interact' | 'record' | 'pick';

export type LiveTone = 'idle' | 'busy' | 'ok' | 'err';
export interface LiveStatus {
  text: string;
  tone: LiveTone;
}

// Normalized-coordinate input events forwarded to /api/edit/input.
export type LiveInput =
  | { type: 'click'; nx: number; ny: number; record?: boolean }
  | { type: 'scroll'; nx: number; ny: number; dx: number; dy: number }
  | { type: 'key'; text?: string; key?: string; record?: boolean }
  | { type: 'back' }
  | { type: 'forward' }
  | { type: 'reload' }
  | { type: 'navigate'; url: string; record?: boolean };

// Composer form state (lifted so pick actions can populate it).
export interface ComposeForm {
  verb: string;
  role: string;
  name: string;
  value: string;
  intent: string;
}

export const EMPTY_FORM: ComposeForm = {
  verb: 'click',
  role: '',
  name: '',
  value: '',
  intent: '',
};
