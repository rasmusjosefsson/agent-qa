#!/usr/bin/env node
// Load the built demo the way GitHub Pages does: files under /agent-qa/play/,
// and 404.html for every other /play route. Visit each workbench route with a
// fresh document, play a recorded run, and fail on console errors or /api calls.

import { spawn, spawnSync } from 'node:child_process'
import { createReadStream, existsSync, mkdirSync, readFileSync, statSync, symlinkSync } from 'node:fs'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { dirname, extname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { writePlayFallback } from './play-spa-fallback.mjs'

const repo = join(dirname(fileURLToPath(import.meta.url)), '..')
const playDir = join(repo, 'site', 'public', 'play')
const browser =
  process.env.AGENT_BROWSER_BIN ||
  join(repo, 'npm', 'agent-qa', 'node_modules', '.bin', 'agent-browser')

const ROUTES = [
  ['/', '[data-demo-ready]'],
  ['/chat', '[data-testid="demo-local-only"]'],
  ['/editor', '[data-testid="demo-local-only"]'],
  ['/cases', '[data-testid="demo-local-only"]'],
  ['/sets', '[data-testid="demo-local-only"]'],
  ['/plans', '[data-testid="demo-local-only"]'],
  ['/goldens', '[data-testid="demo-local-only"]'],
  ['/personas', '[data-testid="demo-local-only"]'],
  ['/environments', '[data-testid="demo-local-only"]'],
  ['/knowledge', '[data-testid="demo-local-only"]'],
  ['/plugins', '[data-testid="demo-local-only"]'],
  ['/settings', '[data-testid="demo-local-only"]'],
]

const failures = []

function fail(message) {
  failures.push(message)
  console.error(`FAIL ${message}`)
}

function run(args, { allowFail = false } = {}) {
  // Async so the static server in this process can answer the browser.
  return new Promise((resolve, reject) => {
    const child = spawn(browser, ['--session', `play-smoke-${process.pid}`, ...args], { env: process.env })
    let out = ''
    child.stdout.on('data', (chunk) => {
      out += chunk
    })
    child.stderr.on('data', (chunk) => {
      out += chunk
    })
    child.on('close', (status) => {
      out = out.trim()
      if (status !== 0 && !allowFail) {
        fail(`${args[0]} exited ${status}\n${out}`)
        if (args[0] === 'open' || args[0] === 'wait') {
          reject(new Error(`browser stopped responding during ${args[0]}`))
          return
        }
      }
      resolve(out)
    })
  })
}

function isClean(text) {
  const body = text.replace(/^\[agent-browser\][^\n]*\n?/gm, '').trim()
  return body === '' || body.startsWith('✓') || /no (page )?errors/i.test(body)
}

function apiFailures(text) {
  return text
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => /\/api\//.test(line) && /\b[45]\d\d\b/.test(line))
}

function fileFor(root, pathname) {
  let rel = decodeURIComponent(pathname).replace(/^\/+/, '')
  if (rel.includes('..')) return null
  let file = join(root, rel)
  try {
    if (statSync(file).isDirectory()) file = join(file, 'index.html')
    if (statSync(file).isFile()) return file
  } catch {
    return null
  }
  return null
}

function serve(root) {
  const types = {
    '.html': 'text/html; charset=utf-8',
    '.js': 'text/javascript; charset=utf-8',
    '.css': 'text/css; charset=utf-8',
    '.json': 'application/json',
    '.png': 'image/png',
    '.txt': 'text/plain; charset=utf-8',
    '.woff2': 'font/woff2',
    '.svg': 'image/svg+xml',
  }
  const server = createServer((req, res) => {
    const url = new URL(req.url || '/', 'http://127.0.0.1')
    const file = fileFor(root, url.pathname)
    if (file) {
      res.writeHead(200, { 'content-type': types[extname(file)] || 'application/octet-stream' })
      createReadStream(file).pipe(res)
      return
    }
    res.writeHead(404, { 'content-type': 'text/html; charset=utf-8' })
    createReadStream(join(root, '404.html')).pipe(res)
  })
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      resolve({ server, port: typeof address === 'object' && address ? address.port : 0 })
    })
  })
}

async function checkPage(path) {
  const errors = await run(['errors'], { allowFail: true })
  if (!isClean(errors)) fail(`${path} page errors:\n${errors}`)
  const api = apiFailures(await run(['network', 'requests'], { allowFail: true }))
  if (api.length) fail(`${path} called the report server:\n${api.join('\n')}`)
}

async function main() {
  if (!existsSync(browser)) {
    console.error(`agent-browser not found at ${browser}`)
    process.exit(1)
  }
  const built = spawnSync(process.execPath, [join(repo, 'scripts', 'build-play.mjs')], {
    cwd: repo,
    stdio: 'inherit',
    env: process.env,
  })
  if (built.status !== 0) process.exit(built.status || 1)

  const root = join(tmpdir(), `play-smoke-${process.pid}`)
  mkdirSync(join(root, 'agent-qa'), { recursive: true })
  symlinkSync(playDir, join(root, 'agent-qa', 'play'), 'dir')
  writePlayFallback(root)
  const index = readFileSync(join(playDir, 'index.html'), 'utf8')
  if (!index.includes('aqa-play-redirect')) fail('play index is missing the redirect restore script')
  const fallback = readFileSync(join(root, '404.html'), 'utf8')
  if (!fallback.includes('aqa-play-redirect')) fail('404.html is missing the play redirect')

  const { server, port } = await serve(root)
  const origin = `http://127.0.0.1:${port}`
  const probe = await fetch(`${origin}/agent-qa/play/`)
  const probeBody = await probe.text()
  console.log(`serving ${origin} home ${probe.status} ${probeBody.includes('aqa-play-redirect') ? 'restore-ok' : 'restore-MISSING'}`)
  const settingsProbe = await fetch(`${origin}/agent-qa/play/settings`)
  console.log(`settings ${settingsProbe.status} ${(await settingsProbe.text()).includes('aqa-play-redirect') ? 'fallback-ok' : 'fallback-MISSING'}`)
  try {
    await run(['set', 'viewport', '1280', '800'])
    for (const [path, ready] of ROUTES) {
      const url = `${origin}/agent-qa/play${path === '/' ? '/' : path}`
      await run(['console', '--clear'], { allowFail: true })
      await run(['errors', '--clear'], { allowFail: true })
      await run(['network', 'requests', '--clear'], { allowFail: true })
      await run(['open', url])
      await run(['wait', ready])
      const landed = await run(['get', 'url'])
      const expectPath = `/agent-qa/play${path === '/' ? '/' : path}`
      if (!landed.includes(expectPath)) fail(`${path} landed on ${landed}`)
      await checkPage(path)
      if (path !== '/') {
        const note = await run(['eval', 'document.querySelector("[data-testid=demo-local-only]")?.textContent||""'])
        if (!note.includes('Available when you run agent-qa locally')) fail(`${path} missing the local-only note`)
      }
    }

    await run(['open', `${origin}/agent-qa/play/settings`])
    await run(['wait', '[data-testid="demo-local-only"]'])
    await run(['reload'])
    await run(['wait', '[data-testid="demo-local-only"]'])
    const refreshed = await run(['eval', 'document.querySelector("[data-testid=demo-local-only]")?.textContent||""'])
    if (!refreshed.includes('Settings')) fail('refresh did not keep the settings page')
    if (!refreshed.includes('Available when you run agent-qa locally')) fail('refresh lost the local-only note')
    await checkPage('/settings reload')

    await run(['open', `${origin}/agent-qa/play/`])
    await run(['wait', '[data-demo-ready]'])
    await run(['errors', '--clear'], { allowFail: true })
    const before = await run(['get', 'url'])
    await run(['eval', 'document.querySelector("[data-local-only]")?.click()'])
    const after = await run(['get', 'url'])
    if (after !== before) fail(`disabled nav navigated from ${before} to ${after}`)
    await checkPage('disabled click')

    await run(['eval', 'document.querySelector(\'[aria-label="Replay scenario"]\').click()'])
    await run(['wait', '[data-testid="event-playback"]'])
    const player = '[data-testid="event-playback"]'
    const state = async () =>
      await run(['eval', `document.querySelector('${player} [aria-label="Play"], ${player} [aria-label="Pause"]')?.getAttribute('aria-label')||''`])
    if ((await state()).includes('Play')) {
      await run(['eval', `document.querySelector('${player} [aria-label="Play"]').click()`])
    }
    await run(['wait', `${player} [aria-label="Pause"]`])
    await run(['eval', `document.querySelector('${player} [aria-label="Pause"]').click()`])
    await run(['wait', `${player} [aria-label="Play"]`])
    const frame = await run(['eval', `document.querySelector('${player} img')?.getAttribute('src')||''`])
    if (!frame.includes('/fixtures/') || frame.includes('/api/')) fail(`playback frame is ${frame || 'missing'}`)
    const clock = (await run([
      'eval',
      `(() => { const el = document.querySelector('[aria-label="Event timeline"]'); const clock = el.getAttribute('aria-valuenow'); const key = clock === '0' ? 'End' : 'Home'; el.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true })); return clock; })()`,
    ])).replace(/^"|"$/g, '')
    await run(['wait', '200'])
    const afterClock = (await run(['eval', `document.querySelector('[aria-label="Event timeline"]').getAttribute('aria-valuenow')`])).replace(/^"|"$/g, '')
    if (!afterClock || afterClock === clock) fail(`scrub did not move the playhead (${clock} -> ${afterClock})`)
    await checkPage('playback')
  } catch (err) {
    fail(err instanceof Error ? err.message : String(err))
  } finally {
    await run(['close'], { allowFail: true })
    await new Promise((resolve) => server.close(resolve))
  }

  if (failures.length) {
    console.error(`\n${failures.length} smoke failure(s)`)
    process.exit(1)
  }
  console.log('play smoke ok')
}

main()
