Set up agent-qa in this repository so a scenario can be recorded and replayed. Run every command yourself from the repository root. Do not swap in another browser driver. Do not commit, and do not edit application code. When you finish, report the items in the last section.

## Runtime

Node.js 24 or newer and npm are required. The bundled agent-browser package sets `engines.node` to `>=24`, and CI installs Node 24.

1. Run `node -v` and `npm -v`.
2. If either command is missing, or Node's major version is below 24, install Node.js 24 with a version manager already on this machine (`nvm`, `fnm`, `volta`, or `asdf`). If none is installed, stop and tell the user Node.js 24 or newer is required.

## Install the CLI

Use npm for this package, including when the repo uses another package manager for its own dependencies.

```bash
npm i -g @rasmusjosefsson/agent-qa
agent-qa info
```

`agent-qa info` must print a line starting with `agent-qa ` and a version. If the global install fails with a permissions error, install it in this repo and call that binary for every later `agent-qa` command:

```bash
npm install --save-dev @rasmusjosefsson/agent-qa
npx agent-qa info
```

## Install the browser

The CLI ships a pinned agent-browser. Download Chrome through that binary. Do not install a second agent-browser.

```bash
node -p process.platform
agent-qa browser install
```

`process.platform` prints `linux`, `darwin`, or `win32`. `agent-qa browser install` is the install on every OS. On Linux, if a later browser command fails with a missing shared library, rerun:

```bash
agent-qa browser install --with-deps
```

That uses the OS package manager and may ask for sudo.

## Install the agent skill

The stub is what coding agents discover. The workflow text is embedded in the CLI and must match the installed version, so load it with `agent-qa skills get core`.

```bash
npx skills add rasmusjosefsson/agent-qa --skill agent-qa -y
agent-qa skills list
agent-qa skills get core
```

`--skill agent-qa` installs only that stub. `-y` skips prompts. `skills list` must include `core`. `skills get core` must print markdown titled `agent-qa core`. Keep the stub path the installer prints (usually `.agents/skills/agent-qa/SKILL.md`).

## Bootstrap the repo

```bash
agent-qa init
agent-qa plugins doctor
```

`agent-qa init` is idempotent. It writes `agent-qa.toml` with `scenarios_root = ./scenarios`, `scenarios/hello/scenario.json`, and an agent-qa block in `.gitignore`. Do not pass `--force`. Pass `--ci` only when the user asked for a pull-request replay workflow; `agent-qa init --help` lists that flag.

`plugins doctor` exits 0 with `(no plugins discovered)` when the repo has no plugins. That is a successful setup. Do not invent a plugin. Apps that need a login register one later in `agent-qa.toml`.

## Prove it works

```bash
agent-qa doctor
agent-qa doctor --deep https://example.com/
agent-qa replay hello
```

All three must succeed:

- `agent-qa doctor` ends with `overall: OK` and an OK agent-browser probe.
- `agent-qa doctor --deep https://example.com/` includes an OK `deep` probe whose `finalUrl` contains `example.com`.
- `agent-qa replay hello` exits 0 and prints `SUMMARY: 2/2 (PASS)`.

If the deep probe or the replay fails on a missing shared library on Linux, run `agent-qa browser install --with-deps` and retry those two commands once. If example.com cannot be reached, say so and still report the doctor output.

## Report back

Tell the user:

- the `agent-qa info` version line
- the `overall` line from `agent-qa doctor`
- the deep probe's `finalUrl`
- the `replay hello` summary line and exit code
- the skill stub path
- files written or already present (`agent-qa.toml`, `scenarios/hello/scenario.json`, `.gitignore`)
- anything you skipped, and why
