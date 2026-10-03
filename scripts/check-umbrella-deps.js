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

const { execFileSync } = require('node:child_process');

const npmView = (args) =>
  execFileSync('npm', ['view', ...args, '--json', '--loglevel=error'], {
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe'],
    timeout: 20000,
  });

const withRetries = (fn, tries = 10) => {
  for (let i = 0; i < tries; i++) {
    try {
      return fn();
    } catch {
      execFileSync('sleep', ['15']);
    }
  }
  return null;
};

const isOwnPkg = (name) => /^@rasmusjosefsson\/agent-qa-/.test(name);

// --published <version>: post-publish verification — assert the umbrella that
// LANDED on the registry has its platform deps stamped to the tag. Used by
// release.yml after `npm publish`; a mismatch fails the workflow loudly.
// Registry reads lag writes by minutes, so this mode retries.
const pubIdx = process.argv.indexOf('--published');
if (pubIdx !== -1) {
  const version = process.argv[pubIdx + 1];
  if (!version) {
    console.error('usage: check-umbrella-deps.js --published <version>');
    process.exit(2);
  }
  const out = withRetries(() => npmView([`${pkg.name}@${version}`, 'optionalDependencies']));
  if (out === null) {
    console.error(`check-umbrella-deps: ${pkg.name}@${version} not visible on the registry — publish may have failed`);
    process.exit(1);
  }
  const deps = JSON.parse(out || '{}');
  const ownDeps = Object.entries(deps || {}).filter(([k]) => isOwnPkg(k));
  if (!ownDeps.length) {
    console.error(`check-umbrella-deps: published ${pkg.name}@${version} has no platform optionalDependencies at all`);
    process.exit(1);
  }
  const bad = ownDeps.filter(([k]) => deps[k] !== version);
  if (bad.length) {
    console.error('check-umbrella-deps: published umbrella has unstamped platform deps:');
    for (const [k, v] of bad) console.error(`  ${k}: ${v}`);
    process.exit(1);
  }
  console.log(`check-umbrella-deps: registry optionalDependencies all stamped ${version}`);
  process.exit(0);
}

const own = Object.entries(pkg.optionalDependencies || {})
  .filter(([name]) => isOwnPkg(name));

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
// umbrella second. Registry reads lag writes by minutes, so retry.
const missing = [];
for (const [name, v] of own) {
  if (String(v).startsWith('file:')) continue;
  if (withRetries(() => npmView([`${name}@${v}`, 'version'])) === null) {
    missing.push(`${name}@${v}`);
  }
}
if (missing.length) {
  console.error('check-umbrella-deps: platform packages not on the registry yet:');
  for (const m of missing) console.error(`  ${m}`);
  console.error('Publish the platform packages first (release.yml does this before the umbrella).');
  process.exit(1);
}
console.log(`check-umbrella-deps: platform deps match ${pkg.version} and exist on the registry`);
