'use strict';
// agent-qa package manager — `agent-qa install <source>` (pi-style).
//
// Fetches an extension package (npm: / git: / https:) into ~/.agent-qa/packages,
// reads its `agent-qa` manifest (or conventional plugins/ + skills/ dirs), and
// wires the discovered plugins + skill dirs into ~/.agent-qa/agent-qa.toml so
// agent-qa (and the workbench) auto-pick them up. No vendor logic here — the
// package supplies it.
//
//   "agent-qa": { "plugins": ["./bin/agent-qa-plugin-x"], "skills": ["./skills"] }
//
// Pure helpers (parseSource, resolveResources, mergeToml) are exported for
// tests; the fetch/exec paths are exercised end-to-end manually.

const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { execFileSync } = require('node:child_process');

const configDir = () => path.join(os.homedir(), '.agent-qa');
const packagesDir = () => path.join(configDir(), 'packages');
const registryFile = () => path.join(packagesDir(), 'registry.json');
const tomlFile = () => path.join(configDir(), 'agent-qa.toml');

// ---- source parsing ----

function npmName(spec) {
  // @scope/pkg@1.2.3 → @scope/pkg ; pkg@1 → pkg
  if (spec.startsWith('@')) {
    const slash = spec.indexOf('/');
    const at = spec.indexOf('@', slash + 1);
    return at > 0 ? spec.slice(0, at) : spec;
  }
  const at = spec.indexOf('@');
  return at > 0 ? spec.slice(0, at) : spec;
}

function gitName(url) {
  return (
    url
      .replace(/\.git$/, '')
      .replace(/[#].*$/, '')
      .split('/')
      .filter(Boolean)
      .pop() || 'package'
  );
}

// Returns { scheme: 'npm'|'git', spec, name }.
function parseSource(input) {
  const s = String(input || '').trim();
  if (!s) throw new Error('usage: agent-qa install <npm:<pkg> | git:<url> | https://…>');
  if (s.startsWith('npm:')) return { scheme: 'npm', spec: s.slice(4), name: npmName(s.slice(4)) };
  if (s.startsWith('git:')) {
    const url = 'https://' + s.slice(4).replace(/^\/+/, '');
    return { scheme: 'git', spec: url, name: gitName(url) };
  }
  if (/^(https?:\/\/|ssh:\/\/|git@)/.test(s)) return { scheme: 'git', spec: s, name: gitName(s) };
  return { scheme: 'npm', spec: s, name: npmName(s) };
}

const safeDir = (n) => String(n).replace(/[^A-Za-z0-9._-]+/g, '_');

// ---- manifest / resource resolution ----

// Resolve a package dir → { plugins: [absPaths], skills: [absDirs] } from its
// `agent-qa` manifest, falling back to conventional plugins/ + skills/ dirs.
function resolveResources(pkgDir) {
  let manifest = {};
  try {
    manifest = JSON.parse(fs.readFileSync(path.join(pkgDir, 'package.json'), 'utf8'))['agent-qa'] || {};
  } catch {
    /* no/!json package.json */
  }
  const plugins = [];
  const skills = [];
  const absList = (rel, into) => {
    for (const r of Array.isArray(rel) ? rel : []) {
      const abs = path.resolve(pkgDir, r);
      if (fs.existsSync(abs)) into.push(abs);
    }
  };
  absList(manifest.plugins, plugins);
  absList(manifest.skills, skills);
  // conventional fallbacks
  if (!plugins.length) {
    const d = path.join(pkgDir, 'plugins');
    if (fs.existsSync(d)) {
      for (const f of fs.readdirSync(d)) {
        if (f.startsWith('agent-qa-plugin-')) plugins.push(path.join(d, f));
      }
    }
  }
  if (!skills.length) {
    const d = path.join(pkgDir, 'skills');
    if (fs.existsSync(d)) skills.push(d);
  }
  return { plugins, skills };
}

function pingKinds(binPath) {
  try {
    fs.chmodSync(binPath, 0o755);
  } catch {
    /* best effort */
  }
  try {
    const out = execFileSync(binPath, ['ping'], { encoding: 'utf8', timeout: 10000 });
    const j = JSON.parse(out);
    return (j.response && Array.isArray(j.response.kinds) && j.response.kinds) || [];
  } catch {
    return [];
  }
}

// ---- minimal toml management (only [skills].extra-dirs + [plugins]) ----

const expandHome = (p) => (p.startsWith('~') ? path.join(os.homedir(), p.slice(1)) : p);

// Split toml text into { rest, extraDirs, plugins } — rest is everything except
// the [skills] and [plugins] tables (preserved verbatim, so other config /
// comments survive); extraDirs + plugins are parsed from those tables.
function splitToml(text) {
  const lines = String(text || '').split('\n');
  const rest = [];
  const skillsBlock = [];
  const pluginsBlock = [];
  let cur = null; // null | 'skills' | 'plugins' | 'other'
  for (const line of lines) {
    const m = /^\s*\[([^\]]+)\]\s*$/.exec(line);
    if (m) {
      const t = m[1].trim();
      cur = t === 'skills' ? 'skills' : t === 'plugins' ? 'plugins' : 'other';
      if (cur === 'other') rest.push(line);
      continue;
    }
    if (cur === 'skills') skillsBlock.push(line);
    else if (cur === 'plugins') pluginsBlock.push(line);
    else rest.push(line);
  }
  const extraDirs = [];
  for (const q of skillsBlock.join('\n').matchAll(/"([^"]+)"/g)) extraDirs.push(q[1]);
  const plugins = {};
  for (const l of pluginsBlock) {
    const pm = /^\s*([A-Za-z0-9_-]+)\s*=\s*"([^"]+)"/.exec(l);
    if (pm) plugins[pm[1]] = pm[2];
  }
  return { rest: rest.join('\n'), extraDirs, plugins };
}

const uniq = (a) => [...new Set(a)];

// Rebuild toml: preserve non-package extra-dirs/plugins (user's manual ones),
// overlay the package-derived ones (everything under ~/.agent-qa/packages).
function mergeToml(text, regSkills, regPlugins) {
  const { rest, extraDirs, plugins } = splitToml(text);
  const pkgRoot = packagesDir();
  const isManaged = (p) => expandHome(p).startsWith(pkgRoot);
  const userDirs = extraDirs.filter((d) => !isManaged(d));
  const dirs = uniq([...userDirs, ...regSkills]);
  const outPlugins = {};
  for (const [k, v] of Object.entries(plugins)) if (!isManaged(v)) outPlugins[k] = v;
  for (const [k, v] of Object.entries(regPlugins)) outPlugins[k] = v;

  let out = rest.replace(/\n+$/, '') + '\n';
  if (dirs.length) {
    out += '\n[skills]\nextra-dirs = [\n' + dirs.map((d) => `  ${JSON.stringify(d)},`).join('\n') + '\n]\n';
  }
  const pk = Object.keys(outPlugins);
  if (pk.length) {
    out += '\n[plugins]\n' + pk.map((k) => `${k} = ${JSON.stringify(outPlugins[k])}`).join('\n') + '\n';
  }
  return out.replace(/^\n+/, '');
}

// ---- registry + wiring ----

function readRegistry() {
  try {
    return JSON.parse(fs.readFileSync(registryFile(), 'utf8'));
  } catch {
    return { schema: 'packages/1', packages: [] };
  }
}
function writeRegistry(reg) {
  fs.mkdirSync(packagesDir(), { recursive: true });
  fs.writeFileSync(registryFile(), JSON.stringify(reg, null, 2) + '\n');
}

// Recompute the toml's managed sections from the whole registry.
function rewireToml() {
  const reg = readRegistry();
  const regSkills = [];
  const regPlugins = {};
  for (const p of reg.packages) {
    for (const s of p.skills || []) regSkills.push(s);
    for (const pl of p.plugins || []) for (const k of pl.kinds || []) if (k && !regPlugins[k]) regPlugins[k] = pl.path;
  }
  let text = '';
  try {
    text = fs.readFileSync(tomlFile(), 'utf8');
  } catch {
    /* fresh */
  }
  fs.mkdirSync(configDir(), { recursive: true });
  fs.writeFileSync(tomlFile(), mergeToml(text, uniq(regSkills), regPlugins));
}

function fetchPackage(src) {
  fs.mkdirSync(packagesDir(), { recursive: true });
  const base = path.join(packagesDir(), src.scheme, safeDir(src.name));
  if (src.scheme === 'npm') {
    fs.mkdirSync(base, { recursive: true });
    execFileSync('npm', ['install', src.spec, '--prefix', base, '--no-audit', '--no-fund', '--loglevel=error'], {
      stdio: 'inherit',
    });
    return path.join(base, 'node_modules', src.name);
  }
  // git
  fs.rmSync(base, { recursive: true, force: true });
  const [url, ref] = src.spec.split('#');
  execFileSync('git', ['clone', '--depth', '1', ...(ref ? ['--branch', ref] : []), url, base], { stdio: 'inherit' });
  if (fs.existsSync(path.join(base, 'package.json'))) {
    try {
      execFileSync('npm', ['install', '--prefix', base, '--no-audit', '--no-fund', '--loglevel=error'], { stdio: 'inherit' });
    } catch {
      /* deps optional */
    }
  }
  return base;
}

function install(input) {
  const src = parseSource(input);
  const pkgDir = fetchPackage(src);
  const { plugins, skills } = resolveResources(pkgDir);
  const pluginRecs = plugins.map((p) => ({ path: p, kinds: pingKinds(p) }));
  const reg = readRegistry();
  reg.packages = (reg.packages || []).filter((p) => p.source !== input);
  reg.packages.push({ source: input, scheme: src.scheme, name: src.name, dir: pkgDir, plugins: pluginRecs, skills });
  writeRegistry(reg);
  rewireToml();
  return { name: src.name, dir: pkgDir, plugins: pluginRecs, skills };
}

function listPackages() {
  return readRegistry().packages || [];
}

function remove(input) {
  const reg = readRegistry();
  const before = (reg.packages || []).length;
  reg.packages = (reg.packages || []).filter((p) => p.source !== input && p.name !== input);
  writeRegistry(reg);
  rewireToml();
  return before - reg.packages.length;
}

module.exports = {
  parseSource,
  resolveResources,
  splitToml,
  mergeToml,
  install,
  listPackages,
  remove,
  // paths (for tests)
  _packagesDir: packagesDir,
  _tomlFile: tomlFile,
};
