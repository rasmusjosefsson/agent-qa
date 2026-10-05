import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { DomshotDiffCard, RunTraffic, ShotDiffCard } from './StepDetail'

describe('ShotDiffCard', () => {
  it('defaults to the before/after slider (baseline vs run) with a diff-map toggle', () => {
    const html = renderToStaticMarkup(
      <ShotDiffCard sid="s1" runId="r9" shotStep="openDialog" onLightbox={() => {}} />
    )
    expect(html).toContain('/api/scenarios/s1/baselines/openDialog.png')
    expect(html).toContain('/api/scenarios/s1/runs/r9/artifact/screenshots/openDialog')
    expect(html).toContain('before/after')
    expect(html).toContain('diff map')
    expect(html).toContain('Visual diff')
    expect(html).toContain('Re-mint baseline')
  })
})

describe('DomshotDiffCard', () => {
  it('renders the structural-diff header + re-mint affordance', () => {
    const html = renderToStaticMarkup(
      <DomshotDiffCard sid="s1" runId="r9" domshotStep="openDialog" />
    )
    expect(html).toContain('Structural diff')
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
