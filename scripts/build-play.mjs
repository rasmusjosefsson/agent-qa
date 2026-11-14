#!/usr/bin/env node
// Build the read-only Runs demo and stage it for the docs site.
//
//   node scripts/build-play.mjs
//
// Output is site/public/play/ (gitignored). `npm --prefix site run build`
// runs this before Astro, and GitHub Pages serves it at /agent-qa/play/.
// The local workbench build is unchanged: it still uses the http source.

import { spawnSync } from 'node:child_process'
import { cpSync, mkdirSync, rmSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const repo = join(dirname(fileURLToPath(import.meta.url)), '..')
const web = join(repo, 'npm', 'agent-qa', 'web')
const out = join(repo, 'site', 'public', 'play')
const fixtures = join(web, 'fixtures')

function run(cmd, args, cwd, env) {
  const result = spawnSync(cmd, args, { cwd, env, stdio: 'inherit' })
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

rmSync(out, { recursive: true, force: true })
mkdirSync(out, { recursive: true })

const env = {
  ...process.env,
  VITE_AQA_SOURCE: 'fixture',
  VITE_AQA_FIXTURE_BASE: '/agent-qa/play/fixtures',
}

run('npx', ['vite', 'build', '--base', '/agent-qa/play/', '--outDir', out], web, env)
cpSync(fixtures, join(out, 'fixtures'), { recursive: true })
console.log(`play build → ${out}`)
