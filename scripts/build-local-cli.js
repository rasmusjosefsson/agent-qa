#!/usr/bin/env node
/* eslint-disable no-console */
// Build the Rust CLI from `cli/` and (re)link it into the locally-installed
// platform package (`@rasmusjosefsson/agent-qa-<os>-<arch>`), so the global
// `agent-qa` launcher — and the report-server's spawned subprocesses — use the
// freshly-built binary.
//
// Why this exists: the platform packages are versioned 0.0.0 (unpublished), so
// `npm install` cannot fetch a real binary from the registry — the local dev
// binary is a symlink to `cli/target/release/agent-qa`. A clean `npm ci` wipes
// node_modules and drops the unresolved optional dep, breaking resolution. This
// script is idempotent and self-healing: run it after a reinstall to rebuild +
// recreate the package dir + symlink. Production binaries are built by
// .github/workflows/release.yml from the same `cli/` source, so committed Rust
// changes ship on the next release with no extra wiring.
//
// Usage:  npm --prefix npm/agent-qa run build:cli   (or: node scripts/build-local-cli.js)
//
// node_modules-independent alternative (survives ANY reinstall, no staging):
//   export AGENT_QA_BINARY_PATH="$PWD/cli/target/release/agent-qa"

'use strict';

const { execFileSync } = require('node:child_process');
const { platform, arch } = require('node:os');
const { join, dirname } = require('node:path');
const { existsSync, mkdirSync, rmSync, symlinkSync, writeFileSync } = require('node:fs');

const repoRoot = join(__dirname, '..');
const cliDir = join(repoRoot, 'cli');
const npmAgentQaDir = join(repoRoot, 'npm', 'agent-qa');

const PLATFORMS = {
  'darwin-arm64': { os: 'darwin', cpu: 'arm64' },
  'darwin-x64': { os: 'darwin', cpu: 'x64' },
  'linux-x64': { os: 'linux', cpu: 'x64' },
  'linux-arm64': { os: 'linux', cpu: 'arm64' },
  'win32-x64': { os: 'win32', cpu: 'x64' },
};

// Mirror bin/agent-qa.js platformPackageName() (musl omitted — dev = glibc).
function platformKey() {
  const p = platform();
  const a = arch();
  const osKey = p === 'darwin' ? 'darwin' : p === 'linux' ? 'linux' : p === 'win32' ? 'win32' : null;
  const archKey = a === 'x64' ? 'x64' : a === 'arm64' ? 'arm64' : null;
  if (!osKey || !archKey) return null;
  return { key: `${osKey}-${archKey}`, exe: p === 'win32' ? '.exe' : '' };
}

const desc = platformKey();
if (!desc || !PLATFORMS[desc.key]) {
  console.error(`unsupported host platform ${platform()}-${arch()}`);
  process.exit(2);
}

console.log(`building cli (cargo build --release) for ${desc.key} …`);
execFileSync('cargo', ['build', '--release'], { cwd: cliDir, stdio: 'inherit' });

const built = join(cliDir, 'target', 'release', `agent-qa${desc.exe}`);
if (!existsSync(built)) {
  console.error(`expected build output missing: ${built}`);
  process.exit(1);
}

// (Re)create the local platform package dir + symlink so `require.resolve`
// finds it and the bin points at the fresh build. Idempotent.
const version = require(join(npmAgentQaDir, 'package.json')).version;
const pkgName = `@rasmusjosefsson/agent-qa-${desc.key}`;
const pkgDir = join(npmAgentQaDir, 'node_modules', pkgName);
const binDir = join(pkgDir, 'bin');
mkdirSync(binDir, { recursive: true });

writeFileSync(
  join(pkgDir, 'package.json'),
  JSON.stringify(
    {
      name: pkgName,
      version,
      description: `agent-qa native binary for ${desc.key} (local dev build)`,
      os: [PLATFORMS[desc.key].os],
      cpu: [PLATFORMS[desc.key].cpu],
      files: ['bin/'],
    },
    null,
    2
  ) + '\n'
);

const dest = join(binDir, `agent-qa-${desc.key}${desc.exe}`);
rmSync(dest, { force: true }); // drop any stale symlink/file (don't write through it)
symlinkSync(built, dest); // symlink → `cargo build --release` keeps it fresh
console.log(`linked ${dest}\n    -> ${built}`);
console.log('done. `agent-qa web` now uses the freshly-built binary.');
console.log(
  `tip: to bypass node_modules entirely, export AGENT_QA_BINARY_PATH="${built}"`
);
