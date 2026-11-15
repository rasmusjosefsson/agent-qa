#!/usr/bin/env node
// Record the checked-in Runs demo and write the static fixture tree.
//
//   node scripts/capture-demo-fixtures.mjs
//
// Replays two scenarios into a temp root, then packs what the report server
// would serve:
//
//   npm/agent-qa/web/fixtures/scenarios.json
//   npm/agent-qa/web/fixtures/health.json
//   npm/agent-qa/web/fixtures/<sid>/scenario.json
//   npm/agent-qa/web/fixtures/<sid>/runs.json
//   npm/agent-qa/web/fixtures/<sid>/audit-trend.json
//   npm/agent-qa/web/fixtures/<sid>/replays/latest.txt
//   npm/agent-qa/web/fixtures/<sid>/replays/<runId>/{audit,events,status,network,artifacts}
//   npm/agent-qa/web/fixtures/<sid>/replays/<runId>/{screenshots,snapshots,shots-diff,baselines}
//
// `hello` is the scenario `agent-qa init` writes (example.com). `fixture`
// drives scripts/demo-fixture-page.html: one passing run, then a revised
// page so the screenshot claim misses and shots-diff is real.
//
// Filesystem paths and home directories are scrubbed. Screenshots are scaled
// down. Video is omitted (too large for a static demo).

import { spawn, spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { createServer } from 'node:http'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { dirname, join, relative } from 'node:path'
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir, homedir } from 'node:os'

const require = createRequire(import.meta.url)
const report = require('../npm/agent-qa/lib/report-server.js')

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..')
const destRoot = join(repoRoot, 'npm', 'agent-qa', 'web', 'fixtures')
const pagePath = join(repoRoot, 'scripts', 'demo-fixture-page.html')
const DEMO_PORT = 8765
const DEMO_ORIGIN = `http://127.0.0.1:${DEMO_PORT}`

const HELLO = `{
  "schema": "scenario/2",
  "id": "hello",
  "intent": "Smoke check — the example page loads and its URL is right",
  "env": { "open": [{ "kind": "fresh" }] },
  "steps": [
    {
      "id": "s0",
      "intent": "Open the example page",
      "kind": "do",
      "verb": "goto",
      "value": { "from": "literal", "literal": "https://example.com/" }
    },
    {
      "id": "s1",
      "intent": "URL is the example page",
      "kind": "check",
      "claim": {
        "subject": { "url": true },
        "predicate": "contains",
        "value": "example.com"
      }
    }
  ]
}
`

export function fixtureScenario() {
  return {
    schema: 'scenario/2',
    id: 'fixture',
    intent: 'Save a name on the directory page and confirm the status',
    env: { open: [{ kind: 'fresh' }] },
    steps: [
      {
        id: 's0',
        intent: 'Use a compact viewport',
        kind: 'do',
        verb: 'viewport',
        params: { width: 960, height: 640 },
      },
      {
        id: 's1',
        intent: 'Open the directory',
        kind: 'do',
        verb: 'goto',
        value: { from: 'literal', literal: `${DEMO_ORIGIN}/` },
      },
      {
        id: 's2',
        intent: 'Enter a name',
        kind: 'do',
        verb: 'type',
        on: { role: 'textbox', name: 'Name' },
        value: { from: 'literal', literal: 'Ada' },
      },
      {
        id: 's3',
        intent: 'Save the name',
        kind: 'do',
        verb: 'click',
        on: { role: 'button', name: 'Save' },
      },
      {
        id: 's4',
        intent: 'Status says the name was saved',
        kind: 'check',
        claim: {
          subject: {
            element: {
              raw: { kind: 'text', value: 'Saved Ada' },
              reason: 'status text',
            },
          },
          predicate: 'isVisible',
        },
      },
      {
        id: 's5',
        intent: 'The directory screenshot matches the baseline',
        kind: 'check',
        claim: {
          subject: { shot: 's1' },
          predicate: 'matches',
          tolerance: { pixels: 0.02 },
        },
      },
    ],
  }
}

const TEXT_EXT = new Set(['.json', '.jsonl', '.txt', '.md', '.diff'])

export function scrub(text, needles) {
  let out = String(text)
  const ordered = [...needles].filter(Boolean).sort((a, b) => b.length - a.length)
  for (const needle of ordered) out = out.split(needle).join('fixtures')
  out = out.replace(/\/(?:home|Users|tmp|private\/tmp|var\/folders|agent)\/[^\s"'\\\]]+/g, 'fixtures')
  return out
}

function isTextFile(name) {
  if (name.endsWith('.diff.txt') || name.endsWith('.snap.txt') || name.endsWith('.patch.json')) return true
  const dot = name.lastIndexOf('.')
  if (dot < 0) return name === 'latest.txt'
  return TEXT_EXT.has(name.slice(dot))
}

function walkFiles(dir) {
  const out = []
  for (const name of readdirSync(dir)) {
    const full = join(dir, name)
    const st = statSync(full)
    if (st.isDirectory()) out.push(...walkFiles(full))
    else out.push(full)
  }
  return out
}

function writeJson(path, value) {
  writeFileSync(path, JSON.stringify(value, null, 2) + '\n')
}

function lastJson(stdout) {
  return report.lastJsonLine(String(stdout || ''))
}

export function shrinkPng(src, dest) {
  mkdirSync(dirname(dest), { recursive: true })
  const result = spawnSync(
    'ffmpeg',
    ['-y', '-i', src, '-vf', "scale='min(960,iw)':-1", '-pred', 'mixed', dest],
    { encoding: 'utf8' }
  )
  if (result.status !== 0 || !statSync(dest, { throwIfNoEntry: false })?.isFile()) {
    copyFileSync(src, dest)
  }
}

const KEEP_RUN_FILES = new Set([
  'audit.json',
  'events.jsonl',
  'status.json',
  'network.json',
  'heal.jsonl',
  'video-cues.json',
  'latest.txt',
])

const KEEP_DIRS = new Set(['screenshots', 'snapshots', 'shots-diff', 'domshots-diff', 'diffs', 'baselines'])

function copyScrubbed(src, dest, needles) {
  mkdirSync(dirname(dest), { recursive: true })
  if (src.endsWith('.png')) {
    shrinkPng(src, dest)
    return
  }
  if (isTextFile(src.split('/').pop())) {
    const body = scrub(readFileSync(src, 'utf8'), needles)
    if (src.endsWith('.json')) JSON.parse(body)
    writeFileSync(dest, body.endsWith('\n') ? body : body + '\n')
    return
  }
  copyFileSync(src, dest)
}

export async function packFixtures(srcRoot, outRoot, needles) {
  rmSync(outRoot, { recursive: true, force: true })
  mkdirSync(outRoot, { recursive: true })
  const listed = await report.listScenarios(srcRoot)
  const scenarios = listed.map((row) => {
    const copy = JSON.parse(scrub(JSON.stringify(row), needles))
    copy.dir = `fixtures/${row.sid}`
    return copy
  })
  writeJson(join(outRoot, 'scenarios.json'), { scenariosRoot: 'fixtures', scenarios })

  const health = await captureHealth(srcRoot, needles)
  writeJson(join(outRoot, 'health.json'), health)

  for (const row of listed) {
    const sid = row.sid
    const srcDir = join(srcRoot, sid)
    const destDir = join(outRoot, sid)
    mkdirSync(destDir, { recursive: true })
    copyScrubbed(join(srcDir, 'scenario.json'), join(destDir, 'scenario.json'), needles)
    const replays = await report.listRuns(srcDir)
    writeJson(join(destDir, 'runs.json'), {
      sid,
      replays: JSON.parse(scrub(JSON.stringify(replays), needles)),
    })
    const trend = await captureTrend(srcRoot, sid, needles)
    if (trend) writeJson(join(destDir, 'audit-trend.json'), { sid, trend })
    const latest = join(srcDir, 'replays', 'latest.txt')
    try {
      copyScrubbed(latest, join(destDir, 'replays', 'latest.txt'), needles)
    } catch {
      /* a scenario with no finished run has no pointer */
    }
    const baselineDir = join(srcDir, 'baselines')
    try {
      for (const name of readdirSync(baselineDir)) {
        copyScrubbed(join(baselineDir, name), join(destDir, 'baselines', name), needles)
      }
    } catch {
      /* no baselines */
    }
    for (const run of replays) {
      const runSrc = join(srcDir, 'replays', run.runId)
      const runDest = join(destDir, 'replays', run.runId)
      mkdirSync(runDest, { recursive: true })
      for (const name of readdirSync(runSrc)) {
        const full = join(runSrc, name)
        if (statSync(full).isDirectory()) {
          if (!KEEP_DIRS.has(name)) continue
          for (const file of walkFiles(full)) {
            const rel = relative(runSrc, file)
            copyScrubbed(file, join(runDest, rel), needles)
          }
          continue
        }
        if (!KEEP_RUN_FILES.has(name)) continue
        copyScrubbed(full, join(runDest, name), needles)
      }
      const detail = await report.runDetail(srcRoot, sid, run.runId)
      const artifacts = {
        heals: detail.heals || [],
        shotDiffs: detail.shotDiffs || [],
        domshotDiffs: detail.domshotDiffs || [],
        video: false,
        videoFile: null,
        cues: detail.cues || null,
      }
      writeJson(join(runDest, 'artifacts.json'), JSON.parse(scrub(JSON.stringify(artifacts), needles)))
    }
  }
}

async function captureHealth(srcRoot, needles) {
  const bin = process.env.AGENT_QA_BINARY_PATH
  if (!bin) return { health: [] }
  const result = spawnSync(bin, ['audit', 'health', '--json'], {
    cwd: srcRoot,
    env: { ...process.env, AGENT_QA_SCENARIOS_DIR: srcRoot },
    encoding: 'utf8',
  })
  const parsed = lastJson(result.stdout)
  const health = Array.isArray(parsed) ? parsed : []
  return JSON.parse(scrub(JSON.stringify({ health }), needles))
}

async function captureTrend(srcRoot, sid, needles) {
  const bin = process.env.AGENT_QA_BINARY_PATH
  if (!bin) return null
  const result = spawnSync(bin, ['audit', 'trend', sid, '--json'], {
    cwd: srcRoot,
    env: { ...process.env, AGENT_QA_SCENARIOS_DIR: srcRoot },
    encoding: 'utf8',
  })
  const parsed = lastJson(result.stdout)
  if (!parsed || typeof parsed !== 'object') return null
  return JSON.parse(scrub(JSON.stringify(parsed), needles))
}

function runIds(root, sid) {
  try {
    return readdirSync(join(root, sid, 'replays')).filter((name) => {
      if (name === 'latest.txt') return false
      return statSync(join(root, sid, 'replays', name)).isDirectory()
    })
  } catch {
    return []
  }
}

function replay(bin, root, sid, session) {
  // Async so the demo page server can answer while replay is in flight.
  // spawnSync would freeze that server and hang a goto to it.
  return new Promise((resolve) => {
    const child = spawn(bin, ['replay', sid, '--session', session], {
      cwd: root,
      env: {
        ...process.env,
        AGENT_QA_SCENARIOS_DIR: root,
        AGENT_BROWSER_BIN: process.env.AGENT_BROWSER_BIN,
        NO_COLOR: '1',
      },
    })
    let text = ''
    child.stdout.on('data', (chunk) => {
      text += chunk
    })
    child.stderr.on('data', (chunk) => {
      text += chunk
    })
    child.on('close', (code) => resolve({ code: code ?? 1, text }))
  })
}

function closeSession(session) {
  const bin = process.env.AGENT_BROWSER_BIN
  if (!bin) return
  spawnSync(bin, ['close', '--session', session], { encoding: 'utf8' })
}

function shotAccept(bin, root, sid) {
  const result = spawnSync(bin, ['shot-accept', sid, '--steps', 's1'], {
    cwd: root,
    env: { ...process.env, AGENT_QA_SCENARIOS_DIR: root, NO_COLOR: '1' },
    encoding: 'utf8',
  })
  if (result.status !== 0) {
    throw new Error(`shot-accept ${sid} failed\n${result.stdout}\n${result.stderr}`)
  }
}

function dropRun(root, sid, runId) {
  rmSync(join(root, sid, 'replays', runId), { recursive: true, force: true })
}

function startPage(rev) {
  const html = readFileSync(pagePath, 'utf8')
  const server = createServer((_req, res) => {
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' })
    res.end(html.replaceAll('__PAGE_REV__', String(rev.current)))
  })
  return new Promise((resolve, reject) => {
    server.once('error', reject)
    server.listen(DEMO_PORT, '127.0.0.1', () => resolve(server))
  })
}

function resolveBinary() {
  if (process.env.AGENT_QA_BINARY_PATH) return process.env.AGENT_QA_BINARY_PATH
  const debug = join(repoRoot, 'cli', 'target', 'debug', 'agent-qa')
  const release = join(repoRoot, 'cli', 'target', 'release', 'agent-qa')
  try {
    statSync(debug)
    return debug
  } catch {
    return release
  }
}

export async function recordFixtures(root) {
  const bin = resolveBinary()
  process.env.AGENT_QA_BINARY_PATH = bin
  mkdirSync(join(root, 'hello'), { recursive: true })
  mkdirSync(join(root, 'fixture'), { recursive: true })
  writeFileSync(join(root, 'hello', 'scenario.json'), HELLO)
  writeJson(join(root, 'fixture', 'scenario.json'), fixtureScenario())

  console.error('replay hello')
  const hello = await replay(bin, root, 'hello', 'demo-hello')
  closeSession('demo-hello')
  if (hello.code !== 0 || !/SUMMARY: .*PASS/.test(hello.text)) {
    throw new Error(`hello replay failed (${hello.code})\n${hello.text}`)
  }

  const rev = { current: 1 }
  const server = await startPage(rev)
  try {
    console.error('replay fixture (seed baseline)')
    const seeded = await replay(bin, root, 'fixture', 'demo-fixture-seed')
    closeSession('demo-fixture-seed')
    const seedIds = runIds(root, 'fixture')
    if (seedIds.length !== 1) throw new Error(`expected one seed run, saw ${seedIds.join(',')}\n${seeded.text}`)
    shotAccept(bin, root, 'fixture')
    dropRun(root, 'fixture', seedIds[0])

    console.error('replay fixture (pass)')
    const passed = await replay(bin, root, 'fixture', 'demo-fixture-pass')
    closeSession('demo-fixture-pass')
    if (passed.code !== 0 || !/SUMMARY: .*PASS/.test(passed.text)) {
      throw new Error(`fixture pass replay failed (${passed.code})\n${passed.text}`)
    }

    rev.current = 2
    console.error('replay fixture (drift)')
    const drifted = await replay(bin, root, 'fixture', 'demo-fixture-drift')
    closeSession('demo-fixture-drift')
    if (!/FAIL/.test(drifted.text)) {
      throw new Error(`fixture drift replay did not fail\n${drifted.text}`)
    }
    const runs = runIds(root, 'fixture')
    if (runs.length !== 2) throw new Error(`expected pass + drift runs, saw ${runs.join(',')}`)
  } finally {
    await new Promise((resolve) => server.close(resolve))
  }
}

async function main() {
  const scratch = mkdtempSync(join(tmpdir(), 'aqa-demo-'))
  const needles = [scratch, repoRoot, homedir()]
  try {
    await recordFixtures(scratch)
    await packFixtures(scratch, destRoot, needles)
    const leaked = walkFiles(destRoot).filter((file) => {
      if (file.endsWith('.png')) return false
      const body = readFileSync(file, 'utf8')
      return /\/home\/|\/Users\/|\/tmp\/|\/agent\/|AGENT_QA|api[_-]?key|password|secret/i.test(body)
    })
    if (leaked.length) {
      throw new Error(`fixture tree still has local paths or secret-like text:\n${leaked.join('\n')}`)
    }
    const bytes = walkFiles(destRoot).reduce((sum, file) => sum + statSync(file).size, 0)
    console.log(`wrote ${destRoot} (${walkFiles(destRoot).length} files, ${bytes} bytes)`)
  } finally {
    rmSync(scratch, { recursive: true, force: true })
  }
}

const invoked = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href
if (invoked) {
  main().catch((err) => {
    console.error(err)
    process.exit(1)
  })
}
