#!/usr/bin/env node
// Desktop surface provider. Speaks the agent-qa surface protocol on
// stdio and shells the agent-desktop CLI for the accessibility tree.
// Core never imports this file. See docs/surfaces.md.
//
//   AGENT_DESKTOP_BIN          desktop CLI (default: agent-desktop)
//   AGENT_QA_SURFACE_HEADED=1  physical input for every command

'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const PROTOCOL = 1;

function bin() {
  return process.env.AGENT_DESKTOP_BIN || 'agent-desktop';
}

function respond(body) {
  process.stdout.write(JSON.stringify(body));
  process.exit(0);
}

function fail(code, message) {
  respond({ ok: false, error: { code, message } });
}

function runDesktop(args) {
  const headed = process.env.AGENT_QA_SURFACE_HEADED === '1';
  const full = headed && args[0] !== '--headed' ? ['--headed', ...args] : args;
  const r = spawnSync(bin(), full, {
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  });
  if (r.error) {
    return {
      ok: false,
      error: { code: 'driver', message: `${bin()}: ${r.error.message}` },
    };
  }
  const text = (r.stdout || '').trim();
  if (!text) {
    const err = (r.stderr || '').trim().slice(0, 500) || 'desktop driver produced no JSON';
    return { ok: false, error: { code: 'driver', message: err } };
  }
  try {
    return JSON.parse(text);
  } catch {
    return {
      ok: false,
      error: { code: 'driver', message: text.slice(0, 500) },
    };
  }
}

function driverError(res) {
  const code = (res.error && res.error.code) || 'driver';
  const message = (res.error && res.error.message) || 'desktop driver failed';
  fail(code, message);
}

function appOf(request) {
  const app = request.target && request.target.app;
  if (!app) fail('invalid-args', 'target.app is required');
  return app;
}

function flatten(node, lines, elements) {
  if (!node || typeof node !== 'object') return;
  const role = node.role || 'group';
  const name = typeof node.name === 'string' ? node.name : '';
  const value = node.value == null ? null : String(node.value);
  const states = Array.isArray(node.states) ? node.states : [];
  if (node.ref_id || name) {
    const flags = [];
    if (states.includes('focused')) flags.push('focused');
    if (states.includes('disabled')) flags.push('disabled');
    if (states.includes('checked')) flags.push('checked');
    const flag = flags.length ? ` [${flags.join(', ')}]` : '';
    const label = name ? ` "${name}"` : '';
    lines.push(`- ${role}${label}${flag}`);
    elements.push({
      role,
      name: name || null,
      value,
      text: name || value || '',
      focused: states.includes('focused'),
      enabled: !states.includes('disabled'),
      checked: states.includes('checked'),
    });
  }
  for (const child of node.children || []) flatten(child, lines, elements);
}

function observe(request) {
  const app = appOf(request);
  const snap = runDesktop(['snapshot', '--app', app, '-i', '--compact']);
  if (!snap.ok) driverError(snap);
  const lines = [];
  const elements = [];
  if (snap.data && snap.data.tree) flatten(snap.data.tree, lines, elements);
  const snapshot = lines.length ? `${lines.join('\n')}\n` : '';
  let screenshotPng = null;
  const tmp = path.join(os.tmpdir(), `aq-surface-${process.pid}-${Date.now()}.png`);
  try {
    const shot = runDesktop(['screenshot', '--app', app, tmp]);
    if (shot.ok && fs.existsSync(tmp)) {
      screenshotPng = fs.readFileSync(tmp).toString('base64');
    } else if (!shot.ok) {
      const message = (shot.error && shot.error.message) || 'screenshot failed';
      process.stderr.write(`screenshot skipped: ${message}\n`);
    }
  } finally {
    fs.rmSync(tmp, { force: true });
  }
  const response = { snapshot, elements };
  if (screenshotPng) response.screenshotPng = screenshotPng;
  const title = snap.data && snap.data.window && snap.data.window.title;
  if (title) response.window = title;
  return response;
}

function findRef(app, on) {
  const args = ['find', '--app', app, '--role', on.role, '--exact', '--limit', '2'];
  if (on.name) args.push('--name', on.name);
  const res = runDesktop(args);
  if (!res.ok) driverError(res);
  const matches = (res.data && res.data.matches) || [];
  const withRef = matches.filter((m) => m.ref_id);
  if (withRef.length === 0) {
    fail('element-not-found', `no ${on.role}${on.name ? ` "${on.name}"` : ''} in ${app}`);
  }
  if (withRef.length > 1) {
    fail('ambiguous', `${withRef.length} ${on.role} elements named ${on.name || '(any)'}`);
  }
  return withRef[0].ref_id;
}

function actArgs(verb, ref, value) {
  switch (verb) {
    case 'click':
      return ['click', ref];
    case 'dblclick':
      return ['--headed', 'double-click', ref];
    case 'rightclick':
      return ['right-click', ref];
    case 'type':
      return ['set-value', ref, value ?? ''];
    case 'clear':
      return ['clear', ref];
    case 'hover':
      return ['--headed', 'hover', ref];
    case 'select':
      return ['select', ref, value ?? ''];
    case 'check':
      return ['check', ref];
    case 'uncheck':
      return ['uncheck', ref];
    case 'scrollTo':
      return ['scroll-to', ref];
    case 'focus':
      return ['focus', ref];
    default:
      return null;
  }
}

function act(request) {
  const app = appOf(request);
  const verb = request.verb;
  const params = request.params || {};
  if (verb === 'wait') {
    const args = typeof params.ms === 'number' ? ['wait', String(params.ms)] : ['wait', '0'];
    const res = runDesktop(args);
    if (!res.ok) driverError(res);
    return observe(request);
  }
  if (verb === 'press') {
    const chord = request.value || '';
    if (!chord) fail('invalid-args', 'press needs a value');
    const res = runDesktop(['press', chord, '--app', app]);
    if (!res.ok) driverError(res);
    return observe(request);
  }
  if (verb === 'read') return observe(request);
  if (!request.on || !request.on.role) fail('invalid-args', `${verb} needs on.role`);
  const ref = findRef(app, request.on);
  const args = actArgs(verb, ref, request.value);
  if (!args) fail('unsupported-verb', `verb ${verb} is not a desktop action`);
  const res = runDesktop(args);
  if (!res.ok) driverError(res);
  return observe(request);
}

function open(request) {
  const app = appOf(request);
  const launched = runDesktop(['launch', app]);
  if (!launched.ok) driverError(launched);
  const window = request.target && request.target.window;
  if (window) {
    const waited = runDesktop(['wait', '--window', window, '--app', app]);
    if (!waited.ok) driverError(waited);
  }
  return { ready: true };
}

function close(request) {
  const app = appOf(request);
  const res = runDesktop(['close-app', app]);
  if (!res.ok && res.error && res.error.code !== 'APP_NOT_FOUND') driverError(res);
  return {};
}

function main() {
  const op = process.argv[2];
  let request = {};
  const stdin = fs.readFileSync(0, 'utf8');
  if (stdin.trim()) {
    try {
      const msg = JSON.parse(stdin);
      request = msg.request || {};
    } catch (e) {
      fail('bad-json', e.message);
    }
  }
  if (op === 'ping') {
    respond({
      ok: true,
      response: { protocolVersion: PROTOCOL, name: 'desktop', surfaces: ['desktop'] },
    });
  }
  try {
    let response;
    if (op === 'open') response = open(request);
    else if (op === 'act') response = act(request);
    else if (op === 'observe') response = observe(request);
    else if (op === 'close') response = close(request);
    else fail('unsupported-op', `unknown op ${op}`);
    respond({ ok: true, response });
  } catch (e) {
    fail('driver', e.message || String(e));
  }
}

if (require.main === module) main();

module.exports = {
  flatten,
  actArgs,
  main,
};
