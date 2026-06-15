# Scenario Authoring

This reference used to describe the removed external prep authoring flow.
Do not create separate prep files or invoke standalone prep runners.

For scenario/v1, author seeded state directly on the scenario root:

- Put pre-step state in `setup`.
- Put cleanup in `teardown`.
- Use `setup.gql[].saveAs` for bindings consumed by later steps.
- Keep browser/DOM work in recorded `steps`, not setup.

Use [`prep.md`](./prep.md) as the current source of truth for setup,
teardown, supported channels, binding scope, and profile caveats.
