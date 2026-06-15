#!/usr/bin/env node
/* eslint-disable no-console */
// Best-effort postinstall: ensure the resolved platform binary is executable.
//
// Some package managers (notably bun, certain sandboxed installs) strip the
// exec bit on extracted files. The launcher will also chmod on first run,
// but doing it at install time gives a cleaner first-run experience.
//
// Failure is non-fatal — we never want a postinstall to break `npm i`.

'use strict';

try {
  const { existsSync, chmodSync } = require('node:fs');
  const { join, dirname } = require('node:path');
  const { platform, arch } = require('node:os');

  const p = platform();
  if (p === 'win32') return;

  const a = arch();

  const osKey =
    p === 'darwin' ? 'darwin' :
    p === 'linux'  ? 'linux'  :
    null;
  const archKey = a === 'x64' ? 'x64' : a === 'arm64' ? 'arm64' : null;
  if (!osKey || !archKey) return;

  const pkgName = `@rasmusjosefsson/agent-qa-${osKey}-${archKey}`;
  let pkgJson;
  try { pkgJson = require.resolve(`${pkgName}/package.json`); } catch { return; }
  const bin = join(dirname(pkgJson), 'bin', `agent-qa-${osKey}-${archKey}`);
  if (existsSync(bin)) {
    try { chmodSync(bin, 0o755); } catch { /* ignore */ }
  }
} catch {
  // never throw from postinstall
}
