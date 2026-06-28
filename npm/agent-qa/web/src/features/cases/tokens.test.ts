// web/src/features/cases/tokens.test.ts
import { describe, it, expect } from 'vitest'
import { extractTokens, isSensitiveName, reconcileInputs, tokenSegments } from './tokens'

describe('extractTokens', () => {
  it('finds UPPER_SNAKE bracket tokens, de-duped across inputs', () => {
    expect(
      extractTokens(['Enter [EMAIL] then [PASSWORD]', 'Re-enter [EMAIL]'], 'Expect [BANNER]')
    ).toEqual(['EMAIL', 'PASSWORD', 'BANNER'])
  })
  it('ignores lowercase / malformed brackets', () => {
    expect(extractTokens('[email] [Foo] [123] [A]')).toEqual(['A'])
  })
})

describe('isSensitiveName', () => {
  it('flags secret-ish names', () => {
    expect(isSensitiveName('PASSWORD')).toBe(true)
    expect(isSensitiveName('API_KEY')).toBe(true)
    expect(isSensitiveName('OTP')).toBe(true)
    expect(isSensitiveName('EMAIL')).toBe(false)
  })
})

describe('reconcileInputs', () => {
  it('adds new tokens (guessing sensitivity), preserves edits, drops removed', () => {
    const prev = { EMAIL: { type: 'string', default: 'a@b.c', sensitive: false } }
    const next = reconcileInputs(['EMAIL', 'PASSWORD'], prev)
    expect(next.EMAIL).toBe(prev.EMAIL) // preserved by reference
    expect(next.PASSWORD).toEqual({ type: 'string', default: '', sensitive: true })
    expect(reconcileInputs(['PASSWORD'], next)).not.toHaveProperty('EMAIL')
  })
})

describe('tokenSegments', () => {
  it('splits into plain/token runs', () => {
    expect(tokenSegments('hi [X] yo')).toEqual([
      { text: 'hi ', token: false },
      { text: '[X]', token: true },
      { text: ' yo', token: false },
    ])
  })
})
