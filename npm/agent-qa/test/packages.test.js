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

test('resolveResources + mergeToml carry personas + environments', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-pkg2-'));

  // manifest-declared personas/environments
  const a = path.join(root, 'a');
  fs.mkdirSync(path.join(a, 'personas'), { recursive: true });
  fs.mkdirSync(path.join(a, 'environments'), { recursive: true });
  fs.writeFileSync(
    path.join(a, 'package.json'),
    JSON.stringify({ name: 'x', 'agent-qa': { personas: ['./personas'], environments: ['./environments'] } }),
  );
  let res = pkgs.resolveResources(a);
  assert.deepEqual(res.personas, [path.join(a, 'personas')]);
  assert.deepEqual(res.environments, [path.join(a, 'environments')]);

  // conventional fallback (no manifest)
  const b = path.join(root, 'b');
  fs.mkdirSync(path.join(b, 'personas'), { recursive: true });
  fs.mkdirSync(path.join(b, 'environments'), { recursive: true });
  fs.writeFileSync(path.join(b, 'package.json'), JSON.stringify({ name: 'y' }));
  res = pkgs.resolveResources(b);
  assert.deepEqual(res.personas, [path.join(b, 'personas')]);
  assert.deepEqual(res.environments, [path.join(b, 'environments')]);

  // mergeToml emits [personas]/[environments] extra-dirs and clears them
  const home = os.homedir();
  const pDir = path.join(home, '.agent-qa', 'packages', 'git', 'p', 'personas');
  const eDir = path.join(home, '.agent-qa', 'packages', 'git', 'p', 'environments');
  const out = pkgs.mergeToml('', [], {}, [pDir], [eDir]);
  assert.match(out, /\[personas\]/);
  assert.match(out, /\[environments\]/);
  assert.ok(out.includes(JSON.stringify(pDir)));
  assert.ok(out.includes(JSON.stringify(eDir)));
  const cleared = pkgs.mergeToml(out, [], {}, [], []);
  assert.ok(!cleared.includes('[personas]'));
  assert.ok(!cleared.includes(JSON.stringify(pDir)));
});

test('remove uninstalls a package: registry entry gone, files deleted, toml rewired', () => {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-home-'));
  const prev = process.env.AGENT_QA_HOME;
  process.env.AGENT_QA_HOME = home;
  try {
    // Seed a fetched git package under <packages>/git/<name> + a registry entry.
    const pkgBase = path.join(pkgs._packagesDir(), 'git', 'demo-ext');
    const pluginPath = path.join(pkgBase, 'bin', 'agent-qa-plugin-demo');
    fs.mkdirSync(path.join(pkgBase, 'skills'), { recursive: true });
    fs.mkdirSync(path.dirname(pluginPath), { recursive: true });
    fs.writeFileSync(pluginPath, '#!/bin/sh\n');
    const reg = {
      schema: 'packages/1',
      packages: [
        {
          source: 'git:github.com/u/demo-ext',
          scheme: 'git',
          name: 'demo-ext',
          dir: pkgBase,
          plugins: [{ path: pluginPath, kinds: ['auth'] }],
          skills: [path.join(pkgBase, 'skills')],
          personas: [],
          environments: [],
        },
      ],
    };
    fs.writeFileSync(path.join(pkgs._packagesDir(), 'registry.json'), JSON.stringify(reg));
    fs.writeFileSync(pkgs._tomlFile(), '# user\n');

    const n = pkgs.remove('git:github.com/u/demo-ext');
    assert.equal(n, 1); // one package removed

    const after = JSON.parse(fs.readFileSync(path.join(pkgs._packagesDir(), 'registry.json'), 'utf8'));
    assert.equal(after.packages.length, 0); // registry entry gone
    assert.ok(!fs.existsSync(pkgBase)); // fetched files deleted
    assert.ok(!fs.readFileSync(pkgs._tomlFile(), 'utf8').includes('agent-qa-plugin-demo')); // toml rewired

    assert.equal(pkgs.remove('git:github.com/u/nope'), 0); // no match → 0
  } finally {
    if (prev === undefined) delete process.env.AGENT_QA_HOME;
    else process.env.AGENT_QA_HOME = prev;
    fs.rmSync(home, { recursive: true, force: true });
  }
});
