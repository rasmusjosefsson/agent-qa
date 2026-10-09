// web/src/features/cases/prompt.test.ts
import { describe, it, expect } from 'vitest'
import { buildRepairPrompt, buildRunPrompt, runIntent } from './prompt'
import type { CaseRecord } from './types'

const base: CaseRecord = {
  schema: 'case/1',
  id: 'login',
  title: 'User can log in',
  startUrl: 'https://app.example.com/login',
  preconditions: '',
  steps: ['Enter [EMAIL]', 'Enter [PASSWORD]', 'Click Sign in'],
  expected: 'Dashboard loads',
  inputs: {
    EMAIL: { type: 'string', default: 'qa@example.com', sensitive: false },
    PASSWORD: { type: 'string', default: 'hunter2', sensitive: true },
  },
  tags: [],
  scenarioSid: null,
  source: 'manual',
  sourceRef: null,
  createdAt: 0,
  updatedAt: 0,
}

describe('runIntent', () => {
  it('carries the case id so the recorded scenario can be matched back', () => {
    expect(runIntent(base)).toBe('User can log in [login]')
  })
})

describe('buildRunPrompt', () => {
  const p = buildRunPrompt(base, 'http://127.0.0.1:7878')

  it('keeps the steps verbatim and includes the start url + expected result', () => {
    expect(p).toContain('Enter [EMAIL]')
    expect(p).toContain('Click Sign in')
    expect(p).toContain('--open "https://app.example.com/login"')
    expect(p).toContain('Expected: Dashboard loads')
  })

  it('records under the id-tagged intent', () => {
    expect(p).toContain('agent-qa start "User can log in [login]"')
  })

  it('lists non-sensitive defaults but redacts sensitive ones', () => {
    expect(p).toContain('[EMAIL] = "qa@example.com"')
    expect(p).not.toContain('hunter2')
    expect(p).toMatch(/\[PASSWORD\] = <use a real test value/)
  })

  it('tells the agent to link the result back to the case', () => {
    expect(p).toContain('SCENARIO_SID=<sid>')
    expect(p).toContain('http://127.0.0.1:7878/api/cases/login/link')
  })
})

describe('buildRepairPrompt', () => {
  const scenario = {
    sid: 's-xyz',
    latestRun: { runId: 'r-1', summary: 'SUMMARY: 4/5 (FAIL)', exitCode: 1 },
  } as unknown as Parameters<typeof buildRepairPrompt>[1]
  const p = buildRepairPrompt(base, scenario, 'http://127.0.0.1:7878')

  it('names the failing scenario, run, and verdict line', () => {
    expect(p).toContain('audit explain s-xyz')
    expect(p).toContain('Latest run: r-1 — SUMMARY: 4/5 (FAIL)')
    expect(p).toContain('SCENARIO_VERDICT=<pass|blocked|regression>')
  })

  it('covers every repair classification', () => {
    expect(p).toContain('heal-respond')
    expect(p).toContain('buffer load s-xyz')
    expect(p).toContain('onFailure: "ignore"')
    expect(p).toContain('product regression')
  })
})
