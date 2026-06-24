//! agent-qa — verb dispatcher.
//!
//! Currently shipped verbs:
//!   - `skills` — serve embedded agent runbooks
//!   - `plugins` — plugin discovery + ping
//!   - `scenario validate <file>` — validate a scenario.json against the schema
//!   - `replay <sid|path>` — runner shell (placeholder step dispatch)

use std::process::ExitCode;

mod aria_snapshot;
mod audit;
mod browser;
mod buffer;
mod byo_doctor;
mod cdp_url;
mod claims;
mod compare;
mod config;
mod doctor;
mod env_ops;
mod fill_unique;
mod flush;
mod global_config;
mod heal_apply;
mod heal_list;
mod heal_promote;
mod heal_respond;
mod info;
mod io;
mod list;
mod paths;
mod perf_snapshot;
mod plugin;
mod profile_add;
mod profile_bootstrap;
mod profile_list;
mod profile_status;
mod record_step;
mod recorder_shape;
mod run_step;
mod runner;
mod scenario;
mod scenario_cli;
mod schema;
mod sidecar;
mod skills;
mod smart_click;
mod start;
mod test_util;
mod time;
mod truncate;
mod value;
mod verb;
mod verb_shape;
mod verbs;
mod verify;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        print_help();
        return ExitCode::from(0);
    }
    if matches!(args[0].as_str(), "-V" | "--version" | "version") {
        let want_json = args.iter().any(|a| a == "--json");
        let v = env!("CARGO_PKG_VERSION");
        if want_json {
            println!("{{\"name\":\"agent-qa\",\"version\":\"{v}\"}}");
        } else {
            println!("agent-qa {v}");
        }
        return ExitCode::from(0);
    }

    let verb = &args[0];
    let rest = &args[1..];

    let result = match verb.as_str() {
        "skills" => skills::run(rest),
        "plugins" => plugin::cli::run(rest),
        "scenario" => scenario_cli::run(rest),
        "replay" => runner::cli(rest),
        "doctor" => doctor::run(rest),
        "info" => info::run(rest),
        "config" => config::run(rest),
        "list" => list::run(rest),
        "compare" | "diff" => compare::run(rest),
        "audit" => audit::run(rest),
        "start" => start::run(rest),
        "record-step" => record_step::run(rest),
        "run-step" => run_step::run(rest),
        "aria-snapshot" => aria_snapshot::run(rest),
        "cdp-url" => cdp_url::run(rest),
        "buffer" => buffer::run(rest),
        "flush" => flush::run(rest),
        "verify" => verify::run(rest),
        "truncate" => truncate::run(rest),
        "profile-add" => profile_add::run(rest),
        "profile-status" => profile_status::run(rest),
        "profile-bootstrap" => profile_bootstrap::run(rest),
        "profile-list" => profile_list::run(rest),
        "byo-doctor" => byo_doctor::run(rest),
        "perf-snapshot" => perf_snapshot::run(rest),
        "fill-unique" => fill_unique::run(rest),
        "smart-click" => smart_click::run(rest),
        "heal-respond" => heal_respond::run(rest),
        "heal-promote" => heal_promote::run(rest),
        "heal-apply" => heal_apply::run(rest),
        "heal-list" => heal_list::run(rest),
        _ => {
            eprintln!(
                "agent-qa: unknown verb {verb:?}. Implemented: skills, plugins, scenario, replay, doctor, config, list, compare, audit, start, record-step, run-step, aria-snapshot, cdp-url, buffer, flush, verify, truncate, profile-add, profile-status, profile-bootstrap, profile-list, byo-doctor, perf-snapshot, fill-unique, smart-click, heal-respond, heal-promote, heal-apply, heal-list."
            );
            eprintln!("Run `agent-qa --help` for usage.");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("agent-qa {verb}: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn print_help() {
    println!(
        "agent-qa — record and replay user scenarios.

Usage: agent-qa <verb> [args...]

Verbs:
  skills [list]                 List embedded agent skills
  skills get <name>             Print a skill's markdown
  skills path [name]            Print the embedded path for a skill
  skills scaffold <name> [--dir <path>]
                                Write a template SKILL.md for a new
                                downstream skill (pages, glossary, auth, …)
                                and register its parent dir in agent-qa.toml.
  plugins list                  Enumerate discovered plugins + kinds they serve
  plugins doctor                Ping every discovered plugin and report status
  plugins path <kind>           Print the resolved plugin binary for a kind
  scenario validate <file>       Validate a scenario.json against the schema
  scenario ls                    List every sid under the scenarios root
  scenario latest                Print the most recently modified sid
  scenario count                 Print the number of scenarios under the root
  scenario check    <file>       validate + lint in one pass
  scenario check-all             validate + lint across every scenario
  scenario summary  <file>       Per-step summary (id, kind, verb/claim)
  scenario inputs   <file> [--json]  List declared inputs
  scenario new      <file>       Scaffold a minimal valid scenario.json
  scenario diff     <a> <b>      Unified diff between two scenario JSON files
  scenario hash     <file>       SHA-256 of scenario bytes (rebase-guard hash)
  scenario id       <file>       Print the scenario's id field
  scenario intent   <file>       Print the scenario's intent field
  scenario step-ids <file>       Print every step id, one per line
  scenario field    <file> <n>   Print any top-level scenario field
  scenario coverage <file>       Per-step check coverage report
  scenario lint     <file>       Lint a scenario (duplicate ids, bare do, etc.)
  scenario lint-all              Lint every scenario under the scenarios root
  scenario rename   <sid> <new>  Rename a scenario directory + id field
  scenario copy     <sid> <new>  Copy a scenario (replays not copied)
  scenario delete   <sid> --yes  Remove a scenario directory + all replays
  scenario prune-replays <sid> --keep N  Cap replay history under <sid>
  scenario prune-all --keep N    Cap replay history across every scenario
  audit show <sid> <runId|latest>  Pretty-print one replay's audit.json
  audit list <sid>                 Table of every run under the scenario
  audit stats <sid>                Pass/fail/tag rollup across all runs
  audit stats-all                  Per-scenario + overall pass/fail rollup
  audit diff <sid> <runIdA> <runIdB>  Unified diff between two replays
  audit summary <sid> <runId|latest>  Print just the summary line
  audit exit-code <sid> <runId|latest>  Print just the exitCode (-1 if missing)
  audit field <sid> <runId|latest> <field>  Print any top-level audit field
  audit count <sid>                Print the number of runs under <sid>
  audit duration <sid> <runId|latest>  Print run duration in seconds
  profile-list                  List registered profiles (id, adapter, default, registeredAt)
  replay <sid | path>           Re-execute a scenario/2 document
  doctor [--json]               Diagnose the local install (agent-browser + plugins)
  info [--json]                 Version + paths + scenario/profile counts
  config show [--json]          Resolve config (toml file, paths, discovered plugins)
  list [<sid|path>] [--json]    Show scenario directory contents + replay history
  compare <sid> [<runA>] [<runB>]   Diff per-step ARIA snapshots between two runs
  diff                          (alias of `compare`)
  start \"<intent>\"             Begin a new recording session
  record-step <do|check> <json> Append a step to the in-flight buffer
  run-step <kind> <payload-json>   Dispatch ONE step against the live session
  aria-snapshot [--json]        Dump the live ARIA tree (element picker data)
  cdp-url [--json]              Print the live session's CDP WebSocket endpoint
  buffer list|delete|move|clear Inspect / reorder / delete the in-flight buffer
  flush                         Assemble scenario.json from the buffer
  verify                        Sanity-check the in-flight buffer
  truncate <N> [--archive-tag <slug>]   Drop steps ≥ N + archive sidecars
  profile-add <id> [flags]      Register a per-identity profile
  profile-status <id> [flags]   Probe profile auth via plugin
  profile-bootstrap <id> [flags] Drive plugin auth login for a profile
  byo-doctor [--json]           Read-only enumeration of attachable CDP browsers
  perf-snapshot [--sid <sid>]   Capture navigation + paint timings
  fill-unique <Label> --template <…>   Fill a unique-token field + record
  smart-click \"<name>\" [--role <r>]   Click by accessible name + auto-record
  heal-respond <sid> --step <id> (--value <…> | --reject)   Record a caller-driven heal
  heal-promote <sid> [--run <id>] [--steps <…>] [--apply]   Promote replay-side patches
  heal-apply <sid> --step <id> [--target-step <…>] [--dry-run]   Patch buffer in place
  heal-list <sid> [--run <id>] [--json]    List heal-responses + applied state

Step dispatch covers `do` verbs and `check` claims."
    );
}

#[cfg(test)]
mod dispatch_audit {
    //! Guard against a bug class: a verb advertised in the help
    //! table without a matching dispatch arm. Read this file at compile
    //! time and assert parity between (top-level help rows) and (match
    //! arms in the verb dispatcher).
    use std::collections::HashSet;

    const SRC: &str = include_str!("main.rs");

    /// Verbs that are intentionally NOT top-level (sub-verbs like
    /// 'list', 'doctor', 'path' under `plugins`; or aliases like 'help').
    /// These appear in the help text but aren't dispatched at the top.
    const NON_TOPLEVEL: &[&str] = &[
        "list",
        "get",
        "path",
        "validate",
        "validate-all",
        "summary",
        "inputs",
        "new",
        "diff",
        "hash",
        "id",
        "intent",
        "step-ids",
        "field",
        "coverage",
        "lint",
        "lint-all",
        "check",
        "check-all",
        "rename",
        "copy",
        "delete",
        "prune-replays",
        "prune-all",
        "ls",
        "latest",
        "count",
        "show",
        "stats",
        "stats-all",
        "exit-code",
        "duration",
        "doctor",
        "plugins",
        "audit",
        "skills",
        "scenario",
        "replay",
    ];

    /// Extract every quoted-string used on the LHS of `=>` in the main
    /// verb match. Captures aliased rows like `"a" | "b" => ...` too.
    fn dispatched_verbs() -> HashSet<String> {
        let mut out = HashSet::new();
        for line in SRC.lines() {
            if !line.contains("=>") {
                continue;
            }
            // Scan every "<word>" token on the line; OR-patterns mean a
            // single arm can introduce several verb names.
            let mut rest = line;
            while let Some(open) = rest.find('\"') {
                let after = &rest[open + 1..];
                if let Some(close) = after.find('\"') {
                    let token = &after[..close];
                    if !token.is_empty()
                        && token.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                    {
                        out.insert(token.to_string());
                    }
                    rest = &after[close + 1..];
                } else {
                    break;
                }
            }
        }
        out
    }

    /// Extract every first-word from the help table (lines starting
    /// with two-space indent, alphabetic first char).
    fn help_topverbs() -> HashSet<String> {
        let mut out = HashSet::new();
        for line in SRC.lines() {
            // Help table rows: '  word ...'.
            let trimmed = line.strip_prefix("  ").unwrap_or("");
            if !trimmed
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase())
            {
                continue;
            }
            let first = trimmed.split_whitespace().next().unwrap_or("");
            // Skip the second-word verbs that are sub-verbs of a
            // hyphenless top-level (e.g. 'plugins list' has 'plugins'
            // as the top-level word; the 'list' on the 'audit list' row
            // is also captured here but we know audit is a top-level
            // verb already).
            if first.is_empty() {
                continue;
            }
            out.insert(first.to_string());
        }
        out
    }

    #[test]
    fn every_top_level_help_verb_has_a_dispatch_arm() {
        let dispatched = dispatched_verbs();
        let help = help_topverbs();
        let non_toplevel: HashSet<&'static str> = NON_TOPLEVEL.iter().copied().collect();
        for verb in &help {
            if dispatched.contains(verb.as_str()) {
                continue;
            }
            if non_toplevel.contains(verb.as_str()) {
                continue;
            }
            panic!(
                "help table mentions top-level verb {verb:?} but no \"{verb}\" => match arm exists in main.rs's dispatcher"
            );
        }
    }
}
