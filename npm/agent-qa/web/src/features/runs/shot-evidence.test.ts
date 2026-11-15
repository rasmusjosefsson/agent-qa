import { describe, expect, it } from 'vitest'
import { artifactUrl, baselineShotUrl } from '@/lib/runs-api'
import { evidenceStepId, focusStep, hasPageFrame, isShotMiss } from './shot-evidence'

const miss = {
  idx: 6,
  id: 's5',
  status: 'fail' as const,
  error:
    "shot 's1' differs from baseline: 4.28% pixels changed (tolerance 2.00%) — diff at fixtures/fixture/replays/r/shots-diff/s1.diff.png",
  screenshot: 'screenshots/s5.png',
}

describe('shot evidence', () => {
  it('points a shot-miss error, its screenshot, and its diff at the same step', () => {
    const id = evidenceStepId(miss)
    expect(id).toBe('s1')
    expect(isShotMiss(miss)).toBe(true)
    expect(miss.error).toContain(`shot '${id}'`)
    const actual = artifactUrl('fixture', 'r', 'screenshots', id!)
    const diff = artifactUrl('fixture', 'r', 'shots-diff', id!)
    const baseline = baselineShotUrl('fixture', `${id}.png`)
    expect(actual).toContain('/screenshots/s1')
    expect(diff).toContain('/shots-diff/s1')
    expect(baseline).toContain('/baselines/s1.png')
    expect(actual).not.toContain('s5')
    expect(diff).not.toContain('s5')
    expect(focusStep([
      { id: 's0', status: 'pass' },
      { id: 's1', status: 'pass' },
      miss,
    ])?.id).toBe('s5')
  })

  it('opens a passing run on the first real page, not a blank viewport', () => {
    const viewport = { id: 's0', status: 'pass', kind: 'do:viewport', screenshot: 'screenshots/s0.png' }
    const open = { id: 's1', status: 'pass', kind: 'do:goto', screenshot: 'screenshots/s1.png' }
    const typed = { id: 's2', status: 'pass', kind: 'do:type', screenshot: 'screenshots/s2.png' }
    expect(hasPageFrame(viewport)).toBe(false)
    expect(hasPageFrame(open)).toBe(true)
    expect(isShotMiss(typed)).toBe(false)
    expect(focusStep([viewport, open, typed])?.id).toBe('s1')
    expect(evidenceStepId(typed)).toBe('s2')
  })

  it('keeps an ordinary step on its own screenshot', () => {
    const step = { id: 's0', status: 'pass', screenshot: 'screenshots/s0.png' }
    expect(evidenceStepId(step)).toBe('s0')
    expect(isShotMiss(step)).toBe(false)
  })

  it('prefers the claim subject over the check step id', () => {
    expect(evidenceStepId({ id: 's5', status: 'fail', error: "shot 's1' differs" }, 's1')).toBe('s1')
  })
})
