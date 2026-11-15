import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { RunsApi } from '../useRuns'
import { DomshotDiffCard, RunTraffic, ShotDiffCard, StepDetail } from './StepDetail'

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

describe('failed shot step', () => {
  it('shows that step’s error beside the screenshot the claim compared, its baseline, and its diff', () => {
    const error =
      "shot 's1' differs from baseline: 4.28% pixels changed (tolerance 2.00%) — diff at shots-diff/s1.diff.png"
    const runs = {
      detail: {
        sid: 'fixture',
        runId: 'run-1',
        audit: { summary: 'SUMMARY: 5/6 (FAIL)' },
        status: { state: 'done' },
        shotDiffs: ['s1'],
        events: [
          { idx: 1, id: 's0', intent: 'Use a compact viewport', status: 'pass', screenshot: 'screenshots/s0.png' },
          { idx: 2, id: 's1', intent: 'Open the directory', status: 'pass', screenshot: 'screenshots/s1.png' },
          { idx: 5, id: 's4', intent: 'Status says the name was saved', status: 'pass', screenshot: 'screenshots/s4.png' },
          {
            idx: 6,
            id: 's5',
            intent: 'The directory screenshot matches the baseline',
            status: 'fail',
            error,
            screenshot: 'screenshots/s5.png',
          },
        ],
      },
      sel: { sid: 'fixture', runId: 'run-1', stepIdx: 6, tab: 'step' },
      selectTab() {},
    } as unknown as RunsApi
    const html = renderToStaticMarkup(<StepDetail runs={runs} onLightbox={() => {}} />)
    expect(html).toContain('shot &#x27;s1&#x27; differs from baseline')
    expect(html).toContain('/artifact/screenshots/s1')
    expect(html).toContain('/baselines/s1.png')
    expect(html).toContain('/artifact/shots-diff/s1')
    expect(html).not.toContain('/artifact/screenshots/s5')
    expect(html).not.toContain('/artifact/screenshots/s4')
    expect(html).not.toContain('/artifact/screenshots/s0')
  })
})

describe('passing step frames', () => {
  it('does not show a blank viewport capture as the step image', () => {
    const runs = {
      detail: {
        sid: 'fixture',
        runId: 'pass-1',
        audit: { summary: 'SUMMARY: 6/6 (PASS)' },
        status: { state: 'done', ok: true },
        events: [
          { idx: 1, id: 's0', intent: 'Use a compact viewport', kind: 'do:viewport', status: 'pass', screenshot: 'screenshots/s0.png' },
          { idx: 2, id: 's1', intent: 'Open the directory', kind: 'do:goto', status: 'pass', screenshot: 'screenshots/s1.png' },
        ],
        shotDiffs: [],
      },
      sel: { sid: 'fixture', runId: 'pass-1', stepIdx: 1, tab: 'step' },
      selectTab() {},
    } as unknown as RunsApi
    const html = renderToStaticMarkup(<StepDetail runs={runs} onLightbox={() => {}} />)
    expect(html).toContain('not captured')
    expect(html).not.toContain('/artifact/screenshots/s0')
    expect(html).not.toContain('Baseline')
    expect(html).not.toContain('>Diff<')
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
