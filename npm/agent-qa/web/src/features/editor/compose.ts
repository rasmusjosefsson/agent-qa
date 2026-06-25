// web/src/features/editor/compose.ts
//
// Pure helpers ported 1:1 from lib/public/editor.js: form → trigger payload
// (composePayload), buffer row → human label (rowLabel), value masking, and the
// auto-record label. Kept React-free so they're unit-testable.

import type { BufferRow, ComposeForm, ComposeResult } from './types';

// Which fields each composer verb shows + their relabeled captions.
export const VERB_FIELDS: Record<
  string,
  { value?: string; name?: string; fields: string[]; intentRequired?: boolean }
> = {
  navigation: { value: 'URL', fields: ['value'] },
  click: { fields: ['role', 'name'] },
  type: { value: 'Value', name: 'Label', fields: ['name', 'value'] },
  press: { value: 'Key', fields: ['value'] },
  wait: { value: 'Milliseconds', fields: ['value'] },
  assertPresent: { fields: ['role', 'name', 'intent'], intentRequired: true },
  assertAbsent: { fields: ['role', 'name', 'intent'], intentRequired: true },
  assertUrl: { value: 'URL pattern', fields: ['value', 'intent'], intentRequired: true },
};

export const VERB_OPTIONS: { value: string; label: string }[] = [
  { value: 'navigation', label: 'navigate (goto URL)' },
  { value: 'click', label: 'click (role + name)' },
  { value: 'type', label: 'type (label + value)' },
  { value: 'press', label: 'press key' },
  { value: 'wait', label: 'wait (ms)' },
  { value: 'assertPresent', label: 'assert present (role + name)' },
  { value: 'assertAbsent', label: 'assert absent (role + name)' },
  { value: 'assertUrl', label: 'assert URL matches' },
];

// Build { kind, payload } for record-step / run-step, or { error }.
export function composePayload(form: ComposeForm): ComposeResult {
  const verb = form.verb;
  const role = form.role.trim();
  const name = form.name.trim();
  const value = form.value.trim();
  const intent = form.intent.trim();
  const spec = VERB_FIELDS[verb] || { fields: [] };
  if (spec.intentRequired && !intent) return { error: 'Intent is required for asserts.' };

  switch (verb) {
    case 'navigation':
      if (!value) return { error: 'URL is required.' };
      return { kind: 'navigation', payload: intent ? { route: value, intent } : { route: value } };
    case 'click':
      if (!role || !name) return { error: 'Role and name are required.' };
      return {
        kind: 'action',
        payload: { method: 'clickRole', args: [role, name], ...(intent ? { intent } : {}) },
      };
    case 'type':
      if (!name) return { error: 'Label is required.' };
      return {
        kind: 'action',
        payload: { method: 'fillByLabel', args: [name, value], ...(intent ? { intent } : {}) },
      };
    case 'press':
      if (!value) return { error: 'Key is required.' };
      return { kind: 'action', payload: { method: 'pressKey', args: [value], ...(intent ? { intent } : {}) } };
    case 'wait': {
      const ms = parseInt(value, 10);
      if (!Number.isInteger(ms) || ms < 0) return { error: 'Milliseconds must be a non-negative integer.' };
      return { kind: 'wait', payload: { condition: { kind: 'duration', ms }, ...(intent ? { intent } : {}) } };
    }
    case 'assertPresent':
      if (!role || !name) return { error: 'Role and name are required.' };
      return { kind: 'assert', payload: { kind: 'present', args: [role, name], intent } };
    case 'assertAbsent':
      if (!role || !name) return { error: 'Role and name are required.' };
      return { kind: 'assert', payload: { kind: 'absent', args: [role, name], intent } };
    case 'assertUrl':
      if (!value) return { error: 'URL pattern is required.' };
      return { kind: 'assert', payload: { kind: 'url', args: [value], intent } };
    default:
      return { error: `unknown verb ${verb}` };
  }
}

// Hide secret-looking field values in the steps list (display only).
export function maskValue(label: unknown, value: unknown): string {
  const v = String(value == null ? '' : value);
  if (/pass|secret|cvv|card|token|otp|\bpin\b/i.test(String(label || ''))) {
    return '•'.repeat(Math.min(10, Math.max(4, v.length)));
  }
  return v;
}

export interface RowLabel {
  cls: string;
  title: string;
  detail: string;
}

// Map a buffer row to a category badge + action title + detail line.
export function rowLabel(row: BufferRow): RowLabel {
  const p = row.payload || {};
  switch (row.kind) {
    case 'navigation':
      return { cls: 'nav', title: 'Go to', detail: p.route || '' };
    case 'action': {
      const m = p.method || 'action';
      const a = (p.args || []) as unknown[];
      switch (m) {
        case 'clickRole':
          return { cls: 'click', title: 'Click', detail: `${a[0]} “${a[1]}”` };
        case 'clickByText':
        case 'clickByLabel':
          return { cls: 'click', title: 'Click', detail: `“${a[0]}”` };
        case 'clickSelector':
          return { cls: 'click', title: 'Click', detail: String(a[0] || '') };
        case 'fillByLabel':
        case 'fillBySelector':
          return { cls: 'fill', title: 'Fill', detail: `${a[0]} → ${maskValue(a[0], a[1])}` };
        case 'pressKey':
          return { cls: 'press', title: 'Press', detail: String(a[0] || '') };
        default:
          return { cls: 'action', title: String(m), detail: a.join('  ·  ') };
      }
    }
    case 'wait': {
      const c = p.condition || {};
      return { cls: 'wait', title: 'Wait', detail: c.ms != null ? `${c.ms} ms` : c.kind || '' };
    }
    case 'assert':
      return { cls: 'assert', title: `Assert ${p.kind || ''}`.trim(), detail: ((p.args || []) as unknown[]).join('  ·  ') };
    default:
      return { cls: 'action', title: row.kind || '?', detail: '' };
  }
}

// Human label for an auto-recorded step payload.
export function recordLabel(payload: BufferRow['payload']): string {
  if (!payload) return 'step';
  const m = payload.method;
  const a = (payload.args || []) as unknown[];
  if (m === 'clickRole') return `click ${a[0]} “${a[1]}”`;
  if (m === 'fillByLabel') return `fill “${a[0]}”`;
  if (m === 'pressKey') return `press ${a[0]}`;
  if (payload.route) return `goto ${payload.route}`;
  return 'step';
}
