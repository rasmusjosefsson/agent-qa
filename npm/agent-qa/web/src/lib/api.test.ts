import { afterEach, describe, expect, it, vi } from 'vitest'
import { browserNavigate, postAbort, postModel, postNew, postPrompt, postThinking } from './api'

describe('chat api offline handling', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('postPrompt returns a failed result instead of throwing when fetch rejects', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('ECONNREFUSED')))
    const res = await postPrompt('c1', 'hello')
    expect(res.ok).toBe(false)
    expect(res.status).toBe(503)
    expect(res.body.error).toContain('unreachable')
  })

  it('postAbort/postNew resolve to a 503 response when fetch rejects', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('down')))
    for (const call of [() => postAbort('c1'), () => postNew('c1')]) {
      const res = await call()
      expect(res.ok).toBe(false)
      expect(res.status).toBe(503)
    }
  })

  it('postModel/postThinking surface the failure as ok:false', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('down')))
    expect((await postModel('c1', 'openai', 'gpt-5')).ok).toBe(false)
    expect((await postThinking('c1', 'high')).ok).toBe(false)
  })

  it('browserNavigate resolves to a 503 response when fetch rejects', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('down')))
    const res = await browserNavigate({ url: 'https://x' })
    expect(res.ok).toBe(false)
    expect(res.status).toBe(503)
  })

  it('live fetches still resolve normally', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response('{"echo":1}', { status: 200 })),
    )
    const res = await postPrompt('c1', 'hello')
    expect(res.ok).toBe(true)
  })
})
