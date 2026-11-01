'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const provider = path.resolve(__dirname, '..', '..', '..', 'examples', 'surfaces', 'desktop', 'provider.js');
const { flatten, actArgs } = require(provider);

test('flatten keeps role, name, and states', () => {
  const lines = [];
  const elements = [];
  flatten(
    {
      role: 'window',
      name: 'Untitled',
      children: [
        { role: 'button', name: 'Save', ref_id: '@s1:e2', states: ['enabled', 'focused'] },
      ],
    },
    lines,
    elements,
  );
  assert.match(lines.join('\n'), /button "Save"/);
  assert.equal(elements[1].role, 'button');
  assert.equal(elements[1].name, 'Save');
  assert.equal(elements[1].focused, true);
  assert.equal(elements[1].enabled, true);
});

test('type maps to set-value and dblclick is headed', () => {
  assert.deepEqual(actArgs('type', '@s:e1', 'Ada'), ['set-value', '@s:e1', 'Ada']);
  assert.deepEqual(actArgs('dblclick', '@s:e1', null), ['--headed', 'double-click', '@s:e1']);
  assert.equal(actArgs('goto', '@s:e1', null), null);
});

function fakeDesktop(dir) {
  const log = path.join(dir, 'desktop.log');
  const bin = path.join(dir, 'agent-desktop');
  fs.writeFileSync(
    bin,
    `#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2);
fs.appendFileSync(process.env.DESKTOP_LOG, JSON.stringify(args) + '\\n');
const cmd = args.find((a) => !a.startsWith('--'));
if (cmd === 'snapshot') {
  process.stdout.write(JSON.stringify({
    ok: true,
    data: {
      window: { title: 'Untitled' },
      tree: {
        role: 'window', name: 'Untitled',
        children: [{ role: 'button', name: 'Save', ref_id: '@s:e1', states: ['enabled'] }],
      },
    },
  }));
  process.exit(0);
}
if (cmd === 'find') {
  process.stdout.write(JSON.stringify({
    ok: true,
    data: { matches: [{ role: 'button', name: 'Save', ref_id: '@s:e1' }] },
  }));
  process.exit(0);
}
if (cmd === 'screenshot') {
  fs.writeFileSync(args[args.length - 1], Buffer.from('png'));
  process.stdout.write(JSON.stringify({ ok: true, data: {} }));
  process.exit(0);
}
process.stdout.write(JSON.stringify({ ok: true, data: {} }));
`,
  );
  fs.chmodSync(bin, 0o755);
  fs.writeFileSync(log, '');
  return { bin, log };
}

function call(op, request, desktop) {
  const r = spawnSync(process.execPath, [provider, op], {
    input: JSON.stringify({ protocolVersion: 1, request }),
    encoding: 'utf8',
    env: {
      ...process.env,
      AGENT_DESKTOP_BIN: desktop.bin,
      DESKTOP_LOG: desktop.log,
    },
  });
  assert.equal(r.status, 0, r.stderr);
  return { body: JSON.parse(r.stdout), log: fs.readFileSync(desktop.log, 'utf8') };
}

test('ping does not launch the desktop driver', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'aq-surface-'));
  const desktop = fakeDesktop(dir);
  const { body, log } = call('ping', {}, desktop);
  assert.equal(body.ok, true);
  assert.deepEqual(body.response.surfaces, ['desktop']);
  assert.equal(log, '');
});

test('click resolves a ref then observes', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'aq-surface-'));
  const desktop = fakeDesktop(dir);
  const { body, log } = call(
    'act',
    {
      surface: 'desktop',
      target: { app: 'TextEdit' },
      verb: 'click',
      on: { role: 'button', name: 'Save' },
    },
    desktop,
  );
  assert.equal(body.ok, true);
  assert.match(body.response.snapshot, /button "Save"/);
  assert.equal(body.response.elements.some((el) => el.name === 'Save'), true);
  assert.match(body.response.screenshotPng, /^[A-Za-z0-9+/=]+$/);
  const lines = log.trim().split('\n').map((l) => JSON.parse(l));
  assert.deepEqual(lines[0].slice(0, 3), ['find', '--app', 'TextEdit']);
  assert.ok(lines.some((args) => args[0] === 'click' && args[1] === '@s:e1'));
  assert.ok(lines.some((args) => args[0] === 'snapshot'));
});
