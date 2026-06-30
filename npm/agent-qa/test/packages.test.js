'use strict';
// Unit tests for the package manager's pure logic (source parsing, manifest
// resolution, toml merge). The fetch/exec paths (npm/git) are exercised
// manually — not in unit tests.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const pkgs = require('../lib/packages.js');

test('parseSource handles npm:, git:, https:, and bare specs', () => {
  assert.deepEqual(pkgs.parseSource('npm:@acme/agent-qa-plugin-x'), {
    scheme: 'npm',
    spec: '@acme/agent-qa-plugin-x',
    name: '@acme/agent-qa-plugin-x',
  });
  assert.equal(pkgs.parseSource('npm:@acme/x@1.2.3').name, '@acme/x'); // version stripped
  assert.equal(pkgs.parseSource('npm:foo@2').name, 'foo');
  let r = pkgs.parseSource('git:github.com/u/repo');
  assert.equal(r.scheme, 'git');
  assert.equal(r.spec, 'https://github.com/u/repo');
  assert.equal(r.name, 'repo');
  r = pkgs.parseSource('https://github.com/u/repo.git');
  assert.equal(r.scheme, 'git');
  assert.equal(r.name, 'repo');
  assert.equal(pkgs.parseSource('plain-pkg').scheme, 'npm');
  assert.throws(() => pkgs.parseSource(''));
});

test('resolveResources reads the agent-qa manifest, falls back to conventional dirs', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-pkg-'));

  // manifest-declared
  const a = path.join(root, 'a');
  fs.mkdirSync(path.join(a, 'bin'), { recursive: true });
  fs.mkdirSync(path.join(a, 'skill-data'), { recursive: true });
  fs.writeFileSync(path.join(a, 'bin', 'agent-qa-plugin-x'), '#!/bin/sh\n');
  fs.writeFileSync(
    path.join(a, 'package.json'),
    JSON.stringify({ name: 'x', 'agent-qa': { plugins: ['./bin/agent-qa-plugin-x'], skills: ['./skill-data'] } }),
  );
  let res = pkgs.resolveResources(a);
  assert.deepEqual(res.plugins, [path.join(a, 'bin', 'agent-qa-plugin-x')]);
  assert.deepEqual(res.skills, [path.join(a, 'skill-data')]);

  // conventional fallback (no manifest)
  const b = path.join(root, 'b');
  fs.mkdirSync(path.join(b, 'plugins'), { recursive: true });
  fs.mkdirSync(path.join(b, 'skills'), { recursive: true });
  fs.writeFileSync(path.join(b, 'plugins', 'agent-qa-plugin-y'), '#!/bin/sh\n');
  fs.writeFileSync(path.join(b, 'plugins', 'README.md'), 'ignored');
  fs.writeFileSync(path.join(b, 'package.json'), JSON.stringify({ name: 'y' }));
  res = pkgs.resolveResources(b);
  assert.deepEqual(res.plugins, [path.join(b, 'plugins', 'agent-qa-plugin-y')]); // only agent-qa-plugin-*
  assert.deepEqual(res.skills, [path.join(b, 'skills')]);
});

test('mergeToml preserves user content + manual dirs, overlays package resources', () => {
  const home = os.homedir();
  const pkgSkill = path.join(home, '.agent-qa', 'packages', 'npm', 'x', 'node_modules', 'x', 'skills');
  const pkgPlugin = path.join(home, '.agent-qa', 'packages', 'npm', 'x', 'node_modules', 'x', 'bin', 'agent-qa-plugin-x');

  const before = [
    '# user config',
    '[skills]',
    'extra-dirs = [',
    '  "~/.agent-qa/skills",',
    ']',
    '',
    '[other]',
    'keep = true',
    '',
  ].join('\n');

  const out = pkgs.mergeToml(before, [pkgSkill], { auth: pkgPlugin });

  assert.match(out, /\[other\]/); // unrelated table preserved
  assert.match(out, /keep = true/);
  assert.match(out, /# user config/); // comment preserved
  assert.match(out, /"~\/\.agent-qa\/skills"/); // manual dir preserved
  assert.ok(out.includes(JSON.stringify(pkgSkill))); // package skill dir added
  assert.match(out, /\[plugins\]/);
  assert.ok(out.includes(`auth = ${JSON.stringify(pkgPlugin)}`)); // plugin wired

  // re-merging with an empty registry drops the package dir but keeps the manual one
  const cleared = pkgs.mergeToml(out, [], {});
  assert.match(cleared, /"~\/\.agent-qa\/skills"/);
  assert.ok(!cleared.includes(JSON.stringify(pkgSkill)));
  assert.ok(!cleared.includes('[plugins]')); // package plugin removed
});
