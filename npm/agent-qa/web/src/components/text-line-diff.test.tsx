import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { TextLineDiff } from './text-line-diff'

describe('TextLineDiff', () => {
  it('renders a side-by-side line diff with the changed token', () => {
    const html = renderToStaticMarkup(
      <TextLineDiff before={'heading "Old title"\n'} after={'heading "New title"\n'} />
    )
    expect(html).toContain('side-by-side')
    expect(html).toContain('unified')
    expect(html).toContain('1 changed line')
    expect(html).toContain('Old')
    expect(html).toContain('New')
    expect(html).toContain('baseline')
    expect(html).toContain('current')
  })

  it('says the snapshots match when the texts are equal', () => {
    const html = renderToStaticMarkup(<TextLineDiff before={'button "Save"\n'} after={'button "Save"\n'} />)
    expect(html).toContain('snapshots match')
    expect(html).toContain('The two snapshots are identical.')
  })
})