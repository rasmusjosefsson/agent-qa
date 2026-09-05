// web/src/features/editor/types.ts

export type EditKind = 'do' | 'check'

export interface DirectStep {
  id?: string
  intent?: string
  kind?: EditKind
  verb?: string
  on?: { role?: string; name?: string; raw?: { kind?: string; value?: string } }
  value?: { from?: string; literal?: unknown }
  claim?: Record<string, unknown>
  params?: Record<string, unknown>
}

export interface BufferRow {
  stepIndex: number
  stepId: string
  step: DirectStep
}

export interface BufferState {
  sid: string | null
  intent: string | null
  session?: string | null
  baseline?: string | null
  rows: BufferRow[]
}

export interface AriaNode { role?: string; name?: string; depth?: number; pickable?: boolean; ref?: string }
export interface PickedBox { nx: number; ny: number; nw: number; nh: number }
export interface PickedElement { role?: string; name?: string; ref?: string; box?: PickedBox; interactive?: boolean }
export type ComposeResult = { error: string } | { kind: EditKind; payload: Record<string, unknown> }
export interface RunResult { ok: boolean; error?: string | null; recorded?: boolean; report?: Record<string, unknown> }
export type ClickMode = 'interact' | 'record' | 'pick'
export type LiveTone = 'idle' | 'busy' | 'ok' | 'err'
export interface LiveStatus { text: string; tone: LiveTone }
export type LiveInput =
  | { type: 'click'; nx: number; ny: number; record?: boolean }
  | { type: 'scroll'; nx: number; ny: number; dx: number; dy: number }
  | { type: 'key'; text?: string; key?: string; record?: boolean }
  | { type: 'back' } | { type: 'forward' } | { type: 'reload' } | { type: 'navigate'; url: string; record?: boolean }
export interface ComposeForm { verb: string; role: string; name: string; value: string; intent: string }
export const EMPTY_FORM: ComposeForm = { verb: 'click', role: '', name: '', value: '', intent: '' }
