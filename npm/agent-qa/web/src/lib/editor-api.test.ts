import { afterEach, describe, expect, it, vi } from 'vitest'
import { deleteRow, flush, getBuffer, getSnapshot, startSession } from './editor-api'

describe('editor api offline handling', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('writes surface as ok:false when fetch rejects instead of throwing', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('ECONNREFUSED')))
    for (const call of [
      () => startSession('i', ''),
      () => deleteRow(0),
      () => flush(),
    ]) {
      const res = await call()
      expect(res.ok).toBe(false)
      expect(res.status).toBe(503)
      expect(res.body.error).toContain('unreachable')
    }
  })

  it('getBuffer/getSnapshot degrade to their empty shapes offline', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('down')))
    const buf = await getBuffer()
    expect(buf.rows).toEqual([])
    const snap = await getSnapshot(false)
    expect(snap.ok).toBe(false)
    expect(snap.error).toContain('unreachable')
  })

  it('live fetches still resolve normally', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response('{"rows":[]}', { status: 200 })),
    )
    const buf = await getBuffer()
    expect(buf.rows).toEqual([])
  })
})
