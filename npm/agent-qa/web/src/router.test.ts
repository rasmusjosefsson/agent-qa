import { describe, expect, it } from 'vitest'
import { withBase, withoutBase } from './router'

describe('app base paths', () => {
  it('leaves the local app at /', () => {
    expect(withBase('/chat', '/')).toBe('/chat')
    expect(withBase('/', '/')).toBe('/')
    expect(withoutBase('/chat', '/')).toBe('/chat')
    expect(withoutBase('/', '/')).toBe('/')
  })

  it('prefixes and strips the published demo base', () => {
    const base = '/agent-qa/play/'
    expect(withBase('/', base)).toBe('/agent-qa/play/')
    expect(withBase('/chat', base)).toBe('/agent-qa/play/chat')
    expect(withoutBase('/agent-qa/play/', base)).toBe('/')
    expect(withoutBase('/agent-qa/play/chat', base)).toBe('/chat')
    expect(withoutBase('/docs', base)).toBe('/docs')
  })
})
