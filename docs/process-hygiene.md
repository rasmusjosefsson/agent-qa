# Session hygiene

Every `record`/`replay` run spawns an agent-browser daemon plus a Chrome tree.
Crashes, `SIGKILL`, and wedged daemons leave pieces behind — registry files in
`~/.agent-browser`, orphaned Chrome processes, and stray profile dirs in
`/tmp/agent-browser-chrome-*`. `agent-qa ps` shows all of it; `agent-qa cleanup`
reaps it.

## agent-qa ps

```
agent-browser sessions  (state dir: /home/user/.agent-browser)

  SESSION                            STATE   PID     AGE    RSS       URL
  my-scenario                        live    40123   3m     412 MB    https://example.com
  old-run                            orphan  —       2d     —         —
  stale-glue                         stale   —       9d     —         —

  unowned Chrome processes: 13 (1.3 GB total)
    pid 55288  194 MB  /tmp/agent-browser-chrome-1ee1a606-…

  unowned agent-browser daemons: 1 (72 MB total)
    pid 10500  31m  72 MB

  stray profile dirs (no live chrome):
    /tmp/agent-browser-chrome-9f31ab2c-…
```

- **live** — `.sock` + `.pid` present and the daemon pid is running.
- **orphan** — registry files exist but the daemon pid is gone (killed replay,
  crashed daemon).
- **stale** — residue files (`.config`/`.target`) with no live socket, left by
  sessions that were never closed.
- **unowned Chrome** — a `--user-data-dir=/tmp/agent-browser-chrome-*` process
  whose ppid chain doesn't reach any live daemon. Its parent daemon is dead.
- **unowned daemons** — bare `agent-browser` daemon processes no registered
  session points at (registry deleted under them).
- **stray profile dirs** — profile dirs no Chrome process uses at all.

`agent-qa ps --json` emits the same inventory for scripting.

## agent-qa cleanup

```
agent-qa cleanup            # reap dead stuff: orphan/stale files, unowned
                            # chrome + daemons, stray dirs
agent-qa cleanup --all      # also `close --session` every live session
agent-qa cleanup --session my-scenario
agent-qa cleanup --older-than 1d
agent-qa cleanup --dry-run  # print the plan, touch nothing
agent-qa cleanup --json
```

A scoped `--session` cleanup against a *live* session only closes that session
— it does not sweep unrelated orphans. When a `close --session` call fails
(wedged daemon), cleanup terminates the recorded daemon pid directly and then
rescans: a session's Chrome children only become unowned after the daemon
dies, so the sweep re-collects the process table to catch them and remove
their profile dirs.

Nothing is deleted or killed without being listed first — run `--dry-run`
whenever you want the plan without the action.

## When to run it

- After a replay or recording crashes or is killed with `SIGKILL`.
- When `agent-qa doctor` reports sessions you don't recognize.
- Periodically on shared/CI boxes — residue accumulates (~hundreds of files a
  day under heavy eval use), and orphaned Chrome trees hold real memory.
