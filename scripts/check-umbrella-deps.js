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

const bad = Object.entries(pkg.optionalDependencies || {})
  .filter(([name]) => /^@rasmusjosefsson\/agent-qa-/.test(name))
  .filter(([, v]) => v !== pkg.version && !String(v).startsWith('file:'));

if (bad.length) {
  console.error(`check-umbrella-deps: ${pkg.name}@${pkg.version} has platform deps that won't install:`);
  for (const [k, v] of bad) console.error(`  ${k}: ${v}`);
  console.error('Stamp them first: node scripts/build-umbrella-pkg.js ' + pkg.version);
  process.exit(1);
}
console.log(`check-umbrella-deps: platform deps match ${pkg.version}`);
