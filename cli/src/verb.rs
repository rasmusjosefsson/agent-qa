//! Shared verb conventions.
//!
//! Every verb is an `fn(&[String]) -> anyhow::Result<u8>` where the `u8` is
//! the process exit code. Errors short-circuit to exit 1 with a stderr
//! message printed by `main`.

#[allow(dead_code)]
pub type VerbResult = anyhow::Result<u8>;
