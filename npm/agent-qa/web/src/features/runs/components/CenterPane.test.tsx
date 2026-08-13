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
  it('keeps replay controls in a full-width wrapping row until the pane is wide', () => {
    const html = renderToStaticMarkup(
      <CenterPane runs={scenarioRuns()} onReplay={() => {}} runConfig={runConfig} />
    )

    expect(html).toContain('Replay</button>')
    expect(html).toContain('@3xl:flex-row')
    expect(html).toContain('w-full min-w-0 flex-wrap')
    expect(html).toContain('max-w-full flex-1')
    expect(html).not.toContain('@md:flex-row')
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
})
