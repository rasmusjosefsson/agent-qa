// web/src/features/editor/compose.ts

import type { BufferRow, ComposeForm, ComposeResult, DirectStep } from './types'

export const VERB_FIELDS: Record<string, { value?: string; name?: string; fields: string[]; intentRequired?: boolean }> = {
  navigation: { value: 'URL', fields: ['value'] },
  click: { fields: ['role', 'name'] },
  type: { value: 'Value', name: 'Label', fields: ['name', 'value'] },
  press: { value: 'Key', fields: ['value'] },
  wait: { value: 'Milliseconds', fields: ['value'] },
  assertPresent: { fields: ['role', 'name', 'intent'], intentRequired: true },
  assertAbsent: { fields: ['role', 'name', 'intent'], intentRequired: true },
  assertUrl: { value: 'URL pattern', fields: ['value', 'intent'], intentRequired: true },
}

export const VERB_OPTIONS = [
  { value: 'navigation', label: 'navigate (goto URL)' }, { value: 'click', label: 'click (role + name)' },
  { value: 'type', label: 'type (label + value)' }, { value: 'press', label: 'press key' },
  { value: 'wait', label: 'wait (ms)' }, { value: 'assertPresent', label: 'assert present (role + name)' },
  { value: 'assertAbsent', label: 'assert absent (role + name)' }, { value: 'assertUrl', label: 'assert URL matches' },
]

export function composePayload(form: ComposeForm): ComposeResult {
  const role = form.role.trim(); const name = form.name.trim(); const value = form.value.trim(); const intent = form.intent.trim();
  const doStep = (verb: string, extra: Record<string, unknown>) => ({ kind: 'do' as const, payload: { intent: intent || verb, verb, ...extra } })
  switch (form.verb) {
    case 'navigation': return value ? doStep('goto', { value: { from: 'literal', literal: value } }) : { error: 'URL is required.' }
    case 'click': return role && name ? doStep('click', { on: { role, name } }) : { error: 'Role and name are required.' }
    case 'type': return name ? doStep('type', { on: { role: 'textbox', name }, value: { from: 'literal', literal: value } }) : { error: 'Label is required.' }
    case 'press': return value ? doStep('press', { value: { from: 'literal', literal: value } }) : { error: 'Key is required.' }
    case 'wait': { const ms = Number(value); return Number.isInteger(ms) && ms >= 0 ? doStep('wait', { params: { ms } }) : { error: 'Milliseconds must be a non-negative integer.' } }
    case 'assertPresent': return role && name && intent ? { kind: 'check', payload: { intent, claim: { subject: { element: { role, name } }, predicate: 'isVisible' } } } : { error: 'Role, name, and intent are required.' }
    case 'assertAbsent': return role && name && intent ? { kind: 'check', payload: { intent, claim: { subject: { element: { role, name } }, predicate: 'isHidden' } } } : { error: 'Role, name, and intent are required.' }
    case 'assertUrl': return value && intent ? { kind: 'check', payload: { intent, claim: { subject: { url: true }, predicate: 'contains', value } } } : { error: 'URL pattern and intent are required.' }
    default: return { error: `unknown verb ${form.verb}` }
  }
}

export function maskValue(label: unknown, value: unknown): string {
  const text = String(value == null ? '' : value)
  return /pass|secret|cvv|card|token|otp|\bpin\b/i.test(String(label || '')) ? '•'.repeat(Math.min(10, Math.max(4, text.length))) : text
}

export interface RowLabel { cls: string; title: string; detail: string }
export function rowLabel(row: BufferRow): RowLabel {
  const step: DirectStep = row.step || {}
  if (step.kind === 'check') return { cls: 'assert', title: 'Check', detail: step.intent || '' }
  const target = step.on?.name || step.on?.raw?.value || ''
  const literal = step.value?.literal
  if (step.verb === 'goto') return { cls: 'nav', title: 'Go to', detail: String(literal || '') }
  if (step.verb === 'click') return { cls: 'click', title: 'Click', detail: target }
  if (step.verb === 'type') return { cls: 'fill', title: 'Fill', detail: `${target} → ${maskValue(target, literal)}` }
  if (step.verb === 'press') return { cls: 'press', title: 'Press', detail: String(literal || '') }
  if (step.verb === 'wait') return { cls: 'wait', title: 'Wait', detail: String(step.params?.ms || '') }
  return { cls: 'action', title: step.verb || 'step', detail: step.intent || '' }
}

export function recordLabel(payload: Record<string, unknown>): string { return String(payload.intent || payload.verb || 'step') }
