#!/usr/bin/env node
/* eslint-disable no-console */
// agent-qa Node launcher.
//
// Does TWO things, then execs the Rust binary:
//
//   1. Resolves the platform-specific agent-qa binary out of the matching
//      optionalDependency (agent-qa-<os>-<arch>). Mirrors the esbuild /
//      Biome / Turborepo pattern.
//
//   2. Resolves the sibling `agent-browser` npm dep via require.resolve
//      and passes its absolute path to the Rust binary via the
//      AGENT_BROWSER_BIN env var. The Rust process never walks
//      node_modules — that breaks on pnpm symlinks and Yarn PnP.
//
// Env-var overrides (for local dev / CI):
//   AGENT_QA_BINARY_PATH   - bypass platform resolution, use this binary
//   AGENT_BROWSER_BIN      - bypass agent-browser resolution, use this binary

'use strict';

const { execFileSync } = require('node:child_process');
const { existsSync, chmodSync, accessSync, constants } = require('node:fs');
const { join, dirname } = require('node:path');
const { platform, arch } = require('node:os');

function detectMusl() {
  if (platform() !== 'linux') return false;
  try {
    const { execSync } = require('node:child_process');
    const out = execSync('ldd --version 2>&1 || true', { encoding: 'utf8' });
    return out.toLowerCase().includes('musl');
  } catch {
    return existsSync('/lib/ld-musl-x86_64.so.1') || existsSync('/lib/ld-musl-aarch64.so.1');
  }
}

function platformPackageName() {
  const p = platform();
  const a = arch();
  const libc = detectMusl() ? '-musl' : '';
  const osKey =
    p === 'darwin' ? 'darwin' :
    p === 'linux'  ? `linux${libc}` :
    p === 'win32'  ? 'win32' :
    null;
  const archKey = a === 'x64' ? 'x64' : a === 'arm64' ? 'arm64' : null;
  if (!osKey || !archKey) return null;
  return { pkg: `@rasmusjosefsson/agent-qa-${osKey}-${archKey}`, exe: p === 'win32' ? '.exe' : '' };
}

function resolveAgentQaBinary() {
  const override = process.env.AGENT_QA_BINARY_PATH;
  if (override) return override;

  const desc = platformPackageName();
  if (!desc) {
    throw new Error(`agent-qa: unsupported platform ${platform()}-${arch()}`);
  }
  let pkgJson;
  try {
    pkgJson = require.resolve(`${desc.pkg}/package.json`);
  } catch {
    throw new Error(
      `agent-qa: platform package "${desc.pkg}" not installed.\n` +
      `It should have been pulled in as an optionalDependency.\n` +
      `Try: npm install --force @rasmusjosefsson/agent-qa`
    );
  }
  const bin = join(dirname(pkgJson), 'bin', `agent-qa-${desc.pkg.replace(/^@rasmusjosefsson\/agent-qa-/, '')}${desc.exe}`);
  if (!existsSync(bin)) {
    throw new Error(`agent-qa: binary missing at ${bin}`);
  }
  if (platform() !== 'win32') {
    try { accessSync(bin, constants.X_OK); }
    catch { try { chmodSync(bin, 0o755); } catch (e) {
      throw new Error(`agent-qa: cannot make ${bin} executable: ${e.message}`);
    } }
  }
  return bin;
}

function resolveAgentBrowserBin() {
  const override = process.env.AGENT_BROWSER_BIN;
  if (override) return override;
  try {
    const pkgJson = require.resolve('agent-browser/package.json');
    // agent-browser's own bin-resolution logic decides the platform binary;
    // we just hand it the parent dir as a hint via the env var. agent-browser
    // ships its own launcher at <pkgdir>/bin/agent-browser.js which knows
    // the per-platform name — but the Rust binary needs the *native* binary
    // path. Re-derive it here using the same naming convention.
    const p = platform();
    const a = arch();
    const libc = detectMusl() ? '-musl' : '';
    const osKey =
      p === 'darwin' ? 'darwin' :
      p === 'linux'  ? `linux${libc}` :
      p === 'win32'  ? 'win32' :
      null;
    const archKey = a === 'x64' ? 'x64' : a === 'arm64' ? 'arm64' : null;
    const ext = p === 'win32' ? '.exe' : '';
    if (!osKey || !archKey) return null;
    const candidate = join(dirname(pkgJson), 'bin', `agent-browser-${osKey}-${archKey}${ext}`);
    return existsSync(candidate) ? candidate : null;
  } catch {
    return null; // Rust binary will error on first use with a clear message.
  }
}

try {
  const binary = resolveAgentQaBinary();
  const env = { ...process.env };
  const ab = resolveAgentBrowserBin();
  if (ab) env.AGENT_BROWSER_BIN = ab;
  execFileSync(binary, process.argv.slice(2), { stdio: 'inherit', env });
} catch (err) {
  if (err && err.status != null) process.exit(err.status);
  console.error(err.message || err);
  process.exit(1);
}
