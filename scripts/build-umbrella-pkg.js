#!/usr/bin/env node
/* eslint-disable no-console */
// Stamp the umbrella `agent-qa` npm package with a release version.
//
// Usage: node scripts/build-umbrella-pkg.js <version> [--local-platforms <a,b,c>]
//
// Reads npm/agent-qa/package.json, sets `version` and aligns every
// optionalDependency on the same version.
//
// `--local-platforms` mode (used in CI smoke tests): rewrites the listed
// optionalDependencies to point at file: paths so we can install from local
// tarballs without going through the npm registry.

'use strict';

const { readFileSync, writeFileSync, cpSync, rmSync, existsSync } = require('node:fs');
const { join } = require('node:path');

const args = process.argv.slice(2);
const version = args[0];
if (!version) {
  console.error('usage: build-umbrella-pkg.js <version> [--local-platforms <a,b,c>]');
  process.exit(2);
}
let localPlatforms = null;
for (let i = 1; i < args.length; i++) {
  if (args[i] === '--local-platforms') localPlatforms = (args[++i] || '').split(',').filter(Boolean);
}

const pkgPath = join('npm', 'agent-qa', 'package.json');
const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'));

pkg.version = version;

// Only OUR per-platform packages track the release version. Third-party
// optionalDependencies (e.g. @earendil-works/pi-coding-agent) keep their
// declared version — stamping them to the tag pins a version that doesn't
// exist, which 404s and makes npm skip the whole optional group (the platform
// binary AND the chat agent silently fail to install). Match by our scope +
// the `agent-qa-<platform>` prefix.
const opt = pkg.optionalDependencies || {};
const isOwnPlatformPkg = (name) => /^@rasmusjosefsson\/agent-qa-/.test(name);
for (const name of Object.keys(opt)) {
  if (isOwnPlatformPkg(name)) opt[name] = version;
}
if (localPlatforms) {
  for (const plat of localPlatforms) {
    const key = `@rasmusjosefsson/agent-qa-${plat}`;
    opt[key] = `file:../platform/${plat}`;
  }
}
pkg.optionalDependencies = opt;

writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
console.log(`stamped ${pkgPath} @ ${version}`);
console.log('optionalDependencies:');
for (const [k, v] of Object.entries(pkg.optionalDependencies || {})) console.log(`  ${k}: ${v}`);

// Mirror the repo-root `skills/` tree into the umbrella package so it ships
// inside the npm tarball. The source of truth lives at the repo root (same
// pattern as `skill-data/` for the embedded runbooks).
const skillsSrc = 'skills';
const skillsDst = join('npm', 'agent-qa', 'skills');
if (existsSync(skillsSrc)) {
  if (existsSync(skillsDst)) rmSync(skillsDst, { recursive: true, force: true });
  cpSync(skillsSrc, skillsDst, { recursive: true });
  console.log(`copied ${skillsSrc}/ -> ${skillsDst}/`);
}
