// web/src/features/editor/compose.test.ts
import { describe, it, expect } from 'vitest'
import { composePayload, rowLabel, maskValue, recordLabel } from './compose'
import { EMPTY_FORM, type BufferRow, type ComposeForm } from './types'

const form = (over: Partial<ComposeForm>): ComposeForm => ({ ...EMPTY_FORM, ...over })
const row = (step: BufferRow['step']): BufferRow => ({ stepIndex: 0, stepId: 's0', step })

describe('composePayload', () => {
  it('navigation requires a URL, attaches intent when present', () => {
    expect(composePayload(form({ verb: 'navigation', value: '' }))).toEqual({ error: 'URL is required.' })
    expect(composePayload(form({ verb: 'navigation', value: 'https://example.com' }))).toEqual({
      kind: 'do',
      payload: { intent: 'goto', verb: 'goto', value: { from: 'literal', literal: 'https://example.com' } },
    })
    expect(composePayload(form({ verb: 'navigation', value: 'https://example.com', intent: 'land' }))).toEqual({
      kind: 'do',
      payload: { intent: 'land', verb: 'goto', value: { from: 'literal', literal: 'https://example.com' } },
    })
  })

  it('click → do step with on {role, name}', () => {
    expect(composePayload(form({ verb: 'click', role: 'button', name: 'Login' }))).toEqual({
      kind: 'do',
      payload: { intent: 'click', verb: 'click', on: { role: 'button', name: 'Login' } },
    })
    expect(composePayload(form({ verb: 'click', role: '', name: 'Login' }))).toEqual({
      error: 'Role and name are required.',
    })
  })

  it('type → do step with textbox label + literal value', () => {
    expect(composePayload(form({ verb: 'type', name: 'Email', value: 'a@b.c' }))).toEqual({
      kind: 'do',
      payload: {
        intent: 'type',
        verb: 'type',
        on: { role: 'textbox', name: 'Email' },
        value: { from: 'literal', literal: 'a@b.c' },
      },
    })
  })

  it('press → do step with literal key', () => {
    expect(composePayload(form({ verb: 'press', value: 'Enter' }))).toEqual({
      kind: 'do',
      payload: { intent: 'press', verb: 'press', value: { from: 'literal', literal: 'Enter' } },
    })
  })

  it('wait validates a non-negative integer', () => {
    expect(composePayload(form({ verb: 'wait', value: '500' }))).toEqual({
      kind: 'do',
      payload: { intent: 'wait', verb: 'wait', params: { ms: 500 } },
    })
    expect(composePayload(form({ verb: 'wait', value: '-1' }))).toEqual({
      error: 'Milliseconds must be a non-negative integer.',
    })
  })

  it('asserts require intent', () => {
    expect(composePayload(form({ verb: 'assertPresent', role: 'button', name: 'Go', intent: '' }))).toEqual({
      error: 'Role, name, and intent are required.',
    })
    expect(composePayload(form({ verb: 'assertPresent', role: 'button', name: 'Go', intent: 'visible' }))).toEqual({
      kind: 'check',
      payload: { intent: 'visible', claim: { subject: { element: { role: 'button', name: 'Go' } }, predicate: 'isVisible' } },
    })
    expect(composePayload(form({ verb: 'assertUrl', value: '/home', intent: 'routed' }))).toEqual({
      kind: 'check',
      payload: { intent: 'routed', claim: { subject: { url: true }, predicate: 'contains', value: '/home' } },
    })
  })
})

describe('rowLabel', () => {
  it('labels navigation / click / fill / wait / check rows', () => {
    expect(rowLabel(row({ verb: 'goto', value: { literal: '/x' } }))).toMatchObject({
      cls: 'nav',
      title: 'Go to',
      detail: '/x',
    })
    expect(rowLabel(row({ verb: 'click', on: { role: 'button', name: 'OK' } }))).toMatchObject({
      cls: 'click',
      title: 'Click',
      detail: 'OK',
    })
    expect(rowLabel(row({ verb: 'press', value: { literal: 'Enter' } }))).toMatchObject({ cls: 'press', detail: 'Enter' })
    expect(rowLabel(row({ verb: 'wait', params: { ms: 300 } }))).toMatchObject({ cls: 'wait', detail: '300' })
    expect(rowLabel(row({ kind: 'check', intent: 'Go is visible' }))).toMatchObject({
      cls: 'assert',
      title: 'Check',
      detail: 'Go is visible',
    })
  })

  it('masks secret-looking fill values', () => {
    const label = rowLabel(row({ verb: 'type', on: { name: 'Password' }, value: { literal: 'hunter2' } }))
    expect(label.detail).toContain('•')
    expect(label.detail).not.toContain('hunter2')
  })
})

describe('maskValue / recordLabel', () => {
  it('maskValue masks secret labels only', () => {
    expect(maskValue('Email', 'a@b.c')).toBe('a@b.c')
    expect(maskValue('CVV', '123')).toBe('••••')
  })
  it('recordLabel summarizes auto-recorded payloads', () => {
    expect(recordLabel({ intent: 'open the login page', verb: 'goto' })).toBe('open the login page')
    expect(recordLabel({ verb: 'click' })).toBe('click')
    expect(recordLabel({})).toBe('step')
  })
})
