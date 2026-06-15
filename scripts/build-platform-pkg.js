#!/usr/bin/env node
/* eslint-disable no-console */
// Build a per-platform npm package directory.
//
// Usage: node scripts/build-platform-pkg.js <platform> <version>
//   <platform>  e.g. darwin-arm64, linux-x64, win32-x64
//   <version>   semver, e.g. 0.1.0
//
// Pre-condition: npm/platform/<platform>/bin/agent-qa-<platform>[.exe]
// already exists (placed there by the release workflow).
//
// Writes: npm/platform/<platform>/package.json, README.md
// Package name: `agent-qa-<platform>` (pinned by the umbrella's
// optionalDependencies).

'use strict';

const { writeFileSync, existsSync, readdirSync } = require('node:fs');
const { join } = require('node:path');

const platform = process.argv[2];
const version = process.argv[3];
if (!platform || !version) {
  console.error('usage: build-platform-pkg.js <platform> <version>');
  process.exit(2);
}

const PLATFORMS = {
  'darwin-arm64':      { os: 'darwin', cpu: 'arm64' },
  'darwin-x64':        { os: 'darwin', cpu: 'x64' },
  'linux-x64':         { os: 'linux',  cpu: 'x64', libc: 'glibc' },
  'linux-arm64':       { os: 'linux',  cpu: 'arm64', libc: 'glibc' },
  'linux-musl-x64':    { os: 'linux',  cpu: 'x64', libc: 'musl' },
  'linux-musl-arm64':  { os: 'linux',  cpu: 'arm64', libc: 'musl' },
  'win32-x64':         { os: 'win32',  cpu: 'x64' },
};

const desc = PLATFORMS[platform];
if (!desc) {
  console.error(`unknown platform: ${platform}; expected one of: ${Object.keys(PLATFORMS).join(', ')}`);
  process.exit(2);
}

const dir = join('npm', 'platform', platform);
if (!existsSync(dir)) {
  console.error(`expected directory ${dir} to exist (binary should already be staged)`);
  process.exit(2);
}

const ext = desc.os === 'win32' ? '.exe' : '';
const expectedBin = join(dir, 'bin', `agent-qa-${platform}${ext}`);
if (!existsSync(expectedBin)) {
  console.error(`missing binary at ${expectedBin}`);
  process.exit(2);
}

const pkg = {
  name: `@rasmusjosefsson/agent-qa-${platform}`,
  version,
  description: `agent-qa native binary for ${platform}`,
  license: 'MIT',
  repository: {
    type: 'git',
    url: 'git+https://github.com/rasmusjosefsson/agent-qa.git',
  },
  os: [desc.os],
  cpu: [desc.cpu],
  ...(desc.libc ? { libc: [desc.libc] } : {}),
  files: ['bin/'],
};

writeFileSync(join(dir, 'package.json'), JSON.stringify(pkg, null, 2) + '\n');
writeFileSync(
  join(dir, 'README.md'),
  `# ${pkg.name}\n\nNative binary for the [agent-qa](https://www.npmjs.com/package/agent-qa) CLI on ${platform}.\nDo not install this package directly — install \`agent-qa\` instead; npm will pick the right platform package automatically via \`optionalDependencies\`.\n`,
);

// Show what was staged
console.log(`staged ${dir}/package.json @ ${version}`);
console.log('contents:');
for (const entry of readdirSync(dir, { recursive: true })) {
  console.log(`  ${entry}`);
}
