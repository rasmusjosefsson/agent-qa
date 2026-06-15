# skill-data — embedded agent runbooks

These markdown files are embedded into the `agent-qa` binary at build time
and served by `agent-qa skills get <name>`.

### Skills

| File                   | Purpose                                        |
| ---------------------- | ---------------------------------------------- |
| `core/SKILL.md`        | Record/replay + live-page inspection           |
| `core/references/*.md` | Deep-dive references loaded with `core --full` |
| `byo/SKILL.md`         | Bring-your-own-browser bridge                  |
| `profiles/SKILL.md`    | Multi-profile replay & comparison              |
| `extend/SKILL.md`      | Adding per-repo plugins + skill content        |
