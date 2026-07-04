//! Test-only utilities shared across the crate's `#[cfg(test)]` modules.
//!
//! Right now this only exports a process-global mutex that serializes
//! env-var mutation across all integration-style tests. Without it,
//! `cargo test`'s parallelism races on `AGENT_BROWSER_BIN` and friends.
#![cfg(test)]

use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the shared env lock. Returns the guard regardless of prior
/// poisoning so a panic in one test doesn't disable the lock for the
/// rest of the suite.
pub fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    // DOM-activation retries poll with real sleeps; disable them under test so
    // "no match" paths return immediately instead of burning seconds.
    std::env::set_var("AGENT_QA_DOM_ACTIVATE_NO_RETRY", "1");
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
