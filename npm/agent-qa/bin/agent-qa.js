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

// `web` (formerly `report view`) is a localhost-only run viewer + authoring
// editor + chat, hosted by this Node launcher (no new Rust crate). The viewer
// is a pure consumer of the per-run sidecar tree; the editor/chat tabs shell
// the CLI. Every OTHER verb passes straight through to the Rust binary below.
function parseWebArgs(argv) {
  const opts = { port: 7878, open: true, root: undefined };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--port' || a === '-p') {
      opts.port = parseInt(argv[++i], 10);
    } else if (a.startsWith('--port=')) {
      opts.port = parseInt(a.slice('--port='.length), 10);
    } else if (a === '--root') {
      opts.root = argv[++i];
    } else if (a.startsWith('--root=')) {
      opts.root = a.slice('--root='.length);
    } else if (a === '--no-open') {
      opts.open = false;
    } else if (a === '-h' || a === '--help' || a === 'help') {
      opts.help = true;
    } else {
      throw new Error(`agent-qa web: unknown argument ${JSON.stringify(a)}`);
    }
  }
  if (!Number.isInteger(opts.port) || opts.port <= 0 || opts.port > 65535) {
    throw new Error('agent-qa web: --port expects an integer in 1..65535');
  }
  return opts;
}

function maybeRunWeb(argv) {
  let rest;
  if (argv[0] === 'web') {
    rest = argv.slice(1);
  } else if (argv[0] === 'report' && argv[1] === 'view') {
    // Deprecated alias kept for back-compat with earlier releases / scripts.
    console.error('agent-qa: `report view` is deprecated — use `agent-qa web`.');
    rest = argv.slice(2);
  } else {
    return false;
  }
  const opts = parseWebArgs(rest);
  if (opts.help) {
    console.log(
      'agent-qa web — localhost run viewer + authoring editor + chat\n\n' +
        'Usage:\n' +
        '  agent-qa web [--port <N>] [--root <dir>] [--no-open]\n\n' +
        'Flags:\n' +
        '  --port <N>    Port to bind on 127.0.0.1 (default 7878)\n' +
        '  --root <dir>  Scenarios root to view (default: AGENT_QA_SCENARIOS_DIR,\n' +
        '                agent-qa.toml [paths].scenarios_root, or\n' +
        '                <cwd>/tmp/agent-qa-scenarios)\n' +
        '  --no-open     Do not auto-open the browser\n\n' +
        'Alias: `agent-qa report view` (deprecated).',
    );
    return true;
  }
  // eslint-disable-next-line global-require
  const { start } = require('../lib/report-server.js');
  // Resolve the Rust binary + agent-browser so the editor's write surface
  // can shell the CLI. Non-fatal if the platform package is missing — the
  // viewer still works; only the editor tab is disabled.
  let agentQaBin;
  try {
    agentQaBin = resolveAgentQaBinary();
  } catch {
    agentQaBin = undefined;
  }
  const agentBrowserBin = resolveAgentBrowserBin() || undefined;
  start({ ...opts, agentQaBin, agentBrowserBin });
  return true;
}

// Answer `--version` / `-v` from the launcher so it works even when the
// platform binary can't be resolved. On a published install the umbrella
// package.json carries the real version (e.g. 0.0.17); in the source tree it
// is 0.0.0, so we fall back to the binary's own (truthful) version string.
function maybeRunVersion(argv) {
  if (!argv.length || !['-v', '-V', '--version', 'version'].includes(argv[0])) {
    return false;
  }
  const wantJson = argv.includes('--json');

  let pkgV = null;
  try { pkgV = require('../package.json').version; } catch { /* ignore */ }

  let cliV = null;
  try {
    const line = execFileSync(resolveAgentQaBinary(), ['--version'], { encoding: 'utf8' }).trim();
    cliV = line.replace(/^agent-qa\s+/, '') || null; // "agent-qa 0.0.1+gSHA" -> "0.0.1+gSHA"
  } catch { /* binary unresolved — fall back to pkg version */ }

  const version = pkgV && pkgV !== '0.0.0' ? pkgV : (cliV || pkgV || 'unknown');
  if (wantJson) {
    console.log(JSON.stringify({ name: 'agent-qa', version, cli: cliV }));
  } else {
    console.log(`agent-qa ${version}${cliV && cliV !== version ? ` (cli ${cliV})` : ''}`);
  }
  return true;
}

try {
  if (maybeRunVersion(process.argv.slice(2))) {
    return;
  }
  if (maybeRunWeb(process.argv.slice(2))) {
    return;
  }
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
