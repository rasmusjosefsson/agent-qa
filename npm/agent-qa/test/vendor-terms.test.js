// Guard: no downstream/tenant terms in tracked source. The core is
// vendor-neutral — product names the feature targets (jira, xray, acli)
// are fine, but customer specifics (tenant hosts, org names, real issue
// keys) must never land. Extend BLOCKLIST only with real violations.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const REPO = path.resolve(__dirname, '..', '..', '..');

// Case-insensitive term → why it's banned.
const BLOCKLIST = [
  ['outreach', 'tenant/org name — lives in the downstream extension, not core'],
];

// Files allowed to contain a blocklisted term — only this guard itself.
const ALLOW = new Set([
  'npm/agent-qa/test/vendor-terms.test.js',
  'npm/agent-qa/test/chat-agent.test.js', // asserts the same absence
]);

const SKIP_DIRS = new Set(['node_modules', 'dist', 'build', '.git']);
const TEXT_EXT = /\.(js|mjs|cjs|ts|tsx|jsx|rs|md|toml|json|yml|yaml|sh|py|html|css)$/;

function* walk(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (SKIP_DIRS.has(e.name)) continue;
    const p = path.join(dir, e.name);
    if (e.isDirectory()) yield* walk(p);
    else if (TEXT_EXT.test(e.name)) yield p;
  }
}

test('no downstream/tenant terms in tracked source', () => {
  const tracked = new Set(
    execFileSync('git', ['ls-files'], { cwd: REPO, encoding: 'utf8' })
      .trim()
      .split('\n'),
  );
  const hits = [];
  for (const rel of tracked) {
    if (ALLOW.has(rel)) continue;
    const file = path.join(REPO, rel);
    if (!fs.existsSync(file)) continue;
    let text;
    try {
      text = fs.readFileSync(file, 'utf8');
    } catch {
      continue; // binary or unreadable — skip
    }
    for (const [term, why] of BLOCKLIST) {
      const re = new RegExp(term, 'i');
      text.split('\n').forEach((line, i) => {
        if (re.test(line)) hits.push(`${rel}:${i + 1}: ${line.trim().slice(0, 100)}`);
      });
      void why;
    }
  }
  assert.equal(hits.join('\n'), '', `vendor terms found in tracked source:\n${hits.join('\n')}`);
});
