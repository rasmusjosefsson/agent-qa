// web/src/features/knowledge/exportPrompt.test.ts
import { describe, it, expect } from 'vitest'
import { buildXrayExportPrompt, type XrayExportItem } from './exportPrompt'

const items: XrayExportItem[] = [
  { xrayKey: 'PROJ-1', title: 'Log in', result: 'pass' },
  { xrayKey: 'PROJ-2', title: 'Checkout', result: 'fail', runUrl: 'http://x/api/scenarios/s/runs/r' },
  { xrayKey: 'PROJ-3', title: 'Profile', result: 'todo' },
]

describe('buildXrayExportPrompt', () => {
  const p = buildXrayExportPrompt('Release', items, 'http://x')

  it('frames a Test Execution named after the plan', () => {
    expect(p).toContain('plan "Release"')
    expect(p).toContain('Test Execution')
  })

  it('lists each test with a mapped result', () => {
    expect(p).toContain('PROJ-1: PASSED')
    expect(p).toContain('PROJ-2: FAILED')
    expect(p).toContain('PROJ-3: TODO (not run)')
  })

  it('attaches an evidence link only for failures that have a run', () => {
    expect(p).toContain('evidence: http://x/api/scenarios/s/runs/r')
    // the passing/todo rows carry no evidence link
    expect(p.match(/evidence:/g)?.length).toBe(1)
  })
})
