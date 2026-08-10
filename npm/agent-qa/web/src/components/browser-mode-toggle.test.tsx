import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { BrowserModeToggle } from './browser-mode-toggle'

describe('BrowserModeToggle', () => {
  it('renders the headless default as an accessible switch', () => {
    const html = renderToStaticMarkup(
      <BrowserModeToggle headed={false} onChange={() => {}} />
    )

    expect(html).toContain('role="switch"')
    expect(html).toContain('aria-label="Show browser window"')
    expect(html).toContain('aria-checked="false"')
    expect(html).toContain('Headless')
  })

  it('labels headed mode as visible', () => {
    const html = renderToStaticMarkup(
      <BrowserModeToggle headed onChange={() => {}} disabled />
    )

    expect(html).toContain('aria-checked="true"')
    expect(html).toContain('Browser visible')
    expect(html).toContain('disabled=""')
  })
})
