import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it } from 'vitest'
import { createFixtureSource } from '@/lib/source/fixture-source'
import { setScenarioSourceForTests } from '@/lib/source'
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
      <CenterPane runs={runs} runConfig={runConfig} />
    )
    expect(html).toContain('Record your first scenario')
    expect(html).toContain('href="/chat"')
    expect(html).toContain('Open Chat')
  })

  it('keeps run settings under the title and leaves Replay on the scenario card', () => {
    const html = renderToStaticMarkup(
      <CenterPane runs={scenarioRuns()} runConfig={runConfig} />
    )

    expect(html).not.toContain('>Replay</button>')
    expect(html).not.toContain('aria-label="Headless"')
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
      <CenterPane runs={runs} runConfig={runConfig} />
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
      <CenterPane runs={base} runConfig={runConfig} />
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
      <CenterPane runs={noVid} runConfig={runConfig} />
    )
    expect(html2).not.toContain('aria-label="Replay timeline"')
    expect(html2).not.toContain('aria-label="Event timeline"')
    expect(html2).not.toContain('<video')
    // Replay belongs to the scenario card. A past run does not grow its own.
    expect(html).not.toContain('>Replay</button>')
    expect(html2).not.toContain('>Replay</button>')
  })

  it('plays events.jsonl when the source cannot write and there is no video', () => {
    setScenarioSourceForTests(createFixtureSource('/demo'))
    const runs = {
      detail: {
        sid: 's-1',
        runId: 'r-1',
        audit: { summary: 'SUMMARY: 2/2 (PASS)', exitCode: 0 },
        status: { state: 'done', ok: true },
        events: [
          { idx: 1, id: 'open', intent: 'Open the directory', kind: 'do:goto', status: 'pass', ms: 400, screenshot: 'screenshots/open.png' },
          { idx: 2, id: 'save', intent: 'Save the name', kind: 'do:click', status: 'pass', ms: 200, screenshot: 'screenshots/save.png' },
        ],
        video: false,
      },
      scenarioDef: null,
      sel: { sid: 's-1', runId: 'r-1', stepIdx: null },
      runDefSteps: { sid: 's-1', steps: [] },
      runsBySid: {},
      selectStep() {},
    } as unknown as RunsApi
    const html = renderToStaticMarkup(<CenterPane runs={runs} runConfig={runConfig} />)
    expect(html).toContain('data-testid="event-playback"')
    expect(html).toContain('aria-label="Event timeline"')
    expect(html).toContain('aria-label="Play"')
    expect(html).toContain('aria-label="Playback speed"')
    expect(html).toContain('>8x</option>')
    expect(html).toContain('Open the directory')
    expect(html).toContain('/demo/s-1/replays/r-1/screenshots/open.png')
    expect(html).not.toContain('<video')
  })

  it('opens a shot miss on the screenshot the error names, not the first frame', () => {
    setScenarioSourceForTests(createFixtureSource('/demo'))
    const error = "shot 's1' differs from baseline: 4.28% pixels changed"
    const runs = {
      detail: {
        sid: 'fixture',
        runId: 'run-1',
        audit: { summary: 'SUMMARY: 5/6 (FAIL)', exitCode: 1 },
        status: { state: 'done', ok: false },
        events: [
          { idx: 1, id: 's0', intent: 'Use a compact viewport', status: 'pass', ms: 100, screenshot: 'screenshots/s0.png' },
          { idx: 6, id: 's5', intent: 'The directory screenshot matches the baseline', status: 'fail', ms: 360, error, screenshot: 'screenshots/s5.png' },
        ],
        video: false,
      },
      scenarioDef: null,
      sel: { sid: 'fixture', runId: 'run-1', stepIdx: null },
      runDefSteps: { sid: 'fixture', steps: [] },
      runsBySid: {},
      selectStep() {},
    } as unknown as RunsApi
    const html = renderToStaticMarkup(<CenterPane runs={runs} runConfig={runConfig} />)
    expect(html).toContain('shot &#x27;s1&#x27; differs from baseline')
    expect(html).toContain('/demo/fixture/replays/run-1/screenshots/s1.png')
    expect(html).not.toContain('/screenshots/s0.png')
    expect(html).not.toContain('/screenshots/s5.png')
  })

  it('opens a passing run on the first page frame, not a blank viewport', () => {
    setScenarioSourceForTests(createFixtureSource('/demo'))
    const runs = {
      detail: {
        sid: 'fixture',
        runId: 'pass-1',
        audit: { summary: 'SUMMARY: 6/6 (PASS)', exitCode: 0 },
        status: { state: 'done', ok: true },
        events: [
          { idx: 1, id: 's0', intent: 'Use a compact viewport', kind: 'do:viewport', status: 'pass', ms: 100, screenshot: 'screenshots/s0.png' },
          { idx: 2, id: 's1', intent: 'Open the directory', kind: 'do:goto', status: 'pass', ms: 1500, screenshot: 'screenshots/s1.png' },
          { idx: 3, id: 's2', intent: 'Enter a name', kind: 'do:type', status: 'pass', ms: 200, screenshot: 'screenshots/s2.png' },
        ],
        video: false,
        shotDiffs: [],
      },
      scenarioDef: null,
      sel: { sid: 'fixture', runId: 'pass-1', stepIdx: null },
      runDefSteps: { sid: 'fixture', steps: [] },
      runsBySid: {},
      selectStep() {},
    } as unknown as RunsApi
    const html = renderToStaticMarkup(<CenterPane runs={runs} runConfig={runConfig} />)
    expect(html).toContain('/demo/fixture/replays/pass-1/screenshots/s1.png')
    expect(html).toContain('Open the directory')
    expect(html).not.toContain('/screenshots/s0.png')
    expect(html).not.toContain('Baseline')
    expect(html).not.toContain('>Diff<')
  })

  it('hides compare, accept, and insert when the source cannot write', () => {
    setScenarioSourceForTests(createFixtureSource('/demo'))
    const runs = {
      detail: {
        sid: 's-1',
        runId: 'r-2',
        audit: { summary: 'SUMMARY: 1/1 (PASS)', exitCode: 0 },
        status: { state: 'done', ok: true },
        events: [{ idx: 1, id: 'open', intent: 'Open', kind: 'do:goto', status: 'pass', ms: 100 }],
        video: false,
      },
      scenarioDef: { id: 's-1', steps: [{ id: 'open', kind: 'do', verb: 'goto', intent: 'Open' }] },
      sel: { sid: 's-1', runId: 'r-2', stepIdx: null },
      runDefSteps: {
        sid: 's-1',
        steps: [
          { id: 'open', kind: 'do', verb: 'goto' },
          { id: 'shot', kind: 'check', claim: { subject: { shot: 'open' } } },
        ],
      },
      runsBySid: {
        's-1': [
          { runId: 'r-1', state: 'done', summary: 'SUMMARY: 1/1 (PASS)' },
          { runId: 'r-2', state: 'done', summary: 'SUMMARY: 1/1 (PASS)' },
        ],
      },
      selectStep() {},
    } as unknown as RunsApi
    const html = renderToStaticMarkup(<CenterPane runs={runs} runConfig={runConfig} />)
    expect(html).not.toContain('Compare</button>')
    expect(html).not.toContain('Accept goldens')
    expect(html).not.toContain('Insert a check')
    expect(html).toContain('data-tour="run"')
    expect(html).toContain('data-tour="steps"')
  })
})

afterEach(() => setScenarioSourceForTests(null))
