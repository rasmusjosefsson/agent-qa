'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const launcher = path.join(__dirname, '..', 'bin', 'agent-qa.js');

test('update checks npm and updates the CLI by default', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'agent-qa-update-'));
  const log = path.join(dir, 'npm.log');
  const fakeNpm = path.join(dir, 'npm');
  fs.writeFileSync(
    fakeNpm,
    `#!/usr/bin/env node
const fs = require('node:fs');
fs.appendFileSync(process.env.NPM_LOG, JSON.stringify(process.argv.slice(2)) + '\\n');
if (process.argv[2] === 'view') console.log('9.9.9');
`,
  );
  fs.chmodSync(fakeNpm, 0o755);
  fs.writeFileSync(path.join(dir, 'npm.cmd'), '@node "%~dp0\\npm" %*\r\n');

  const result = spawnSync(process.execPath, [launcher, 'update'], {
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${dir}${path.delimiter}${process.env.PATH || ''}`,
      NPM_LOG: log,
      AGENT_QA_HOME: path.join(dir, 'home'),
    },
  });

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Updated agent-qa from 0\.0\.0 to 9\.9\.9/);
  const calls = fs.readFileSync(log, 'utf8').trim().split('\n').map(JSON.parse);
  assert.deepEqual(calls, [
    ['view', '@rasmusjosefsson/agent-qa', 'version', '--loglevel=error'],
    ['install', '-g', '@rasmusjosefsson/agent-qa@9.9.9', '--no-audit', '--no-fund', '--loglevel=error'],
  ]);
});
