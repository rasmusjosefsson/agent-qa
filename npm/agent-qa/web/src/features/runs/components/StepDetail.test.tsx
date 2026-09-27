import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { ShotDiffCard } from './StepDetail'

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
