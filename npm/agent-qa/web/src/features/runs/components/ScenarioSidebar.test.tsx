import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { RunsApi } from '../useRuns'
import type { ScenarioSummary } from '../types'
import { flakyTitle, ScenarioSidebar } from './ScenarioSidebar'

function sc(sid: string, intent: string): ScenarioSummary {
  return {
    sid,
    dir: `/x/${sid}`,
    scenarioId: sid,
    hasScenario: true,
    intent,
    steps: 3,
    coverage: null,
    tags: [],
    latestRunId: null,
    activeRunId: null,
    latestRun: null,
  }
}

function runsApi(healthBySid: RunsApi['healthBySid']): RunsApi {
  return {
    scenarios: [sc('s-login', 'Log in and open dashboard'), sc('s-quiet', 'Clean scenario')],
    healthBySid,
    runsBySid: {},
    expanded: new Set(),
    sel: { sid: null, runId: null, stepIdx: null, tab: 'step' },
  } as unknown as RunsApi
}

describe('ScenarioSidebar health badges', () => {
  it('renders a pill per flagged detector with its step count', () => {
    const html = renderToStaticMarkup(
      <ScenarioSidebar
        runs={runsApi({
          's-login': { scenarioId: 's-login', flaky: ['s1', 's2'], slow: ['s3'], chronic: ['s4'] },
        })}
      />
    )
    expect(html).toContain('2 flaky')
    expect(html).toContain('1 slow')
    expect(html).toContain('1 chronic')
    expect(html).toContain('s1 — passed and failed across runs')
    expect(html).toContain('audit heal-chronic: s4')
  })

  it('names a flaky step and why its option text missed', () => {
    expect(
      flakyTitle({
        flaky: ['s11'],
        flakyNotes: [
          {
            stepId: 's11',
            intent: 'smart-select To Account',
            lastError:
              'select: opened combobox "To Account" but found no option matching "High-Yield Savings — $12,800.00"',
          },
        ],
      })
    ).toBe('To Account — the option “High-Yield Savings — $12,800.00” was not in the list')
  })

  it('omits detectors with no flagged steps and scenarios with no flags', () => {
    const html = renderToStaticMarkup(
      <ScenarioSidebar
        runs={runsApi({
          's-login': { scenarioId: 's-login', flaky: [], slow: ['s3'], chronic: [] },
        })}
      />
    )
    expect(html).toContain('1 slow')
    expect(html).not.toContain('flaky')
    expect(html).not.toContain('chronic')
  })
})
