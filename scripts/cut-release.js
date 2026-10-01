#!/usr/bin/env node
/* eslint-disable no-console */
// Prep + cut a release: bump the umbrella version, move the
// `## [Unreleased]` changelog body under a dated `## [<v>]` heading,
// commit, tag — then (with --push) push main + the tag, which runs the
// release workflow (cross-build -> npm publish -> GitHub release notes
// extracted from that same section).
//
// Usage: node scripts/cut-release.js <patch|minor|major|x.y.z> [--push] [--dry-run] [--allow-empty]
//
// Idempotent on the prepped state: if `## [<v>]` already exists in
// CHANGELOG.md and package.json already carries <v>, only the tag +
// push happen — that is how the never-tagged first release ships.

'use strict';

const { readFileSync, writeFileSync } = require('node:fs');
const { execFileSync } = require('node:child_process');

const args = process.argv.slice(2);
const flags = new Set(args.filter((a) => a.startsWith('--')));
const positional = args.filter((a) => !a.startsWith('--'));
const bump = positional[0];
if (!bump) {
  console.error('usage: cut-release.js <patch|minor|major|x.y.z> [--push] [--dry-run] [--allow-empty]');
  process.exit(2);
}

const git = (...a) => execFileSync('git', a, { encoding: 'utf8' }).trim();
const run = (cmd, a) => {
  console.log(`+ ${cmd} ${a.join(' ')}`);
  if (!flags.has('--dry-run')) execFileSync(cmd, a, { stdio: 'inherit' });
};

// ---- preconditions --------------------------------------------------
const branch = git('rev-parse', '--abbrev-ref', 'HEAD');
if (branch !== 'main') bail(`cut releases from main (currently on ${branch})`);
if (git('status', '--porcelain')) bail('working tree is not clean');
git('fetch', 'origin', 'main', '--quiet');
const behind = git('rev-list', '--count', 'HEAD..origin/main');
if (behind !== '0') bail(`main is ${behind} commit(s) behind origin/main — pull first`);

// ---- version --------------------------------------------------------
const pkgPath = 'npm/agent-qa/package.json';
const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'));
const [maj, min, pat] = pkg.version.split('.').map(Number);
const next = /^\d+\.\d+\.\d+$/.test(bump)
  ? bump
  : { patch: `${maj}.${min}.${pat + 1}`, minor: `${maj}.${min + 1}.0`, major: `${maj + 1}.0.0` }[bump];
if (!next) bail(`unknown bump "${bump}" — expected patch|minor|major|x.y.z`);
if (git('tag', '-l', `v${next}`)) bail(`tag v${next} already exists — pick a later version`);

// ---- changelog ------------------------------------------------------
const date = new Date().toISOString().slice(0, 10);
const cl = readFileSync('CHANGELOG.md', 'utf8');
const unreleased = cl.match(/^## \[Unreleased\]\s*\n(.*?)(?=^## \[|\s*$)/ms);
if (!unreleased) bail('CHANGELOG.md has no `## [Unreleased]` heading');

const sectioned = new RegExp(`^## \\[${next.replace(/\./g, '\\.')}\\]`, 'm').test(cl);
let changed = [];

if (!sectioned) {
  if (!unreleased[1].trim() && !flags.has('--allow-empty')) {
    bail(`nothing under ## [Unreleased] — write the ${next} entries first, or pass --allow-empty`);
  }
  const stamped = cl.replace(
    /^## \[Unreleased\]\s*\n/m,
    `## [Unreleased]\n\n## [${next}] - ${date}\n`,
  );
  if (stamped === cl) bail('stamp produced no change — unexpected CHANGELOG layout');
  if (!flags.has('--dry-run')) writeFileSync('CHANGELOG.md', stamped);
  changed.push('CHANGELOG.md');
  console.log(`${flags.has('--dry-run') ? 'would stamp' : 'stamped'} CHANGELOG.md: ## [Unreleased] -> ## [${next}] - ${date}`);
} else {
  console.log(`CHANGELOG.md already carries ## [${next}] — leaving it as-is`);
}

if (pkg.version !== next) {
  pkg.version = next;
  if (!flags.has('--dry-run')) writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
  changed.push(pkgPath);
  console.log(`${flags.has('--dry-run') ? 'would bump' : 'bumped'} ${pkgPath}: ${maj}.${min}.${pat} -> ${next}`);
} else {
  console.log(`${pkgPath} already at ${next}`);
}

// ---- commit + tag ----------------------------------------------------
if (changed.length) {
  run('git', ['add', ...changed]);
  run('git', ['commit', '-m', `chore(release): v${next}`]);
}
run('git', ['tag', '-a', `v${next}`, '-m', `v${next}`]);

console.log(`\nv${next} prepped. To ship it (runs .github/workflows/release.yml):`);
console.log(`  git push origin main v${next}`);
if (flags.has('--push')) {
  run('git', ['push', 'origin', 'main', `v${next}`]);
  console.log('pushed — watch https://github.com/' + git('remote', 'get-url', 'origin').replace(/.*github\.com[/:]/, '').replace(/\.git$/, '') + '/actions/workflows/release.yml');
}

function bail(msg) {
  console.error(`cut-release: ${msg}`);
  process.exit(1);
}
