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

// The agent-qa config home (holds agent-qa.toml + packages/). Overridable via
// AGENT_QA_HOME so the workbench + tests can point at an isolated dir; defaults
// to ~/.agent-qa. The report-server reads the same override.
const configDir = () => process.env.AGENT_QA_HOME || path.join(os.homedir(), '.agent-qa');
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

// Resolve a package dir → { plugins, skills, personas, environments } (abs
// dirs/paths) from its `agent-qa` manifest, falling back to conventional
// plugins/ + skills/ + personas/ + environments/ dirs. personas/environments
// dirs hold flat `<id>.json` records the workbench discovers read-only.
function resolveResources(pkgDir) {
  let manifest = {};
  try {
    manifest = JSON.parse(fs.readFileSync(path.join(pkgDir, 'package.json'), 'utf8'))['agent-qa'] || {};
  } catch {
    /* no/!json package.json */
  }
  const plugins = [];
  const skills = [];
  const personas = [];
  const environments = [];
  const absList = (rel, into) => {
    for (const r of Array.isArray(rel) ? rel : []) {
      const abs = path.resolve(pkgDir, r);
      if (fs.existsSync(abs)) into.push(abs);
    }
  };
  absList(manifest.plugins, plugins);
  absList(manifest.skills, skills);
  absList(manifest.personas, personas);
  absList(manifest.environments, environments);
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
  if (!personas.length) {
    const d = path.join(pkgDir, 'personas');
    if (fs.existsSync(d)) personas.push(d);
  }
  if (!environments.length) {
    const d = path.join(pkgDir, 'environments');
    if (fs.existsSync(d)) environments.push(d);
  }
  return { plugins, skills, personas, environments };
}

// Scan a package's persona dirs for LITERAL (non-`vault:`) credential values
// and return their file paths. Shipping literal secrets in a shareable package
// is a leak; `install` warns (doesn't block). Best-effort — unreadable/!json
// files are skipped.
function personasWithLiteralSecrets(personaDirs) {
  const bad = [];
  for (const dir of personaDirs || []) {
    let files = [];
    try {
      files = fs.readdirSync(dir).filter((f) => f.endsWith('.json'));
    } catch {
      continue;
    }
    for (const f of files) {
      try {
        const rec = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'));
        const entries = (rec && rec.credentials && rec.credentials.entries) || {};
        const leak = Object.values(entries).some(
          (v) => typeof v === 'string' && v.trim() && !v.startsWith('vault:')
        );
        if (leak) bad.push(path.join(dir, f));
      } catch {
        /* skip unreadable/!json */
      }
    }
  }
  return bad;
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

// Split toml into { rest, extraDirs, personaDirs, envDirs, plugins } — rest is
// everything except the [skills]/[personas]/[environments]/[plugins] tables
// (preserved verbatim, so other config / comments survive). The three dir
// tables all carry an `extra-dirs = [...]` list; [plugins] is key = "path".
function splitToml(text) {
  const lines = String(text || '').split('\n');
  const rest = [];
  const blocks = { skills: [], personas: [], environments: [], plugins: [] };
  let cur = null; // null | 'skills' | 'personas' | 'environments' | 'plugins' | 'other'
  for (const line of lines) {
    const m = /^\s*\[([^\]]+)\]\s*$/.exec(line);
    if (m) {
      const t = m[1].trim();
      cur = Object.prototype.hasOwnProperty.call(blocks, t) ? t : 'other';
      if (cur === 'other') rest.push(line);
      continue;
    }
    if (cur && cur !== 'other') blocks[cur].push(line);
    else rest.push(line);
  }
  const dirsOf = (b) => {
    const out = [];
    for (const q of b.join('\n').matchAll(/"([^"]+)"/g)) out.push(q[1]);
    return out;
  };
  const plugins = {};
  for (const l of blocks.plugins) {
    const pm = /^\s*([A-Za-z0-9_-]+)\s*=\s*"([^"]+)"/.exec(l);
    if (pm) plugins[pm[1]] = pm[2];
  }
  return {
    rest: rest.join('\n'),
    extraDirs: dirsOf(blocks.skills),
    personaDirs: dirsOf(blocks.personas),
    envDirs: dirsOf(blocks.environments),
    plugins,
  };
}

const uniq = (a) => [...new Set(a)];

// Rebuild toml: preserve non-package dirs/plugins (user's manual ones), overlay
// the package-derived ones (everything under ~/.agent-qa/packages).
function mergeToml(text, regSkills, regPlugins, regPersonas = [], regEnvs = []) {
  const { rest, extraDirs, personaDirs, envDirs, plugins } = splitToml(text);
  const pkgRoot = packagesDir();
  const isManaged = (p) => expandHome(p).startsWith(pkgRoot);
  const mergeDirs = (existing, reg) => uniq([...existing.filter((d) => !isManaged(d)), ...reg]);
  const dirs = mergeDirs(extraDirs, regSkills);
  const pDirs = mergeDirs(personaDirs, regPersonas);
  const eDirs = mergeDirs(envDirs, regEnvs);
  const outPlugins = {};
  for (const [k, v] of Object.entries(plugins)) if (!isManaged(v)) outPlugins[k] = v;
  for (const [k, v] of Object.entries(regPlugins)) outPlugins[k] = v;

  const dirTable = (name, arr) =>
    arr.length
      ? `\n[${name}]\nextra-dirs = [\n` + arr.map((d) => `  ${JSON.stringify(d)},`).join('\n') + '\n]\n'
      : '';
  let out = rest.replace(/\n+$/, '') + '\n';
  out += dirTable('skills', dirs);
  out += dirTable('personas', pDirs);
  out += dirTable('environments', eDirs);
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
  const regPersonas = [];
  const regEnvs = [];
  const regPlugins = {};
  for (const p of reg.packages) {
    for (const s of p.skills || []) regSkills.push(s);
    for (const s of p.personas || []) regPersonas.push(s);
    for (const s of p.environments || []) regEnvs.push(s);
    for (const pl of p.plugins || []) for (const k of pl.kinds || []) if (k && !regPlugins[k]) regPlugins[k] = pl.path;
  }
  let text = '';
  try {
    text = fs.readFileSync(tomlFile(), 'utf8');
  } catch {
    /* fresh */
  }
  fs.mkdirSync(configDir(), { recursive: true });
  fs.writeFileSync(tomlFile(), mergeToml(text, uniq(regSkills), regPlugins, uniq(regPersonas), uniq(regEnvs)));
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
  const { plugins, skills, personas, environments } = resolveResources(pkgDir);
  const pluginRecs = plugins.map((p) => ({ path: p, kinds: pingKinds(p) }));
  // Guard against shipping real secrets: shared packages should carry only
  // `vault:` references, never literal credential values.
  const leaks = personasWithLiteralSecrets(personas);
  if (leaks.length) {
    console.error(
      `warning: ${src.name} ships persona(s) with LITERAL credential values (not vault: refs):\n` +
        leaks.map((f) => `  - ${f}`).join('\n') +
        `\n  Anyone who installs this package gets those secrets. Use vault: references instead.`
    );
  }
  const reg = readRegistry();
  reg.packages = (reg.packages || []).filter((p) => p.source !== input);
  reg.packages.push({
    source: input,
    scheme: src.scheme,
    name: src.name,
    dir: pkgDir,
    plugins: pluginRecs,
    skills,
    personas,
    environments,
  });
  writeRegistry(reg);
  rewireToml();
  return { name: src.name, dir: pkgDir, plugins: pluginRecs, skills, personas, environments };
}

function listPackages() {
  return readRegistry().packages || [];
}

// Compare an installed package against its remote to detect an available
// update. git → local HEAD vs `origin` HEAD; npm → installed package.json
// version vs `npm view <name> version`. Best-effort + network-bounded; any
// failure reports current='?' updateAvailable=false rather than throwing.
function packageUpdate(p) {
  try {
    if (p.scheme === 'npm') {
      const cur = JSON.parse(fs.readFileSync(path.join(p.dir, 'package.json'), 'utf8')).version || '?';
      const latest = execFileSync('npm', ['view', p.name, 'version'], {
        encoding: 'utf8',
        timeout: 20000,
      }).trim();
      return { current: cur, latest, updateAvailable: !!latest && latest !== cur };
    }
    const cur = execFileSync('git', ['-C', p.dir, 'rev-parse', 'HEAD'], {
      encoding: 'utf8',
      timeout: 20000,
    }).trim();
    const out = execFileSync('git', ['-C', p.dir, 'ls-remote', 'origin', 'HEAD'], {
      encoding: 'utf8',
      timeout: 25000,
    }).trim();
    const latest = (out.split(/\s+/)[0] || '').trim();
    return {
      current: cur.slice(0, 7),
      latest: latest.slice(0, 7),
      updateAvailable: !!latest && latest !== cur,
    };
  } catch {
    return { current: '?', latest: '?', updateAvailable: false };
  }
}

// Per-package update status for the whole registry (each entry gains
// current/latest/updateAvailable). Network-bound — call on demand.
function checkUpdates() {
  return listPackages().map((p) => ({
    source: p.source,
    name: p.name,
    scheme: p.scheme,
    ...packageUpdate(p),
  }));
}

// Re-install (re-pull) installed packages so newly-shipped resources —
// personas, environments, skills, plugin updates — flow in. With no argument,
// updates every registered package; else the one matching a name or source.
// Returns the install result for each updated package (empty if none matched).
function update(nameOrSource) {
  const reg = readRegistry();
  const targets = (reg.packages || []).filter(
    (p) => !nameOrSource || p.name === nameOrSource || p.source === nameOrSource,
  );
  return targets.map((p) => install(p.source));
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
  update,
  checkUpdates,
  listPackages,
  remove,
  // paths (for tests)
  _packagesDir: packagesDir,
  _tomlFile: tomlFile,
};
