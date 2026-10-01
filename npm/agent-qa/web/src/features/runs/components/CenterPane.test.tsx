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

  it('warns when setup has reported no progress for over a minute', () => {
    const runs = {
      detail: {
        sid: 's-stalled',
        runId: '2026-08-13T15-00-00-000Z__feedface',
        audit: { startedAt: '2020-01-01T00:00:00.000Z' },
        status: null,
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

  it('shows a video chip when the run was recorded (--record-video)', () => {
    const base = {
      detail: {
        sid: 's-vid',
        runId: '2026-08-13T15-00-00-000Z__beefcafe',
        audit: { summary: 'PASS 2/2', exitCode: 0 },
        status: { state: 'done', ok: true },
        events: [],
        video: true,
      },
      scenarioDef: null,
      sel: { sid: 's-vid', runId: '2026-08-13T15-00-00-000Z__beefcafe', stepIdx: null },
      runDefSteps: { sid: 's-vid', steps: [] },
      runsBySid: {},
    } as unknown as RunsApi

    const html = renderToStaticMarkup(
      <CenterPane runs={base} onReplay={() => {}} runConfig={runConfig} />
    )
    expect(html).toContain('>video</button>')
    expect(html).not.toContain('<video')

    const noVid = { ...base, detail: { ...base.detail, video: false } } as RunsApi
    const html2 = renderToStaticMarkup(
      <CenterPane runs={noVid} onReplay={() => {}} runConfig={runConfig} />
    )
    expect(html2).not.toContain('>video</button>')
  })
})
