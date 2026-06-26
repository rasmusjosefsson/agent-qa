// Embeds a truthful version string into the binary at build time.
//
//   release (CI):  AGENT_QA_RELEASE=1 is set and the build is cut from a clean
//                  tagged checkout, so the bare crate version is the truth
//                  (e.g. "0.0.17") — no git suffix.
//   dev:           append the git short sha (+ ".dirty" when the tree has
//                  uncommitted changes) so a local build can't masquerade as a
//                  release, e.g. "0.0.1+g1a2b3c4d5.dirty".
//
// Exposed to main.rs via option_env!("AGENT_QA_VERSION").

use std::process::Command;

fn main() {
    // Keep the embedded sha fresh when HEAD moves or the tree changes.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
    println!("cargo:rerun-if-env-changed=AGENT_QA_RELEASE");

    let base = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();

    let version = if std::env::var_os("AGENT_QA_RELEASE").is_some() {
        base
    } else {
        match git_suffix() {
            Some(suffix) => format!("{base}+{suffix}"),
            None => base,
        }
    };

    println!("cargo:rustc-env=AGENT_QA_VERSION={version}");
}

fn git_suffix() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--short=9", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if sha.is_empty() {
        return None;
    }
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    Some(format!("g{sha}{}", if dirty { ".dirty" } else { "" }))
}
