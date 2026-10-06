#!/usr/bin/env node
/* eslint-disable no-console */
// Dev loop for the workbench: `cargo build` the debug CLI, then launch
// `agent-qa web` with AGENT_QA_BINARY_PATH pointed at it — every verb the
// UI shells out (start, record-step, flush, replay, validate) runs your
// working tree instead of the released binary.
//
// The workbench shells the CLI per action, so iterating is just
// `cargo build` in cli/ — the next UI click picks up the new binary.
// No workbench restart needed.
//
// Usage:  npm --prefix npm/agent-qa run dev -- [agent-qa web args]
//         node scripts/dev-web.js --port 7878 --root <dir>

'use strict';

const { execFileSync, spawn } = require('node:child_process');
const { existsSync } = require('node:fs');
const { join } = require('node:path');

const repoRoot = join(__dirname, '..');
const cliDir = join(repoRoot, 'cli');
const exe = process.platform === 'win32' ? 'agent-qa.exe' : 'agent-qa';
const bin = join(cliDir, 'target', 'debug', exe);

const env = { ...process.env };
// Linking fails on hosts where the Xcode license was never accepted;
// Command Line Tools is a valid developer dir and needs no license.
if (!env.DEVELOPER_DIR && existsSync('/Library/Developer/CommandLineTools/usr/bin')) {
  env.DEVELOPER_DIR = '/Library/Developer/CommandLineTools';
}

console.log('[dev] cargo build (debug)…');
execFileSync('cargo', ['build'], { cwd: cliDir, stdio: 'inherit', env });
if (!existsSync(bin)) {
  console.error(`[dev] expected build output missing: ${bin}`);
  process.exit(1);
}

console.log(`[dev] AGENT_QA_BINARY_PATH=${bin}`);
const launcher = join(repoRoot, 'npm', 'agent-qa', 'bin', 'agent-qa.js');
const child = spawn(process.execPath, [launcher, 'web', ...process.argv.slice(2)], {
  stdio: 'inherit',
  env: { ...env, AGENT_QA_BINARY_PATH: bin },
});
child.on('exit', (code) => process.exit(code ?? 0));
child.on('error', (e) => {
  console.error(`[dev] launcher failed: ${e.message}`);
  process.exit(1);
});
