import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { packFixtures, scrub } from './capture-demo-fixtures.mjs'

test('scrub replaces scratch paths and home directories', () => {
  const text = 'see /tmp/aqa-demo-abc/hello and /home/ubuntu/.agent-browser/chrome'
  assert.equal(scrub(text, ['/tmp/aqa-demo-abc']), 'see fixtures/hello and fixtures')
})

test('packFixtures writes the static tree and drops absolute paths', async () => {
  const prev = process.env.AGENT_QA_BINARY_PATH
  delete process.env.AGENT_QA_BINARY_PATH
  const src = mkdtempSync(join(tmpdir(), 'aqa-pack-src-'))
  const dest = mkdtempSync(join(tmpdir(), 'aqa-pack-dest-'))
  try {
    const run = join(src, 'hello', 'replays', 'r-1')
    mkdirSync(join(run, 'screenshots'), { recursive: true })
    writeFileSync(
      join(src, 'hello', 'scenario.json'),
      JSON.stringify({
        schema: 'scenario/2',
        id: 'hello',
        intent: 'Smoke check',
        steps: [{ id: 's0', kind: 'do', verb: 'goto' }],
      })
    )
    writeFileSync(join(run, 'audit.json'), `{"summary":"SUMMARY: 1/1 (PASS)","exitCode":0,"log":"${src}/hello"}\n`)
    writeFileSync(join(run, 'events.jsonl'), '{"idx":1,"id":"s0","status":"pass","ms":10}\n')
    writeFileSync(join(run, 'status.json'), '{"state":"done","ok":true}\n')
    writeFileSync(join(src, 'hello', 'replays', 'latest.txt'), 'r-1\n')
    writeFileSync(join(run, 'screenshots', 's0.png'), Buffer.from([0x89, 0x50, 0x4e, 0x47]))
    await packFixtures(src, dest, [src])
    const scenarios = JSON.parse(readFileSync(join(dest, 'scenarios.json'), 'utf8'))
    assert.equal(scenarios.scenariosRoot, 'fixtures')
    assert.equal(scenarios.scenarios[0].dir, 'fixtures/hello')
    assert.equal(scenarios.scenarios[0].sid, 'hello')
    const audit = readFileSync(join(dest, 'hello', 'replays', 'r-1', 'audit.json'), 'utf8')
    assert.equal(audit.includes(src), false)
    assert.match(audit, /fixtures\/hello/)
    const runs = JSON.parse(readFileSync(join(dest, 'hello', 'runs.json'), 'utf8'))
    assert.equal(runs.replays[0].runId, 'r-1')
    const artifacts = JSON.parse(readFileSync(join(dest, 'hello', 'replays', 'r-1', 'artifacts.json'), 'utf8'))
    assert.deepEqual(artifacts.shotDiffs, [])
    assert.equal(artifacts.video, false)
  } finally {
    if (prev === undefined) delete process.env.AGENT_QA_BINARY_PATH
    else process.env.AGENT_QA_BINARY_PATH = prev
    rmSync(src, { recursive: true, force: true })
    rmSync(dest, { recursive: true, force: true })
  }
})
