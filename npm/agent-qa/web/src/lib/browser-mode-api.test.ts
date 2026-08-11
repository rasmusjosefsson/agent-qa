import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { runPlan } from './plans-api'
import { connectPersona } from './run-config-api'
import { startReplay } from './runs-api'

const fetchMock = vi.fn<typeof fetch>()

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

function requestBody(call: unknown[]): Record<string, unknown> {
  const init = call[1] as RequestInit
  return JSON.parse(String(init.body)) as Record<string, unknown>
}

describe('browser mode API payloads', () => {
  beforeEach(() => {
    fetchMock.mockReset()
    vi.stubGlobal('fetch', fetchMock)
  })

  afterEach(() => vi.unstubAllGlobals())

  it('sends headed mode for scenario and plan replays', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ ok: true }, 202))
      .mockResolvedValueOnce(jsonResponse({ ok: true, started: [], skipped: [] }, 202))

    await startReplay('s-demo', { headed: true })
    await runPlan('smoke', { headed: true })

    expect(requestBody(fetchMock.mock.calls[0])).toMatchObject({ headed: true })
    expect(requestBody(fetchMock.mock.calls[1])).toMatchObject({ headed: true })
  })

  it('defaults persona connect to headless and forwards an explicit headed choice', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ ok: true }))
      .mockResolvedValueOnce(jsonResponse({ ok: true }))

    await connectPersona('viewer', 'test')
    await connectPersona('viewer', 'test', true)

    expect(requestBody(fetchMock.mock.calls[0])).toMatchObject({ headed: false })
    expect(requestBody(fetchMock.mock.calls[1])).toMatchObject({ headed: true })
  })
})
