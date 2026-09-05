# Scenario anatomy

New recordings use `scenario/2`. The recorder writes the local recording state,
then `flush` seals `scenario.json`.

Keep replay setup in `env.open` and cleanup in `env.close`. Keep browser actions
and assertions in `steps`. Use `record-setup` to add generic setup operations
without editing the scenario file.

See `schema.md` for the contract and `scenario-authoring.md` for commands.
