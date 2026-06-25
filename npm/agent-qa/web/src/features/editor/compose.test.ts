// web/src/features/editor/compose.test.ts
import { describe, it, expect } from 'vitest'
import { composePayload, rowLabel, maskValue, recordLabel } from './compose'
import { EMPTY_FORM, type ComposeForm } from './types'

const form = (over: Partial<ComposeForm>): ComposeForm => ({ ...EMPTY_FORM, ...over })

describe('composePayload', () => {
  it('navigation requires a URL, attaches intent when present', () => {
    expect(composePayload(form({ verb: 'navigation', value: '' }))).toEqual({ error: 'URL is required.' })
    expect(composePayload(form({ verb: 'navigation', value: 'https://example.com' }))).toEqual({
      kind: 'navigation',
      payload: { route: 'https://example.com' },
    })
    expect(composePayload(form({ verb: 'navigation', value: 'https://example.com', intent: 'land' }))).toEqual({
      kind: 'navigation',
      payload: { route: 'https://example.com', intent: 'land' },
    })
  })

  it('click → clickRole action with role+name args', () => {
    expect(composePayload(form({ verb: 'click', role: 'button', name: 'Login' }))).toEqual({
      kind: 'action',
      payload: { method: 'clickRole', args: ['button', 'Login'] },
    })
    expect(composePayload(form({ verb: 'click', role: '', name: 'Login' }))).toEqual({
      error: 'Role and name are required.',
    })
  })

  it('type → fillByLabel with label+value', () => {
    expect(composePayload(form({ verb: 'type', name: 'Email', value: 'a@b.c' }))).toEqual({
      kind: 'action',
      payload: { method: 'fillByLabel', args: ['Email', 'a@b.c'] },
    })
  })

  it('press → pressKey', () => {
    expect(composePayload(form({ verb: 'press', value: 'Enter' }))).toEqual({
      kind: 'action',
      payload: { method: 'pressKey', args: ['Enter'] },
    })
  })

  it('wait validates a non-negative integer', () => {
    expect(composePayload(form({ verb: 'wait', value: '500' }))).toEqual({
      kind: 'wait',
      payload: { condition: { kind: 'duration', ms: 500 } },
    })
    expect(composePayload(form({ verb: 'wait', value: '-1' }))).toEqual({
      error: 'Milliseconds must be a non-negative integer.',
    })
  })

  it('asserts require intent', () => {
    expect(composePayload(form({ verb: 'assertPresent', role: 'button', name: 'Go', intent: '' }))).toEqual({
      error: 'Intent is required for asserts.',
    })
    expect(composePayload(form({ verb: 'assertPresent', role: 'button', name: 'Go', intent: 'visible' }))).toEqual({
      kind: 'assert',
      payload: { kind: 'present', args: ['button', 'Go'], intent: 'visible' },
    })
    expect(composePayload(form({ verb: 'assertUrl', value: '/home', intent: 'routed' }))).toEqual({
      kind: 'assert',
      payload: { kind: 'url', args: ['/home'], intent: 'routed' },
    })
  })
})

describe('rowLabel', () => {
  it('labels navigation / click / fill / wait / assert rows', () => {
    expect(rowLabel({ kind: 'navigation', payload: { route: '/x' } })).toMatchObject({ cls: 'nav', title: 'Go to', detail: '/x' })
    expect(rowLabel({ kind: 'action', payload: { method: 'clickRole', args: ['button', 'OK'] } })).toMatchObject({
      cls: 'click',
      title: 'Click',
      detail: 'button “OK”',
    })
    expect(rowLabel({ kind: 'action', payload: { method: 'pressKey', args: ['Enter'] } })).toMatchObject({ cls: 'press', detail: 'Enter' })
    expect(rowLabel({ kind: 'wait', payload: { condition: { ms: 300 } } })).toMatchObject({ cls: 'wait', detail: '300 ms' })
    expect(rowLabel({ kind: 'assert', payload: { kind: 'present', args: ['button', 'Go'] } })).toMatchObject({
      cls: 'assert',
      title: 'Assert present',
    })
  })

  it('masks secret-looking fill values', () => {
    const label = rowLabel({ kind: 'action', payload: { method: 'fillByLabel', args: ['Password', 'hunter2'] } })
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
    expect(recordLabel({ method: 'clickRole', args: ['button', 'Login'] })).toBe('click button “Login”')
    expect(recordLabel({ method: 'pressKey', args: ['Enter'] })).toBe('press Enter')
    expect(recordLabel({ route: '/home' })).toBe('goto /home')
  })
})
