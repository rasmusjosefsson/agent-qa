---
name: extend
description: How to extend agent-qa from a downstream repo — add vendor-specific runtime behavior via plugins (auth, session policy, setup hooks, healing strategies) and add per-repo skill content via `[skills] extra-dirs`. Read this when the user asks "how do I add my own plugin", "how do I register auth for my product", "how do I ship a skill from my repo", "how do I extend agent-qa", or before authoring an `agent-qa-plugin-*` binary or new SKILL.md.
---

# Extending agent-qa from a downstream repo

Every app-specific concern lands via one of two extension points, registered in
`agent-qa.toml` files that the CLI auto-discovers in two layers:

- **Global** (~/.agent-qa/agent-qa.toml) — defaults for every cwd.
- **Per-repo** (nearest `agent-qa.toml` walked up from cwd) — overlay.

| Need | Extension point | Lives where |
|---|---|---|
| Runtime behavior (auth, session policy, setup hook, healing) | **Plugin** (JSON-over-stdio binary) | `[plugins]` table |
| Extra skill markdown the agent should read (runbooks, page maps, glossaries) | **Extra skill-data dir** | `[skills] extra-dirs` |

No fork of agent-qa is ever needed. The CLI stays version-locked to npm
releases; your repo (and your `$HOME`) own the vendor logic and runbooks.

## The config home (global)

Mirrors the `~/.agent-browser`, `~/.aws`, `~/.kube`, `~/.pi` family
convention. agent-qa looks for, in order:

1. `~/.agent-qa/agent-qa.toml`                 — **primary**
2. `$XDG_CONFIG_HOME/agent-qa/agent-qa.toml`   — XDG fallback
   (default `~/.config/agent-qa/agent-qa.toml`)

Typical layout:

```
~/.agent-qa/
├── agent-qa.toml           # [skills], [plugins], defaults
├── plugins/                # (optional) global plugin binaries
│   └── agent-qa-plugin-<vendor>
└── sessions/               # (future) auth/session vault
```

Minimal `~/.agent-qa/agent-qa.toml`:

```toml
[skills]
extra-dirs = [
  "/abs/path/to/some-repo/skill-data",
]

[plugins]
auth = "~/.agent-qa/plugins/agent-qa-plugin-<vendor>"
```

## The per-repo overlay

Drop `agent-qa.toml` anywhere on the path from cwd up to /. First match
wins for plugin kinds; `extra-dirs` are unioned with the global list.

```toml
[plugins]
auth = "./plugins/acme-auth/agent-qa-plugin-acme-auth"

[skills]
extra-dirs = [
  "./packages/ingestion/scenario-runner/skill-data",
]
```

Merge rules:

| Section | Merge |
|---|---|
| `[skills] extra-dirs` | **union** (global + repo dirs both apply) |
| `[plugins]` | **repo wins per-binary on dedupe**; for distinct binaries serving the same kind, the higher-priority entry resolves via `plugins path <kind>` |

Verify with:

```bash
agent-qa plugins list      # what plugins were discovered + from where
agent-qa plugins doctor    # ping each, report kinds served
agent-qa skills list       # merged catalogue: embedded + external
```

---

## Part 1 — adding a plugin (runtime behavior)

### When to write a plugin

Reach for a plugin when agent-qa needs to **do** something product-specific
during recording or replay: log into Okta, mint a session cookie, apply a
custom session-policy expiry, invoke a setup hook before the first step,
or supply a vendor-specific healing strategy.

If you just want to ship **reading material** for the agent (runbooks,
page maps, glossaries), use Part 2 — no plugin needed.

### What a plugin is

An out-of-process binary that:

1. Is spawned by agent-qa with positional args: `<binary> <kind> [<op>]`
2. Reads one JSON request envelope on stdin
3. Writes one JSON response envelope on stdout
4. Exits

Any language that can read stdin / write stdout / parse JSON qualifies. A
10-line shell script is a valid plugin.

### Wire contract

Every plugin **must** implement the `ping` kind:

```bash
$ ./agent-qa-plugin-acme-auth ping
{"protocolVersion":1,"request":{}}    # ← stdin
```

Response:

```json
{
  "protocolVersion": 1,
  "response": {
    "name": "acme-auth",
    "protocolVersion": 1,
    "kinds": ["ping", "auth"]
  }
}
```

Currently defined kinds: `ping`, `auth`, `session-policy`,
`setup-hook`, `heal-strategy`, `discovery-defaults`. Per-kind payload shapes are documented inline with
each verb's port. Full wire-contract reference: `docs/plugins.md` in the
agent-qa repo.

### Skeleton (shell)

```bash
#!/usr/bin/env bash
# plugins/acme-auth/agent-qa-plugin-acme-auth
set -euo pipefail
kind="${1:-}"
case "$kind" in
  ping)
    echo '{"protocolVersion":1,"response":{"name":"acme-auth","protocolVersion":1,"kinds":["ping","auth"]}}'
    ;;
  auth)
    payload=$(cat)   # request JSON on stdin
    # …perform Okta/SSO/whatever using "$payload"…
    echo '{"protocolVersion":1,"response":{"ok":true,"session":{"cookies":[]}}}'
    ;;
  *)
    echo "{\"protocolVersion\":1,\"error\":{\"message\":\"unknown kind $kind\"}}" >&2
    exit 2
    ;;
esac
```

```bash
chmod +x plugins/acme-auth/agent-qa-plugin-acme-auth
```

Reference scaffold: `examples/plugins/noop-auth/` in the agent-qa repo.

### Register it

In `agent-qa.toml`:

```toml
[plugins]
auth = "./plugins/acme-auth/agent-qa-plugin-acme-auth"
```

### Discovery priority (first match wins per binary)

1. `--plugin <path>` CLI flag (repeatable, ad-hoc override)
2. `agent-qa.toml [plugins]` table (per-repo, walked up from cwd)
3. Global config — `~/.agent-qa/agent-qa.toml` or XDG fallback
4. `AGENT_QA_PLUGINS` env var, colon-separated paths
5. Any executable on `$PATH` named `agent-qa-plugin-*` (gh-extension style)

### Verify

```bash
$ agent-qa plugins doctor
OK  ./plugins/acme-auth/agent-qa-plugin-acme-auth (config:/abs/path/agent-qa.toml)
    — name=acme-auth, protocol=1, kinds=[ping,auth]
```

---

## Part 2 — adding skill content (per-repo runbooks)

### When to add an extra skill-data dir

Reach for this when you want the agent to **read** something
product-specific before acting — runbooks like "how to navigate the admin
console", page-route maps, glossaries, in-house naming conventions, or a
vendor-specific deep-dive on how `agent-qa core` should be used for your
product.

If the content is generic guidance you'd want every agent-qa user to see,
contribute it upstream to `skill-data/` in the agent-qa repo. If it's
specific to your product or repo, keep it local and register it via
`[skills] extra-dirs`.

### One-command scaffold

For most cases you don't need to remember the layout — use:

```bash
agent-qa skills scaffold <name>
# e.g.
agent-qa skills scaffold pages
agent-qa skills scaffold glossary --dir ./packages/foo/skill-data
```

This writes a starter `SKILL.md` with the right frontmatter and registers
the containing directory in `agent-qa.toml` (creating the file with a
`[skills]` section if needed). It refuses to overwrite an existing skill
and will not duplicate an already-registered `extra-dirs` entry. Then
edit the generated file — fill in `description` (be specific about
*when* the agent should load this) and the body.

### Recommended downstream skills

There is no fixed list, but these slots show up in almost every product
deployment. Use the same name across repos and your agents will pick
them up by convention:

| Skill      | When the agent should load it |
|------------|-------------------------------|
| `pages`    | Known target routes — named pages → URL patterns. Load before navigating or asserting on URLs so the agent stops guessing. |
| `glossary` | In-house terminology + canonical spellings. Load when transcribing user input or naming things in scenarios. |
| `auth`     | How to bootstrap an authenticated session (cookies, SSO dance, test users). Load before recording any flow that requires login. |
| `flags`    | Feature-flag atlas — flag names + what they gate. Load when a flow depends on a flag being on/off. |

None of these ship with the binary — scaffold the ones you need.

### Layout

Mirror the embedded layout — one subdirectory per skill, each containing
a `SKILL.md`:

```
packages/ingestion/scenario-runner/skill-data/
├── pages/
│   └── SKILL.md
├── acme/
│   └── SKILL.md
└── …
```

### Authoring a SKILL.md

```markdown
---
name: pages
description: Route map for the Acme admin console. Read this when the
  agent needs to navigate between admin pages or resolve a route name to
  a URL pattern. Companion to agent-qa `core` — that one covers
  record/replay mechanics; this one is the product-specific page atlas.
---

# Pages — Acme admin console route map

…body…
```

The `description` is what `agent-qa skills list --json` returns and what
agents use to decide whether to load the skill. Be specific about
**when** to read it; vague descriptions get ignored.

### Register it

In `agent-qa.toml`:

```toml
[skills]
extra-dirs = [
  "./packages/ingestion/scenario-runner/skill-data",
]
```

Paths are relative to the toml file (or absolute). Multiple dirs allowed.

### Discovery + precedence

- Embedded skills (`core`, `byo`, `profiles`) always win on name collision
  — the binary is the source of truth for them.
- External skills are unioned in alongside the embedded set.
- Missing dirs are silently skipped (no error), so it's safe to leave
  entries pointing at sibling packages that may or may not be checked out.

### Verify

```bash
$ agent-qa skills list
byo
core
acme        ← from external dir
pages           ← from external dir
profiles

$ agent-qa skills list --json
[
  { "name": "byo",      "source": "embedded", "path": "skill-data/byo/SKILL.md" },
  { "name": "core",     "source": "embedded", "path": "skill-data/core/SKILL.md" },
  { "name": "acme", "source": "external", "path": "/abs/…/skill-data/acme/SKILL.md" },
  { "name": "pages",    "source": "external", "path": "/abs/…/skill-data/pages/SKILL.md" },
  { "name": "profiles", "source": "embedded", "path": "skill-data/profiles/SKILL.md" }
]

$ agent-qa skills get pages       # served from disk
$ agent-qa skills get core        # served from embedded binary
```

---

## Pi / Claude-Code discovery stub

If you want your downstream repo's skill catalogue to be auto-loaded by
the user's AI coding agent (pi, Claude Code, Cursor, Codex, …), ship a
thin pointer SKILL.md at the standard top-level location and let the
`skills` package install it:

```
your-repo/skills/<your-repo>/SKILL.md   ← stub
```

The stub should be short and direct the agent at
`agent-qa skills get <name>` for the actual workflow content — same
pattern as agent-qa's own `skills/agent-qa/SKILL.md`. Then users run:

```bash
npx skills add <your-gh-org>/<your-repo>
```

and the stub lands at `~/.agents/skills/<your-repo>/SKILL.md`.

---

## Mental model

```
┌──────────────────────────────────────────────────────────────────────┐
│ agent-qa (npm, version-locked)                                       │
│   embedded skills:  core, byo, profiles                              │
│   plugin protocol:  ping + per-kind verbs                            │
└──────────────────────────────────────────────────────────────────────┘
                              ▲                ▲
                              │                │
                  agent-qa.toml  ───────────────│
                  [plugins]  ─── runtime ───▶ your plugin binaries
                  [skills]   ─── reading ───▶ your skill-data dirs
                              │
┌─────────────────────────────┴────────────────────────────────────────┐
│ your repo (vendor-specific, lives outside agent-qa)                  │
│   plugins/<vendor>-auth/agent-qa-plugin-<vendor>-auth                │
│   packages/.../skill-data/{pages,acme,…}/SKILL.md                │
│   skills/<your-repo>/SKILL.md   ← pi/Claude discovery stub           │
└──────────────────────────────────────────────────────────────────────┘
```

One config file. Two extension points. Zero forks.
