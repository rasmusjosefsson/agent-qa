import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { StepList } from './StepList'
import type { BufferRow } from '../types'

const rows: BufferRow[] = [
  {
    stepIndex: 0,
    stepId: 's0',
    step: { id: 's0', kind: 'do', intent: 'open page', verb: 'goto' },
  },
  {
    stepIndex: 1,
    stepId: 's1',
    step: { id: 's1', kind: 'check', intent: 'on home', claim: { subject: { url: true }, predicate: 'contains' } },
  },
]

describe('StepList', () => {
  it('offers a visual-check action on do steps only', () => {
    const html = renderToStaticMarkup(
      <StepList rows={rows} onMove={() => {}} onDelete={() => {}} onAddShot={() => {}} />
    )
    // Exactly one camera button: the do-step's (title + aria-label both
    // carry the text, so 2 string matches per button); the check row has none.
    expect(html.match(/Add a visual check/g)!.length).toBe(2)
  })

  it('hides the action when no handler is provided', () => {
    const html = renderToStaticMarkup(<StepList rows={rows} onMove={() => {}} onDelete={() => {}} />)
    expect(html).not.toContain('Add a visual check')
  })
})
