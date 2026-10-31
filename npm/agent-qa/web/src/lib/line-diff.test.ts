import { describe, expect, it } from 'vitest'
import { lineDiff, splitLines } from './line-diff'

describe('lineDiff', () => {
  it('treats a shared trailing newline as no extra blank line', () => {
    expect(splitLines('a\nb\n')).toEqual(['a', 'b'])
    expect(splitLines('')).toEqual([])
  })

  it('aligns an insertion between unchanged lines', () => {
    const rows = lineDiff('heading "Home"\nbutton "Save"\n', 'heading "Home"\nlink "Docs"\nbutton "Save"\n')
    expect(rows.map((r) => r.kind)).toEqual(['same', 'add', 'same'])
    expect(rows[1].after).toBe('link "Docs"')
  })

  it('pairs a rewritten line and highlights the changed token', () => {
    const rows = lineDiff('heading "Old title"\n', 'heading "New title"\n')
    expect(rows).toHaveLength(1)
    expect(rows[0].kind).toBe('change')
    const changed = rows[0].beforeSpans?.filter((s) => s.changed).map((s) => s.text)
    expect(changed).toEqual(['"Old'])
    const afterChanged = rows[0].afterSpans?.filter((s) => s.changed).map((s) => s.text)
    expect(afterChanged).toEqual(['"New'])
  })

  it('keeps a pure deletion on the baseline side only', () => {
    const rows = lineDiff('button "Save"\n', '')
    expect(rows).toEqual([{ kind: 'del', before: 'button "Save"' }])
  })
})
