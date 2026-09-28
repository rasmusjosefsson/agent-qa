import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { RunTraffic, ShotDiffCard } from './StepDetail'

describe('ShotDiffCard', () => {
  it('renders the diff map via the shots-diff artifact route', () => {
    const html = renderToStaticMarkup(
      <ShotDiffCard sid="s1" runId="r9" shotStep="openDialog" onLightbox={() => {}} />
    )
    expect(html).toContain('/api/scenarios/s1/runs/r9/artifact/shots-diff/openDialog')
    expect(html).toContain('Visual diff')
    expect(html).toContain('Re-mint baseline')
  })
})

describe('RunTraffic', () => {
  it('renders one row per captured request with method, url, and status', () => {
    const html = renderToStaticMarkup(
      <RunTraffic
        network={{
          requestCount: 2,
          requests: [
            { requestId: 'r1', url: 'https://api.example.com/users', method: 'GET', status: 200 },
            { requestId: 'r2', url: 'https://api.example.com/save', method: 'POST', status: 500 },
          ],
        }}
      />
    )
    expect(html).toContain('2 request(s)')
    expect(html).toContain('GET')
    expect(html).toContain('https://api.example.com/save')
    expect(html).toContain('500')
  })

  it('explains when the run predates the network.json artifact', () => {
    const html = renderToStaticMarkup(<RunTraffic network={null} />)
    expect(html).toContain('network.json not written')
  })
})
