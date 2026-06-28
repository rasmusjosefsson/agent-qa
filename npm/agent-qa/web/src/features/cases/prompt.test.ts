// web/src/features/cases/prompt.test.ts
import { describe, it, expect } from 'vitest'
import { buildRunPrompt, runIntent } from './prompt'
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

  it('numbers the steps and includes the start url + expected result', () => {
    expect(p).toContain('1. Enter [EMAIL]')
    expect(p).toContain('3. Click Sign in')
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
