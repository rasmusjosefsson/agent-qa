# Scenario Anatomy

This reference used to describe the removed external prep-file authoring system.
Do not use it to author new scenarios.

Current scenario/v1 anatomy is inline:

- Root `setup` prepares state before step 0.
- Root `teardown` cleans up after replay.
- Setup bindings from `gql[].saveAs` share the same template scope as recording-time bindings.
- Recorded browser gestures remain in root `steps`.

Use [`prep.md`](./prep.md) for the current setup/teardown contract and
[`scenario-authoring.md`](./scenario-authoring.md) for the v1 authoring
guidance.
