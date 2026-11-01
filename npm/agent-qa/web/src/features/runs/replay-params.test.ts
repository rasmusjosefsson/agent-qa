import { describe, expect, it } from 'vitest'
import { replayParams, visibleInputs } from './replay-params'

describe('visibleInputs', () => {
  it('shows defaults and hides secrets', () => {
    expect(
      visibleInputs({
        flow: { type: 'string', default: '', description: 'Empty is classic' },
        token: { type: 'string', sensitive: true, default: 'secret' },
      }),
    ).toEqual([{ name: 'flow', value: '', description: 'Empty is classic' }])
  })
})

describe('replayParams', () => {
  it('sends the flag the form shows and drops keys this scenario does not declare', () => {
    expect(
      replayParams(
        { flow: { type: 'string', default: '' } },
        { flow: 'new', region: 'eu' },
        { flow: '' },
      ),
    ).toEqual({ flow: '' })
  })

  it('keeps an environment value for a declared input the form is not editing', () => {
    expect(replayParams({ flow: { type: 'string' } }, { flow: 'new' }, {})).toEqual({ flow: 'new' })
  })

  it('sends nothing when the scenario declares no inputs', () => {
    expect(replayParams(undefined, { flow: 'new' }, { flow: 'new' })).toBeUndefined()
    expect(replayParams({}, { flow: 'new' }, {})).toBeUndefined()
  })
})
