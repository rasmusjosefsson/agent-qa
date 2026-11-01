import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { RunsApi } from '../useRuns'
import { CenterPane, type RunConfig } from './CenterPane'

function scenarioRuns(): RunsApi {
  return {
    detail: null,
    scenarioDef: {
      id: 's-2026-08-13T14-42-17-736Z__197c1c78',
      intent: 'Navigate to a page and open a record',
      steps: [{ id: 's0', kind: 'do', verb: 'goto', intent: 'open the page' }],
    },
    sel: { sid: 's-2026-08-13T14-42-17-736Z__197c1c78', runId: null, stepIdx: null },
    runDefSteps: { sid: null, steps: null },
    runsBySid: {},
  } as unknown as RunsApi
}

const runConfig: RunConfig = {
  personas: [{ id: 'admin', name: 'Admin user' }],
  environments: [{ id: 'staging', name: 'Staging environment with a long name' }],
  personaId: 'admin',
  envId: 'staging',
  headed: false,
  setPersonaId() {},
  setEnvId() {},
  setHeaded() {},
}

describe('CenterPane scenario controls', () => {
  it('links an empty workbench to Chat', () => {
    const runs = {
      detail: null,
      scenarioDef: null,
      sel: { sid: null, runId: null, stepIdx: null },
      runDefSteps: { sid: null, steps: null },
      runsBySid: {},
      scenarios: [],
    } as unknown as RunsApi
    const html = renderToStaticMarkup(
      <CenterPane runs={runs} onReplay={() => {}} runConfig={runConfig} />
    )
    expect(html).toContain('Record your first scenario')
    expect(html).toContain('href="/chat"')
    expect(html).toContain('Open Chat')
  })

  it('puts replay settings in their own wrapping row under the title', () => {
    const html = renderToStaticMarkup(
      <CenterPane runs={scenarioRuns()} onReplay={() => {}} runConfig={runConfig} />
    )

    expect(html).toContain('Replay</button>')
    // Settings row wraps instead of overflowing a narrow pane, and its
    // selects can shrink below their content width.
    expect(html).toContain('flex flex-wrap items-center gap-x-2')
    expect(html).toContain('aria-label="Replay as persona"')
    expect(html).toContain('aria-label="Replay on environment"')
    expect(html).toContain('min-w-0 max-w-[16rem]')
  })

  it('shows both branches and a field for a declared input', () => {
    const runs = scenarioRuns()
    runs.scenarioDef = {
      ...runs.scenarioDef,
      inputs: { flow: { type: 'string', default: '', description: 'Empty is classic' } },
      steps: [
        {
          id: 'express',
          verb: 'when',
          params: {
            present: { role: 'button', name: 'Pay now' },
            steps: [{ id: 'pay', verb: 'click', intent: 'Pay now' }],
          },
        },
      ],
    } as RunsApi['scenarioDef']
    const html = renderToStaticMarkup(
      <CenterPane
        runs={runs}
        onReplay={() => {}}
        runConfig={{
          ...runConfig,
          inputs: [{ name: 'flow', value: '', description: 'Empty is classic' }],
          setInput() {},
        }}
      />
    )
    expect(html).toContain('Pay now')
    expect(html).toContain('only when button “Pay now” is on the page')
    expect(html).toContain('aria-label="Input flow"')
  })

  it('warns when setup has reported no progress for over a minute', () => {
    const runs = {
      detail: {
        sid: 's-stalled',
        runId: '2026-08-13T15-00-00-000Z__feedface',
        audit: { startedAt: '2020-01-01T00:00:00.000Z' },
        status: { state: 'running' },
        events: [],
      },
      scenarioDef: null,
      sel: { sid: 's-stalled', runId: '2026-08-13T15-00-00-000Z__feedface', stepIdx: null },
      runDefSteps: { sid: 's-stalled', steps: [] },
      runsBySid: {},
    } as unknown as RunsApi

    const html = renderToStaticMarkup(
      <CenterPane runs={runs} onReplay={() => {}} runConfig={runConfig} />
    )

    expect(html).toContain('No replay progress for over a minute')
    expect(html).toContain('host watchdog')
  })

  it('shows the replay scrubber when the run was recorded', () => {
    const base = {
      detail: {
        sid: 's-vid',
        runId: '2026-08-13T15-00-00-000Z__beefcafe',
        audit: { summary: 'PASS 2/2', exitCode: 0 },
        status: { state: 'done', ok: true },
        events: [
          { idx: 1, id: 'open', intent: 'Navigated to the homepage', kind: 'do:goto', status: 'pass', ms: 400 },
          { idx: 2, id: 'check', intent: 'Verified', kind: 'check', status: 'pass', ms: 200 },
        ],
        video: true,
        videoFile: 'run.mp4',
        cues: [
          { id: 'open', idx: 1, tMs: 400 },
          { id: 'check', idx: 2, tMs: 1800 },
        ],
      },
      scenarioDef: null,
      sel: { sid: 's-vid', runId: '2026-08-13T15-00-00-000Z__beefcafe', stepIdx: null },
      runDefSteps: { sid: 's-vid', steps: [] },
      runsBySid: {},
      selectStep() {},
    } as unknown as RunsApi

    const html = renderToStaticMarkup(
      <CenterPane runs={base} onReplay={() => {}} runConfig={runConfig} />
    )
    expect(html).toContain('aria-label="Replay timeline"')
    expect(html).toContain('aria-label="Playback speed"')
    expect(html).toContain('>8x</option>')
    expect(html).toContain('>Frames</button>')
    expect(html).toContain('>Video</button>')
    expect(html).toContain('<video')
    expect(html).toContain('Navigated to the homepage')

    const noVid = { ...base, detail: { ...base.detail, video: false } } as RunsApi
    const html2 = renderToStaticMarkup(
      <CenterPane runs={noVid} onReplay={() => {}} runConfig={runConfig} />
    )
    expect(html2).not.toContain('aria-label="Replay timeline"')
    expect(html2).not.toContain('<video')
  })
})
