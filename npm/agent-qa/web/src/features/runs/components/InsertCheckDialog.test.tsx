// web/src/features/runs/components/InsertCheckDialog.test.tsx
//
// The claim JSON the dialog emits — the dialog itself portals under Radix so
// it can't be rendered statically; the builder is what must not drift from
// the scenario schema.

import { describe, expect, it } from 'vitest'
import { buildCheckClaim, checkDraftValid, type CheckDraft } from './InsertCheckDialog'

const base: CheckDraft = {
  subjectKind: 'element',
  role: '',
  name: '',
  css: '',
  useCss: false,
  shotStep: '',
  shotTolerance: '',
  consoleType: 'any',
  consoleText: '',
  netUrl: '',
  netMethod: 'any',
  netKind: 'fired',
  netPath: '',
  predicate: 'exists',
  value: '',
}

describe('buildCheckClaim', () => {
  it('elementCount emits an ofKind=count subject over a raw css locator', () => {
    const claim = buildCheckClaim({
      ...base,
      subjectKind: 'elementCount',
      css: ' .athing.submission ',
      predicate: 'countEquals',
      value: '30',
    })
    expect(claim).toEqual({
      subject: {
        element: {
          raw: { kind: 'css', value: '.athing.submission' },
          reason: 'assertion added from the Runs pane',
        },
        ofKind: 'count',
      },
      predicate: 'countEquals',
      value: 30, // JSON number — the claim compares numerically
    })
  })

  it('element claims keep the role locator and plain predicate surface', () => {
    const claim = buildCheckClaim({ ...base, role: 'button', name: 'Save', predicate: 'isVisible' })
    expect(claim).toEqual({
      subject: { element: { role: 'button', name: 'Save' } },
      predicate: 'isVisible',
    })
  })

  it('network claims emit ofKind + matcher fields', () => {
    const claim = buildCheckClaim({
      ...base,
      subjectKind: 'network',
      netUrl: '/api/',
      netKind: 'status',
      predicate: 'equals',
      value: '200',
    })
    expect(claim.subject).toEqual({ network: { urlMatches: '/api/' }, ofKind: 'status' })
    expect(claim.value).toBe('200')
  })
})

describe('checkDraftValid', () => {
  it('elementCount needs a css selector and a numeric value', () => {
    const d = { ...base, subjectKind: 'elementCount' as const, predicate: 'countEquals', value: '30' }
    expect(checkDraftValid({ ...d, css: '' })).toBe(false)
    expect(checkDraftValid({ ...d, css: '.row', value: 'abc' })).toBe(false)
    expect(checkDraftValid({ ...d, css: '.row', value: '  ' })).toBe(false)
    expect(checkDraftValid({ ...d, css: '.row' })).toBe(true)
  })
})
