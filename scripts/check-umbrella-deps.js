#!/usr/bin/env node
/* eslint-disable no-console */
// Publish guard: fail `npm publish` when the umbrella's own-scope platform
// optionalDependencies don't match the package version. optionalDependencies
// that resolve to nothing are skipped silently by npm, so a mismatch ships a
// launcher with no binary. The release workflow stamps these via
// scripts/build-umbrella-pkg.js — this check exists for any publish path that
// skips the stamp (e.g. a manual `npm publish` from a raw checkout).

'use strict';

const { readFileSync } = require('node:fs');
const { join } = require('node:path');

const pkgPath = join(__dirname, '..', 'npm', 'agent-qa', 'package.json');
const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'));

const own = Object.entries(pkg.optionalDependencies || {})
  .filter(([name]) => /^@rasmusjosefsson\/agent-qa-/.test(name));

const bad = own.filter(([, v]) => v !== pkg.version && !String(v).startsWith('file:'));
if (bad.length) {
  console.error(`check-umbrella-deps: ${pkg.name}@${pkg.version} has platform deps that won't install:`);
  for (const [k, v] of bad) console.error(`  ${k}: ${v}`);
  console.error('Stamp them first: node scripts/build-umbrella-pkg.js ' + pkg.version);
  process.exit(1);
}

// Stamped deps still fail at install time if the platform packages were never
// published (npm skips unresolvable optionalDependencies silently). Check each
// pinned version exists on the registry — publish order is platforms first,
// umbrella second. npm's CDN takes a few minutes to index a fresh publish, so
// poll instead of failing on the first 404 (the release workflow's umbrella
// job ran seconds behind the platform jobs and raced this every time).
const { execFileSync } = require('node:child_process');

function onRegistry(name, v) {
  try {
    execFileSync('npm', ['view', `${name}@${v}`, 'version', '--loglevel=error'], {
      stdio: 'pipe',
      timeout: 20000,
    });
    return true;
  } catch {
    return false;
  }
}

// Observed: a fresh provenance-signed publish took ~25 min to index, so the
// cap has to clear that. The release workflow re-runs are cheap enough that a
// long-but-bounded wait beats a spurious failure mid-release.
const deadline = Date.now() + 30 * 60 * 1000;
let missing = [];
for (;;) {
  missing = own
    .filter(([, v]) => !String(v).startsWith('file:'))
    .filter(([name, v]) => !onRegistry(name, v))
    .map(([name, v]) => `${name}@${v}`);
  if (!missing.length) break;
  if (Date.now() > deadline) {
    console.error('check-umbrella-deps: platform packages not on the registry after 30 min:');
    for (const m of missing) console.error(`  ${m}`);
    console.error('Publish the platform packages first (release.yml does this before the umbrella).');
    process.exit(1);
  }
  console.log(`check-umbrella-deps: waiting on registry index for ${missing.length} package(s)…`);
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 20000);
}
console.log(`check-umbrella-deps: platform deps match ${pkg.version} and exist on the registry`);
