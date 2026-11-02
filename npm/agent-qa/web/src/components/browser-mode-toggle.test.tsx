import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { BrowserModeToggle } from './browser-mode-toggle'

describe('BrowserModeToggle', () => {
  it('keeps the Headless label and shows the switch on', () => {
    const html = renderToStaticMarkup(
      <BrowserModeToggle headed={false} onChange={() => {}} />
    )

    expect(html).toContain('role="switch"')
    expect(html).toContain('aria-label="Headless"')
    expect(html).toContain('aria-checked="true"')
    expect(html).toContain('>Headless</span>')
    expect(html).not.toContain('Browser visible')
    expect(html).toContain('translate-x-3')
  })

  it('stays labeled Headless and turns the switch off when a window is shown', () => {
    const html = renderToStaticMarkup(
      <BrowserModeToggle headed onChange={() => {}} disabled />
    )

    expect(html).toContain('aria-label="Headless"')
    expect(html).toContain('aria-checked="false"')
    expect(html).toContain('>Headless</span>')
    expect(html).not.toContain('Browser visible')
    expect(html).toContain('translate-x-0')
    expect(html).toContain('disabled=""')
  })
})
