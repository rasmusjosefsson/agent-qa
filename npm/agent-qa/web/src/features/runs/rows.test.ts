// web/src/features/runs/rows.test.ts
import { describe, it, expect } from 'vitest'
import {
  cleanSummary,
  collapseEvents,
  fmtMs,
  icon,
  isRunLive,
  mergeRows,
  scenarioVerdict,
  stepText,
  verbBadge,
  verbCat,
  verdictTone,
} from './rows'

describe('fmtMs / icon', () => {
  it('formats ms and seconds', () => {
    expect(fmtMs(250)).toBe('250ms')
    expect(fmtMs(1500)).toBe('1.5s')
    expect(fmtMs(undefined)).toBe('')
  })
  it('maps status to icons', () => {
    expect(icon('pass')).toBe('✓')
    expect(icon('fail')).toBe('✗')
    expect(icon('running')).toBe('…')
    expect(icon('pending')).toBe('○')
    expect(icon('whatever')).toBe('·')
  })
})

describe('verbCat / verbBadge', () => {
  it('categorizes verbs', () => {
    expect(verbCat('goto')).toBe('nav')
    expect(verbCat('navigate')).toBe('nav')
    expect(verbCat('clickRole')).toBe('click')
    expect(verbCat('fillByLabel')).toBe('fill')
    expect(verbCat('type')).toBe('fill')
    expect(verbCat('pressKey')).toBe('press')
    expect(verbCat('assertPresent')).toBe('assert')
    expect(verbCat('wait')).toBe('wait')
    expect(verbCat('weird')).toBe('action')
  })
  it('labels badges', () => {
    expect(verbBadge('goto')).toBe('GO TO')
    expect(verbBadge('click')).toBe('CLICK')
    expect(verbBadge('type')).toBe('FILL')
    expect(verbBadge('custom')).toBe('CUSTOM')
  })
})

describe('stepText', () => {
  it('prefers role/name, then literal, then verb/id', () => {
    expect(stepText({ on: { role: 'button', name: 'OK' } } as never)).toBe('button “OK”')
    expect(stepText({ value: { literal: 'hello' } } as never)).toBe('hello')
    expect(stepText({ verb: 'wait' } as never)).toBe('wait')
  })
})

describe('collapseEvents', () => {
  it('merges by idx (terminal wins) and sorts', () => {
    const out = collapseEvents([
      { idx: 1, status: 'running' },
      { idx: 0, status: 'pass', id: 's0' },
      { idx: 1, status: 'pass', id: 's1', ms: 12 },
    ])
    expect(out.map((e) => e.idx)).toEqual([0, 1])
    expect(out[1]).toMatchObject({ status: 'pass', id: 's1', ms: 12 })
  })
})

describe('mergeRows', () => {
  it('returns events unchanged with no def steps', () => {
    const ev = [{ idx: 1, id: 'a', status: 'pass' }]
    expect(mergeRows(ev, null)).toBe(ev)
  })
  it('fills not-yet-run steps as pending while keeping real events', () => {
    const events = [{ idx: 1, id: 's0', status: 'pass', ms: 5 }]
    const def = [{ id: 's0', verb: 'goto', intent: 'open' }, { id: 's1', verb: 'click', intent: 'press' }]
    const rows = mergeRows(events, def)
    expect(rows).toHaveLength(2)
    expect(rows[0]).toMatchObject({ id: 's0', status: 'pass' })
    expect(rows[1]).toMatchObject({ id: 's1', status: 'pending', pending: true, idx: 2, kind: 'click' })
  })
})

describe('verdicts', () => {
  it('isRunLive', () => {
    expect(isRunLive({ status: { state: 'running' } })).toBe(true)
    expect(isRunLive({ status: { state: 'done' } })).toBe(false)
    expect(isRunLive(null)).toBe(false)
  })
  it('verdictTone from summary/state', () => {
    expect(verdictTone(null, 'running')).toBe('running')
    expect(verdictTone('SUMMARY: PASS 3/3')).toBe('pass')
    expect(verdictTone('FAIL 1/3')).toBe('fail')
    expect(verdictTone('weird')).toBe('running')
  })
  it('scenarioVerdict from exitCode/ok/state', () => {
    expect(scenarioVerdict({ state: 'running' })).toBe('running')
    expect(scenarioVerdict({ exitCode: 0 })).toBe('pass')
    expect(scenarioVerdict({ ok: true })).toBe('pass')
    expect(scenarioVerdict({ exitCode: 1 })).toBe('fail')
    expect(scenarioVerdict({ ok: false })).toBe('fail')
    expect(scenarioVerdict(null)).toBe(null)
    expect(scenarioVerdict({})).toBe(null)
  })
  it('cleanSummary strips the SUMMARY prefix', () => {
    expect(cleanSummary('SUMMARY: PASS 2/2')).toBe('PASS 2/2')
    expect(cleanSummary(null)).toBe('')
  })
})
