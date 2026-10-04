# Flow DSL

The flow DSL is a compact line format for writing scenarios by hand or having
an agent draft them: one step per line, grouped blocks with `group`/`when`, and
a `compile` step that emits a standard `scenario/2` document. Replay,
lint, goldens — everything downstream is unchanged because the output is an
ordinary scenario file.

```text
title Sign-in flow
base https://app.example.com
input user = "default-user"
input pass

open https://app.example.com/login
! sign in with valid credentials
fill "Username" with {{inputs.user}}
fill "Password" with {{inputs.pass}}
click "Sign in"
check url matches "/dashboard"
shot dashboard
```

```bash
agent-qa compile sign-in.flow            # writes sign-in.json (scenario/2)
agent-qa compile sign-in.flow --check    # validate only, no output file
agent-qa describe sign-in                # scenario → line DSL on stdout
agent-qa describe sign-in --out back.flow
```

`describe` is the inverse: every step it can express becomes a DSL line and the
step's intent becomes a `!` line, so `describe → compile` is byte-stable for
supported steps. Steps the DSL can't express (a `mail` verb, an env op, a raw
claim shape) are emitted as `# unsupported:` comments plus a stderr warning —
never silently dropped.

## Lines

| Line | Emits |
|---|---|
| `title <text>` | scenario `intent` |
| `base <url>` | `env.open` nav op |
| `input <name>` / `input <name> = "<default>"` | `inputs` declaration |
| `open <url>` | `goto` step |
| `click` `dblclick` `hover` `clear` `focus` `blur` `<loc>` | the do-verb |
| `tick` / `untick` <loc> | `check` / `uncheck` do-verb |
| `fill <loc> with <text>` | `type` step; a quoted loc (`"Label"`) targets role `textbox` |
| `type <text> into <loc>` | `type` step |
| `press <key>` | `press` step |
| `select <loc> = <value>` | `select` step |
| `scroll <loc>` / `scroll bottom` / `scroll <px>` | `scroll` step |
| `wait <N>ms` / `wait <loc>` / `wait url <x>` / `wait idle` | `wait` step |
| `viewport <W>x<H>` | `viewport` step |
| `check <loc> <predicate>` / `check url <predicate> <x>` / `check console quiet` | claim step |
| `shot <name>` | shot claim bound to the previous `open`/`goto` step |
| `group <label>` … `end` | `group` step |
| `when present <loc>` … `end` | `when` step |
| `include <file>` | `include` step |
| `! <text>` | next step's `intent` |
| `#` / `//` | comment |

`{{inputs.name}}`, `{{vars.x}}`, and env templating work anywhere a literal
does — the text is passed through verbatim.

## Locators

`css:` `xpath:` `testid:` `text:` prefixes, `"…"` for visible text, `role:<r>`
or `role:<r>:"name"` (or `role:<r>:<name>`) for ARIA, and the shorthands
`#id`, `.class`, `[attr]`, `> descendant`. Check predicates: `exists`,
`notExists`, `visible`, `hidden`, `text`, `text contains`, `text starts`,
`text ends`, `count`, `value`, `attr:<name>`, plus `url` predicates
`is`/`matches`/`contains`/`starts`/`ends`.

## Try it

```bash
agent-qa compile demo.flow
agent-qa replay demo          # normal replay path
agent-qa shot-accept demo --steps s1
agent-qa describe demo        # back to text for review/diff
```

The DSL is deliberately narrow — anything it can't express stays in JSON.
Treat it as an authoring surface: edit the `.flow`, recompile, keep the
compiled `scenario.json` as the source of truth for replay.
