//! Plugin host — out-of-process subprocesses speaking JSON over stdio.
//!
//! See [`docs/plugins.md`](../../../docs/plugins.md) for the user-facing
//! protocol description. This module is the core: types, discovery,
//! invocation, plus the `plugins` verb implementation.
//!
//! Why subprocess + stdio (vs. dylib / WASM / embedded scripting): plugin
//! authors can write in any language, agent-qa has zero ABI surface, and
//! plugins routinely need network + filesystem + the ability to spawn
//! agent-browser anyway — sandboxing buys nothing here.

pub mod cli;
pub mod discovery;
pub mod host;
pub mod protocol;
