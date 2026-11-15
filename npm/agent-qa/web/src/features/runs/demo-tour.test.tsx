import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { DemoBanner } from './components/DemoBanner'
import { tourStepEvent } from './components/DemoTour'
import { DEMO_BANNER, TOUR_STEPS } from './demo-tour'

describe('demo tour', () => {
  it('uses the read-only banner sentence and eight stops', () => {
    expect(DEMO_BANNER).toBe('Demo, run agent-qa locally to record your own')
    expect(TOUR_STEPS).toHaveLength(8)
    expect(TOUR_STEPS.map((step) => step.title)).toEqual([
      'Runs list',
      'Run detail',
      'Timeline playback',
      'Screenshots and snapshots',
      'Checks and claims',
      'Heals',
      'Diffs',
      'Run it locally',
    ])
  })

  it('renders the banner sentence, the quickstart link, and the tour button', () => {
    const html = renderToStaticMarkup(<DemoBanner />)
    expect(html).toContain(DEMO_BANNER)
    expect(html).toContain('href="/agent-qa/docs/quickstart/"')
    expect(html).toContain('Take the tour')
  })

  it('walks a passing run on a page frame, not the blank viewport', () => {
    const events = [
      { idx: 1, id: 's0', status: 'pass', kind: 'do:viewport', screenshot: 'screenshots/s0.png' },
      { idx: 2, id: 's1', status: 'pass', kind: 'do:goto', screenshot: 'screenshots/s1.png' },
      { idx: 3, id: 's2', status: 'pass', kind: 'do:type', screenshot: 'screenshots/s2.png' },
      { idx: 4, id: 's3', status: 'pass', kind: 'do:click', screenshot: 'screenshots/s3.png' },
    ]
    expect(tourStepEvent(events, 'run')?.id).toBe('s1')
    expect(tourStepEvent(events, 'playback')?.id).toBe('s2')
    expect(tourStepEvent([...events, { idx: 6, id: 's5', status: 'fail', kind: 'check' }], 'playback')?.id).toBe('s5')
  })
})
