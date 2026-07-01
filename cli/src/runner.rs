//! scenario/2 replay runner.
//!
//! Lifecycle (mirrors the TS runner header):
//!
//!   1. Load + validate the artifact against `scenario-schema.json`.
//!   2. Mint `<runId>` and create `<scenarioDir>/replays/<runId>/`.
//!   3. Write initial `audit.json` (`startedAt` + `scenarioContentHash`).
//!   4. Resolve `inputs` (declared defaults; CLI overrides land later).
//!   5. Run `env.open[*]` as setup. `nav` is wired end-to-end. Every
//!      other kind raises a structured `not-implemented` boundary so
//!      consumers don't silently no-op.
//!   6. Iterate `steps[]`. Each step:
//!        a. render a live progress line to stderr (N/M counter +
//!           pass/fail glyph in a TTY; a durable plain line when piped;
//!           nothing under `--quiet`)
//!        b. dispatch on `kind` — this path ships a placeholder that
//!           always reports success and writes no sidecars. The real
//!           `do` dispatcher and `check` handling live in the runner.
//!   7. Run `env.close[*]` as teardown — best-effort, even on step
//!      failure (matches spec).
//!   8. Print the literal `SUMMARY: N/M (PASS|FAIL)` line to stderr +
//!      write final `audit.json`.
//!   9. Update `replays/latest.txt`.
//!
//! `scenario.json` is NEVER mutated. Sidecars are written atomically.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

use crate::browser;
use crate::claims::{dispatch_check, CheckContext};
use crate::env_ops;
use crate::paths;
use crate::scenario::{InputDecl, InputType, Locator, NameMatch, Scenario, Step};
use crate::schema;
use crate::sidecar::{
    append_event, hash_scenario_bytes, mint_run_id, prepare_run_root, update_latest_pointer,
    write_run_audit, write_run_status, InputType as AuditInputType, ParameterSource, RunAudit,
    RunAuditParameter, RunStatus, StepEvent,
};
use crate::value::ValueScope;
use crate::verbs::{dispatch_do, DoContext};

const DEFAULT_SESSION: &str = "default";

#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Either the path to a `scenario.json` file or a sid that resolves
    /// to `<scenarios_root>/<sid>/scenario.json`.
    pub source: ScenarioSource,
    pub profile: Option<String>,
    pub session_name: String,
    /// `--heal-from-run <runId>` — pre-load caller-driven heal overrides
    /// from a prior run's `replays/<runId>/heal-responses/<stepId>.json`
    /// files. At step dispatch time, any do-step whose id appears in
    /// the map has its `value` replaced with the corrected literal.
    pub heal_from_run: Option<String>,
    /// `--param name=value` overrides. Resolved + coerced against the
    /// declared input type at runner start; sensitive entries are
    /// recorded as `[REDACTED]` in `audit.parameters[]` but flow
    /// through verbatim to `scope.inputs`. Duplicate `name=value` is
    /// last-wins.
    pub input_overrides: BTreeMap<String, String>,
    /// `--dry-run` — load + validate + mint run id + write the initial
    /// audit row, but skip env.open / env.close and step dispatch.
    /// The run directory still gets created and audit.json carries
    /// `summary: 'SUMMARY: 0/N (DRY-RUN)'`. Useful in CI to confirm a
    /// scenario is dispatchable without driving Chrome.
    pub dry_run: bool,
    /// `--no-sidecars` — skip per-step ARIA snapshot + screenshot capture.
    /// audit.json is still written. Useful when running with `--runs N`
    /// for flake detection where the per-step forensics aren't needed.
    pub no_sidecars: bool,
    /// `--quiet` — suppress per-step progress lines. The failure block
    /// and the final SUMMARY still print, so CI logs stay scannable
    /// without burying the actual failure under N progress lines.
    pub quiet: bool,
    /// `--plain` — force the plain, escape-code-free progress output even
    /// on a TTY. Mirrors what piped/CI output gets automatically (the
    /// runner already falls back to plain when stderr is not a terminal).
    /// Purely cosmetic; changes nothing about replay behaviour.
    pub plain: bool,
    /// `--tag <label>` — free-form label stored in `audit.tag`. Used
    /// for grouping runs across replays (e.g. 'pre-deploy', 'nightly',
    /// 'smoke'). Not enforced by anything; tooling can group/filter
    /// on it.
    pub tag: Option<String>,
    /// `--output-audit <path>` — also write the final audit.json to this
    /// additional path. The canonical copy still lives under
    /// <sid>/replays/<runId>/audit.json; this is for CI artifact upload
    /// or convenience pipelines.
    pub output_audit: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum ScenarioSource {
    Path(PathBuf),
    Sid(String),
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub passed: u32,
    pub total: u32,
    pub ok: bool,
}

impl RunSummary {
    pub fn render(&self) -> String {
        format!(
            "SUMMARY: {}/{} ({})",
            self.passed,
            self.total,
            if self.ok { "PASS" } else { "FAIL" }
        )
    }
}

// ---------- legible terminal (progress + failure pointer) ----------

/// How per-step progress is rendered to stderr, resolved once per run from
/// `--quiet`, `--plain`, and whether stderr is a TTY. Pure presentation —
/// it never changes replay behaviour, the event stream, or the SUMMARY
/// line (other tooling depends on those staying byte-stable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgressMode {
    /// `--quiet`: no per-step lines at all (the failure block still prints).
    Quiet,
    /// Non-TTY (piped/CI) or `--plain`: one durable ASCII line per finished
    /// step, no escape codes — logs stay clean and greppable.
    Plain,
    /// Interactive TTY: a live `…` line per step, overwritten in place by a
    /// `✓`/`✗` terminal line.
    Pretty,
}

fn resolve_progress_mode(opts: &RunOptions) -> ProgressMode {
    if opts.quiet {
        ProgressMode::Quiet
    } else if opts.plain || !std::io::stderr().is_terminal() {
        ProgressMode::Plain
    } else {
        ProgressMode::Pretty
    }
}

#[derive(Debug, Clone, Copy)]
enum StepState {
    Running,
    Pass,
    Fail,
}

/// `[ 5/12]` — right-aligned index, width derived from `total` so the
/// counter column stays fixed for the whole run.
fn fmt_counter(idx: u32, total: u32) -> String {
    let w = total.max(1).to_string().len();
    format!("[{idx:>w$}/{total}]")
}

/// Human-friendly step duration: `850ms` under a second, `1.2s` above.
fn fmt_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

/// One progress line's text (no carriage-return / no ANSI — the printer
/// adds those in pretty mode). Glyphs differ by mode so plain/CI logs
/// stay ASCII.
fn fmt_progress(
    mode: ProgressMode,
    idx: u32,
    total: u32,
    state: StepState,
    label: &str,
    ms: Option<u64>,
) -> String {
    let glyph = match (mode, state) {
        (ProgressMode::Pretty, StepState::Running) => "…",
        (ProgressMode::Pretty, StepState::Pass) => "✓",
        (ProgressMode::Pretty, StepState::Fail) => "✗",
        (_, StepState::Running) => "·",
        (_, StepState::Pass) => "PASS",
        (_, StepState::Fail) => "FAIL",
    };
    let dur = ms
        .map(|m| format!("  ({})", fmt_duration(m)))
        .unwrap_or_default();
    format!("{} {glyph} {label}{dur}", fmt_counter(idx, total))
}

/// Concise, vendor-neutral label for a step's progress line. Uses the
/// verb + targeted accessible-name when present (`click "Submit"`), and
/// falls back to the author's intent for untargeted verbs and checks.
fn progress_label(step: &Step) -> String {
    match step {
        Step::Do {
            verb, on, intent, ..
        } => {
            let v = format!("{verb:?}").to_ascii_lowercase();
            match locator_name(on.as_ref()) {
                Some(name) => format!("{v} \"{name}\""),
                None if !intent.trim().is_empty() => format!("{v} — {intent}"),
                None => v,
            }
        }
        Step::Check { intent, .. } => {
            if intent.trim().is_empty() {
                "check".to_string()
            } else {
                format!("check — {intent}")
            }
        }
    }
}

/// The accessible-name of a role locator (or its pattern / i18n key), used
/// only for the human progress label. Raw selectors are omitted to keep
/// the line legible.
fn locator_name(loc: Option<&Locator>) -> Option<String> {
    match loc? {
        Locator::Role(r) => match r.name.as_ref()? {
            NameMatch::Plain(s) => Some(s.clone()),
            NameMatch::Pattern { pattern, .. } => Some(pattern.clone()),
            NameMatch::I18n { i18n_key } => Some(i18n_key.clone()),
        },
        Locator::Raw(_) => None,
    }
}

/// Emit the in-flight `…` line for a step (pretty/TTY only). It is
/// overwritten in place by [`emit_step_done`]; plain and quiet modes show
/// nothing until the step finishes.
fn emit_step_start(mode: ProgressMode, idx: u32, total: u32, label: &str) {
    if mode == ProgressMode::Pretty {
        // `\r` returns to column 0; `\x1b[K` erases to end-of-line so a
        // shorter terminal line leaves no leftover characters behind.
        eprint!(
            "\r\x1b[K{}",
            fmt_progress(mode, idx, total, StepState::Running, label, None)
        );
        let _ = std::io::stderr().flush();
    }
}

/// Emit the terminal `✓`/`✗` (or `PASS`/`FAIL`) line for a finished step.
fn emit_step_done(mode: ProgressMode, idx: u32, total: u32, ok: bool, label: &str, ms: u64) {
    let state = if ok { StepState::Pass } else { StepState::Fail };
    match mode {
        ProgressMode::Quiet => {}
        ProgressMode::Pretty => eprintln!(
            "\r\x1b[K{}",
            fmt_progress(mode, idx, total, state, label, Some(ms))
        ),
        ProgressMode::Plain => {
            eprintln!("{}", fmt_progress(mode, idx, total, state, label, Some(ms)))
        }
    }
}

/// The fields of a failure pointer, grouped so the renderer stays under
/// the argument-count lint and the call site reads as named fields.
struct FailurePointer<'a> {
    idx: u32,
    total: u32,
    intent: &'a str,
    kind: &'a str,
    reason: &'a str,
    screenshot: Option<&'a str>,
    snapshot: Option<&'a str>,
    run_dir: &'a str,
}

/// The scannable failure block printed once at the first failing step.
/// Paths are absolute + copy-pasteable and follow the stable sidecar
/// convention in `docs/specs/scenario-sidecar-tree.md`.
fn render_failure_block(p: &FailurePointer) -> String {
    let shot = p.screenshot.unwrap_or("(not captured — --no-sidecars)");
    let snap = p.snapshot.unwrap_or("(not captured — --no-sidecars)");
    // Collapse the dispatch error to a single clean line so a stray
    // trailing newline (common from agent-browser stderr) doesn't punch a
    // blank gap into the block. The raw error is preserved verbatim on the
    // `fail` event row; this is display-only.
    let reason = p.reason.split_whitespace().collect::<Vec<_>>().join(" ");
    let FailurePointer {
        idx,
        total,
        intent,
        kind,
        run_dir,
        ..
    } = *p;
    format!(
        "\n✗ FAILED at step {idx}/{total}  \"{intent}\"  ({kind})\n  \
         reason:     {reason}\n  \
         screenshot: {shot}\n  \
         snapshot:   {snap}\n  \
         run dir:    {run_dir}"
    )
}

/// Make a path absolute + copy-pasteable for display. Prefers the
/// canonical form when the file exists (capture succeeded); otherwise
/// joins the cwd so the printed path is still absolute.
fn abs_display(p: &Path) -> String {
    if let Ok(abs) = std::fs::canonicalize(p) {
        return abs.display().to_string();
    }
    if p.is_absolute() {
        return p.display().to_string();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(p))
        .unwrap_or_else(|_| p.to_path_buf())
        .display()
        .to_string()
}

// ---------- entry point ----------

pub fn run(opts: &RunOptions) -> Result<RunSummary> {
    // 1. Load + validate.
    let (scenario_file, scenario_dir) = resolve_source(&opts.source)?;
    let bytes =
        fs::read(&scenario_file).with_context(|| format!("read {}", scenario_file.display()))?;
    let parsed = schema::validate_bytes(&bytes)
        .with_context(|| format!("validate {}", scenario_file.display()))?;
    let scenario: Scenario = serde_json::from_value(parsed.clone()).context("parse scenario")?;
    let hash = hash_scenario_bytes(&bytes);

    // 2. Mint run + prepare root.
    let run_id = mint_run_id(opts.profile.as_deref());
    eprintln!("[v2-replay] {} → run {}", scenario.id, run_id);
    let run = prepare_run_root(&scenario_dir, &run_id)?;

    // 3. Initial audit (startedAt + scenarioContentHash).
    let started_at = now_iso();
    let mut audit = RunAudit {
        schema: RunAudit::SCHEMA_ID.to_string(),
        run_id: run.run_id.clone(),
        scenario_id: scenario.id.clone(),
        started_at: started_at.clone(),
        finished_at: None,
        exit_code: None,
        summary: None,
        profile: opts.profile.clone(),
        session_name: Some(opts.session_name.clone()),
        scenario_content_hash: hash.clone(),
        parameters: None,
        heal_overrides_applied: None,
        tag: opts.tag.clone(),
    };
    write_run_audit(&run, &audit)?;

    // 4. Resolve inputs (declared defaults + --param overrides).
    let (resolved_inputs, audit_params) =
        resolve_inputs(scenario.inputs.as_ref(), &opts.input_overrides)?;
    audit.parameters = if audit_params.is_empty() {
        None
    } else {
        Some(audit_params)
    };

    // 5. env.open setup. Skipped under --dry-run.
    //
    // A setup failure (e.g. a `useProfile` op whose profile bootstrap can't
    // authenticate — no credentials in env) must still leave a TERMINAL run.
    // Before, `?` propagated here BEFORE the first status.json write, so the
    // run had only audit.json and the viewer showed it "in flight" forever (a
    // ghost). Now we finalise a done/failed status + audit first, so the run
    // surfaces as FAIL with the setup error, then propagate.
    let mut scope = ValueScope::new(resolved_inputs);
    if !opts.dry_run {
        if let Some(env) = &scenario.env {
            if let Some(open_ops) = &env.open {
                if let Err(e) =
                    env_ops::run_phase("env.open", open_ops, &opts.session_name, &mut scope)
                {
                    let reason = format!("env.open setup failed: {e}");
                    let _ = write_run_status(
                        &run,
                        &RunStatus {
                            state: "done".to_string(),
                            current_idx: 0,
                            total: 0,
                            ok: Some(false),
                        },
                    );
                    let _ = append_event(
                        &run,
                        &StepEvent {
                            idx: 0,
                            total: 0,
                            id: "env.open".to_string(),
                            intent: "setup".to_string(),
                            kind: "setup".to_string(),
                            status: "fail".to_string(),
                            ms: None,
                            error: Some(reason.clone()),
                            screenshot: None,
                            snapshot: None,
                        },
                    );
                    let summary_line = "SUMMARY: 0/0 (FAIL — setup)".to_string();
                    eprintln!("{summary_line}");
                    audit.finished_at = Some(now_iso());
                    audit.summary = Some(summary_line);
                    audit.exit_code = Some(1);
                    let _ = write_run_audit(&run, &audit);
                    let _ = update_latest_pointer(&scenario_dir, &run.run_id);
                    return Err(e.context("env.open setup failed"));
                }
            }
        }
    }

    // 6. Step loop (do dispatch via verbs::dispatch_do; check via
    //    claims::dispatch_check). After each step, capture per-step
    //    sidecars (ARIA snapshot + screenshot) keyed by literal stepId.
    //    Capture failures are non-fatal — logged to stderr, the step
    //    still counts as passed.
    let heal_overrides = if let Some(prior_run) = opts.heal_from_run.as_deref() {
        load_heal_overrides(&scenario_dir, prior_run)?
    } else {
        std::collections::HashMap::new()
    };
    let mut applied_overrides: Vec<String> = Vec::new();
    let mut summary = RunSummary {
        passed: 0,
        total: 0,
        ok: true,
    };
    let mut first_failure: Option<String> = None;

    if opts.dry_run {
        summary.total = scenario.steps.len() as u32;
        eprintln!(
            "[v2-replay] dry-run — {} step(s) parseable; skipping dispatch + env.close",
            summary.total
        );
    } else {
        let do_ctx = DoContext {
            session: &opts.session_name,
            scenario_dir: &scenario_dir,
        };
        let check_ctx = CheckContext {
            session: &opts.session_name,
        };
        // Flatten group/useTemplate steps inline so the iterator below
        // sees one dispatchable step per iteration. Loop is still not
        // expanded here — it bails inside dispatch_do.
        let flat: Vec<Step> = match flatten_steps_with_scope(
            &scenario.steps,
            scenario.templates.as_ref(),
            Some(&scope.inputs),
        ) {
            Ok(v) => v,
            Err(e) => {
                summary.ok = false;
                first_failure.get_or_insert(format!("flatten: {e}"));
                Vec::new()
            }
        };
        // Event stream: `total` is fixed once the run is flattened; the
        // live status file and the events stream both key off it. All of
        // this is additive — the per-step stderr trace below is unchanged.
        let total = flat.len() as u32;
        // Resolve once how per-step progress is rendered (quiet /
        // plain-or-piped / pretty-TTY). Pure formatting; no behaviour change.
        let progress_mode = resolve_progress_mode(opts);
        if let Err(e) = write_run_status(
            &run,
            &RunStatus {
                state: "running".to_string(),
                current_idx: 0,
                total,
                ok: None,
            },
        ) {
            eprintln!("[v2-replay] status.json init failed: {e}");
        }
        for step in &flat {
            summary.total += 1;
            let idx = summary.total;
            let id = step.id();
            let intent = step.intent();
            let kind_label = match step {
                Step::Do { verb, .. } => format!("do:{verb:?}").to_ascii_lowercase(),
                Step::Check { .. } => "check".to_string(),
            };
            // A concise, vendor-neutral label for the progress line.
            let label = progress_label(step);
            // Live progress — a `…` running line in pretty/TTY mode
            // (overwritten in place by the terminal line), nothing in
            // plain/quiet mode. Replaces the old flat `[v2-replay] step`
            // trace; `--quiet` still silences per-step output entirely.
            emit_step_start(progress_mode, idx, total, &label);
            // Mark this step in-flight in status.json and append a
            // `running` row to events.jsonl, then time the dispatch.
            let _ = write_run_status(
                &run,
                &RunStatus {
                    state: "running".to_string(),
                    current_idx: idx,
                    total,
                    ok: None,
                },
            );
            let _ = append_event(
                &run,
                &StepEvent {
                    idx,
                    total,
                    id: id.to_string(),
                    intent: intent.to_string(),
                    kind: kind_label.clone(),
                    status: "running".to_string(),
                    ms: None,
                    error: None,
                    screenshot: None,
                    snapshot: None,
                },
            );
            let step_started = std::time::Instant::now();
            // Apply heal-from-run override BEFORE dispatch, by mutating a
            // local copy of the step. The override carries the corrected
            // literal; we replace the step's value with
            // {from:'literal', literal:<corrected>}.
            let patched_step = if let Some(corrected) = heal_overrides.get(id) {
                applied_overrides.push(id.to_string());
                apply_heal_override(step, corrected)
            } else {
                step.clone()
            };
            let result = match &patched_step {
                Step::Do { save_as, .. } => match dispatch_do(&patched_step, &do_ctx, &mut scope) {
                    Ok(saved) => {
                        if let (Some(name), Some(value)) = (save_as.as_deref(), saved) {
                            scope.saved_steps.insert(name.to_string(), value);
                        }
                        Ok(())
                    }
                    Err(e) => Err(e),
                },
                Step::Check { claim, .. } => dispatch_check(claim, &check_ctx, &mut scope, None),
            };
            let step_ms = step_started.elapsed().as_millis() as u64;
            // When sidecars are enabled the runner captures a
            // screenshot + ARIA snapshot per step at stable, convention
            // paths; record those run-root-relative paths on the terminal
            // event so a consumer can find them by path without
            // re-deriving the sidecar tree convention.
            let (screenshot, snapshot) = if opts.no_sidecars {
                (None, None)
            } else {
                (
                    Some(format!("screenshots/{id}.png")),
                    Some(format!("snapshots/{id}.txt")),
                )
            };
            match result {
                Ok(()) => {
                    if !opts.no_sidecars {
                        capture_step_sidecars(&run, id, &opts.session_name);
                    }
                    summary.passed += 1;
                    emit_step_done(progress_mode, idx, total, true, &label, step_ms);
                    let _ = append_event(
                        &run,
                        &StepEvent {
                            idx,
                            total,
                            id: id.to_string(),
                            intent: intent.to_string(),
                            kind: kind_label.clone(),
                            status: "pass".to_string(),
                            ms: Some(step_ms),
                            error: None,
                            screenshot,
                            snapshot,
                        },
                    );
                }
                Err(e) => {
                    if !opts.no_sidecars {
                        capture_step_sidecars(&run, id, &opts.session_name);
                    }
                    summary.ok = false;
                    let reason = format!("{e:#}");
                    emit_step_done(progress_mode, idx, total, false, &label, step_ms);
                    // Failure pointer: a scannable end block with
                    // absolute, copy-pasteable paths to the captured
                    // screenshot/snapshot + the run dir. Printed in every
                    // mode (including --quiet) since it IS the failure
                    // output. Paths follow scenario-sidecar-tree.md.
                    let (shot_abs, snap_abs) = if opts.no_sidecars {
                        (None, None)
                    } else {
                        (
                            Some(abs_display(
                                &run.run_root.join("screenshots").join(format!("{id}.png")),
                            )),
                            Some(abs_display(
                                &run.run_root.join("snapshots").join(format!("{id}.txt")),
                            )),
                        )
                    };
                    eprintln!(
                        "{}",
                        render_failure_block(&FailurePointer {
                            idx,
                            total,
                            intent,
                            kind: &kind_label,
                            reason: &reason,
                            screenshot: shot_abs.as_deref(),
                            snapshot: snap_abs.as_deref(),
                            run_dir: &abs_display(&run.run_root),
                        })
                    );
                    let _ = append_event(
                        &run,
                        &StepEvent {
                            idx,
                            total,
                            id: id.to_string(),
                            intent: intent.to_string(),
                            kind: kind_label.clone(),
                            status: "fail".to_string(),
                            ms: Some(step_ms),
                            error: Some(reason),
                            screenshot,
                            snapshot,
                        },
                    );
                    first_failure.get_or_insert_with(|| format!("step {id}: {e}"));
                    break;
                }
            }
        }
        // The step phase is finished; finalise the live status with
        // the run verdict. env.close (teardown) runs after this and is
        // intentionally not counted as a step.
        if let Err(e) = write_run_status(
            &run,
            &RunStatus {
                state: "done".to_string(),
                current_idx: summary.total,
                total,
                ok: Some(summary.ok),
            },
        ) {
            eprintln!("[v2-replay] status.json finalise failed: {e}");
        }
    }

    // 7. env.close teardown — best effort. Skipped under --dry-run.
    if !opts.dry_run {
        if let Some(env) = &scenario.env {
            if let Some(close_ops) = &env.close {
                if let Err(e) =
                    env_ops::run_phase("env.close", close_ops, &opts.session_name, &mut scope)
                {
                    eprintln!("[v2-replay] env.close error (continuing): {e}");
                }
            }
        }
    }

    // 8. Summary + final audit. --dry-run reports DRY-RUN instead of
    //    PASS/FAIL since no step was executed.
    let summary_line = if opts.dry_run {
        format!("SUMMARY: 0/{} (DRY-RUN)", summary.total)
    } else {
        summary.render()
    };
    eprintln!("{summary_line}");
    audit.finished_at = Some(now_iso());
    audit.summary = Some(summary_line.clone());
    audit.exit_code = Some(if summary.ok { 0 } else { 1 });
    if opts.heal_from_run.is_some() {
        audit.heal_overrides_applied = Some(applied_overrides);
    }
    write_run_audit(&run, &audit)?;

    // 9.5. Optional --output-audit duplicate (atomic write).
    if let Some(out) = &opts.output_audit {
        let body = serde_json::to_vec_pretty(&audit)?;
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        crate::sidecar::atomic_write_file(out, &body)
            .with_context(|| format!("--output-audit write {}", out.display()))?;
    }

    // 9. Latest pointer.
    update_latest_pointer(&scenario_dir, &run.run_id)?;

    if let Some(msg) = first_failure {
        bail!(msg);
    }
    Ok(summary)
}

// ---------- helpers ----------

fn resolve_source(source: &ScenarioSource) -> Result<(PathBuf, PathBuf)> {
    match source {
        ScenarioSource::Path(p) => {
            let abs = if p.is_absolute() {
                p.clone()
            } else {
                std::env::current_dir()?.join(p)
            };
            let dir = abs
                .parent()
                .ok_or_else(|| anyhow!("path {} has no parent", abs.display()))?
                .to_path_buf();
            Ok((abs, dir))
        }
        ScenarioSource::Sid(sid) => {
            let dir = paths::scenario_dir(sid)?;
            let file = dir.join("scenario.json");
            if !file.is_file() {
                bail!("no scenario.json at {} (sid={sid:?})", file.display());
            }
            Ok((file, dir))
        }
    }
}

fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// Recursively expand `do/group` steps inline AND `do/useTemplate`
/// steps by inlining the scenario's `templates[<name>].steps[]`. Other
/// do verbs and all check steps pass through unchanged.
///
/// `params.steps[]` for a group must deserialise as `Vec<Step>`;
/// `params.template` for useTemplate must name a key in
/// `templates`. Bad shape errors with the step's id in the message.
fn flatten_steps(
    steps: &[Step],
    templates: Option<&std::collections::BTreeMap<String, crate::scenario::Template>>,
) -> Result<Vec<Step>> {
    flatten_steps_with_scope(steps, templates, None)
}

fn flatten_steps_with_scope(
    steps: &[Step],
    templates: Option<&std::collections::BTreeMap<String, crate::scenario::Template>>,
    inputs: Option<&std::collections::HashMap<String, serde_json::Value>>,
) -> Result<Vec<Step>> {
    let mut out: Vec<Step> = Vec::with_capacity(steps.len());
    for step in steps {
        match step {
            Step::Do {
                id,
                verb: crate::scenario::Verb::Group,
                params,
                ..
            } => {
                let raw = params
                    .as_ref()
                    .and_then(|p| p.get("steps"))
                    .ok_or_else(|| anyhow!("step '{id}' verb=group requires params.steps[]"))?;
                let arr = raw.as_array().ok_or_else(|| {
                    anyhow!("step '{id}' verb=group: params.steps must be an array")
                })?;
                let mut subs: Vec<Step> = Vec::with_capacity(arr.len());
                for (idx, child) in arr.iter().enumerate() {
                    let parsed: Step =
                        serde_json::from_value(child.clone()).with_context(|| {
                            format!("step '{id}' verb=group: params.steps[{idx}] failed to parse")
                        })?;
                    subs.push(parsed);
                }
                for sub in flatten_steps_with_scope(&subs, templates, inputs)? {
                    out.push(sub);
                }
            }
            Step::Do {
                id,
                verb: crate::scenario::Verb::UseTemplate,
                params,
                ..
            } => {
                let name = params
                    .as_ref()
                    .and_then(|p| p.get("template"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow!("step '{id}' verb=useTemplate requires params.template (string)")
                    })?;
                let templates = templates.ok_or_else(|| {
                    anyhow!("step '{id}' verb=useTemplate: scenario carries no templates")
                })?;
                let template = templates.get(name).ok_or_else(|| {
                    anyhow!(
                        "step '{id}' verb=useTemplate: template {name:?} not found (declared: {:?})",
                        templates.keys().collect::<Vec<_>>()
                    )
                })?;
                let nested_templates = template.templates.as_ref().or(Some(templates));
                for sub in flatten_steps_with_scope(&template.steps, nested_templates, inputs)? {
                    out.push(sub);
                }
            }
            Step::Do {
                id,
                verb: crate::scenario::Verb::Loop,
                params,
                ..
            } => {
                let p = params
                    .as_ref()
                    .ok_or_else(|| anyhow!("step '{id}' verb=loop requires params"))?;
                let over_raw = p
                    .get("over")
                    .ok_or_else(|| anyhow!("step '{id}' verb=loop: params.over is required"))?;
                let as_name = p
                    .get("as")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("step '{id}' verb=loop: params.as is required"))?;
                let do_arr = p
                    .get("do")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| anyhow!("step '{id}' verb=loop: params.do[] is required"))?;
                let over_value: crate::scenario::Value = serde_json::from_value(over_raw.clone())
                    .with_context(|| {
                    format!("step '{id}' verb=loop: params.over not a Value")
                })?;
                let items: Vec<serde_json::Value> = match over_value {
                    crate::scenario::Value::Literal { literal } => {
                        let arr = literal.as_array().ok_or_else(|| {
                            anyhow!("step '{id}' verb=loop: params.over.literal must be an array")
                        })?;
                        arr.clone()
                    }
                    crate::scenario::Value::Input { input } => {
                        let scope = inputs.ok_or_else(|| {
                            anyhow!("step '{id}' verb=loop: params.over=input requires resolved inputs")
                        })?;
                        let v = scope.get(&input).ok_or_else(|| {
                            anyhow!(
                                "step '{id}' verb=loop: input {input:?} not in resolved scope"
                            )
                        })?;
                        let arr = v.as_array().ok_or_else(|| {
                            anyhow!(
                                "step '{id}' verb=loop: input {input:?} must resolve to an array"
                            )
                        })?;
                        arr.clone()
                    }
                    other => bail!(
                        "step '{id}' verb=loop: params.over.from {:?} not yet supported (use literal or input)",
                        match other {
                            crate::scenario::Value::Step { .. } => "step",
                            crate::scenario::Value::Mint { .. } => "mint",
                            crate::scenario::Value::Loop { .. } => "loop",
                            _ => "?",
                        }
                    ),
                };
                // For each item, clone the inner steps and substitute
                // {{vars.<as>}} placeholders with the item's value.
                // Recurse so nested groups / templates / loops within
                // the inner steps also expand.
                let mut subs: Vec<Step> = Vec::with_capacity(do_arr.len() * items.len());
                for item in &items {
                    let item_str = match item {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        other => serde_json::to_string(other).unwrap_or_default(),
                    };
                    for (idx, child) in do_arr.iter().enumerate() {
                        let serialised = serde_json::to_string(child).unwrap_or_default();
                        let placeholder = format!("{{{{vars.{as_name}}}}}");
                        let substituted = serialised.replace(&placeholder, &item_str);
                        let parsed: Step = serde_json::from_str(&substituted).with_context(|| {
                            format!("step '{id}' verb=loop: params.do[{idx}] failed to parse after substitution")
                        })?;
                        subs.push(parsed);
                    }
                }
                for sub in flatten_steps_with_scope(&subs, templates, inputs)? {
                    out.push(sub);
                }
            }
            other => out.push(other.clone()),
        }
    }
    Ok(out)
}

/// Backward-compat alias retained for older test sites; prefer
/// [`flatten_steps`] in new code.
#[cfg(test)]
fn flatten_groups(steps: &[Step]) -> Result<Vec<Step>> {
    flatten_steps(steps, None)
}

/// Load heal overrides from a prior run's `heal-responses/` directory.
/// Each `<stepId>.json` carrying `mode: 'value-correction'` and a `value`
/// becomes a `(stepId -> corrected-literal)` entry. `reject` and
/// `<stepId>.applied.json` markers are skipped (the latter signals the
/// override was already consumed by `heal-apply` against the buffer).
fn load_heal_overrides(
    scenario_dir: &std::path::Path,
    run_id: &str,
) -> Result<std::collections::HashMap<String, String>> {
    let dir = scenario_dir
        .join("replays")
        .join(run_id)
        .join("heal-responses");
    let mut out: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(it) => it,
        Err(_) => return Ok(out),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        // Skip already-applied markers.
        if name.ends_with(".applied.json") {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let body = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mode = v.get("mode").and_then(|m| m.as_str()).unwrap_or("");
        if mode != "value-correction" {
            continue;
        }
        let step_id = match v.get("stepId").and_then(|s| s.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let value = match v.get("value").and_then(|s| s.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        out.insert(step_id, value);
    }
    Ok(out)
}

/// Resolve declared inputs against `--param` overrides + declared
/// defaults. Returns `(scope.inputs, audit.parameters)`.
///
/// Resolution per declared input:
///   1. CLI override (`overrides[name]`) present → coerce to declared
///      type, audit source = `Cli`
///   2. else declared default present → use as-is, audit source = `Default`
///   3. else: omitted from both (the scenario is allowed to ship without
///      a default if no step references the input)
///
/// Unknown override names are rejected with a clear error.
/// Sensitive entries store `[REDACTED]` in audit.parameters but
/// flow the real value into `scope.inputs`.
fn resolve_inputs(
    declared: Option<&BTreeMap<String, InputDecl>>,
    overrides: &BTreeMap<String, String>,
) -> Result<(
    std::collections::HashMap<String, serde_json::Value>,
    Vec<RunAuditParameter>,
)> {
    let declared = match declared {
        Some(d) => d,
        None => {
            if let Some((name, _)) = overrides.iter().next() {
                bail!(
                    "--param {name:?} given but the scenario declares no inputs; remove the override or add an inputs entry"
                );
            }
            return Ok((std::collections::HashMap::new(), Vec::new()));
        }
    };

    for name in overrides.keys() {
        if !declared.contains_key(name) {
            bail!(
                "--param {name:?} is not declared in scenario.inputs; declared inputs: {:?}",
                declared.keys().collect::<Vec<_>>()
            );
        }
    }

    let mut scope_inputs: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    let mut audit_params: Vec<RunAuditParameter> = Vec::new();
    for (name, decl) in declared {
        let (value, source) = match overrides.get(name) {
            Some(raw) => (
                coerce_input(raw, decl.ty).with_context(|| format!("--param {name}"))?,
                ParameterSource::Cli,
            ),
            None => match &decl.default {
                Some(v) => (v.clone(), ParameterSource::Default),
                None => continue,
            },
        };
        let sensitive = decl.sensitive.unwrap_or(false);
        let audit_value = if sensitive {
            serde_json::Value::String("[REDACTED]".to_string())
        } else {
            value.clone()
        };
        audit_params.push(RunAuditParameter {
            name: name.clone(),
            ty: map_input_type(decl.ty),
            sensitive,
            value: audit_value,
            source,
        });
        scope_inputs.insert(name.clone(), value);
    }
    Ok((scope_inputs, audit_params))
}

fn map_input_type(t: InputType) -> AuditInputType {
    match t {
        InputType::String => AuditInputType::String,
        InputType::Number => AuditInputType::Number,
        InputType::Boolean => AuditInputType::Boolean,
        InputType::Array => AuditInputType::Array,
        InputType::Object => AuditInputType::Object,
    }
}

/// Coerce a raw `--param` value into the declared input type.
/// Strings pass through verbatim; numbers / booleans / arrays /
/// objects parse the raw as JSON first.
fn coerce_input(raw: &str, ty: InputType) -> Result<serde_json::Value> {
    match ty {
        InputType::String => Ok(serde_json::Value::String(raw.to_string())),
        InputType::Number => {
            serde_json::from_str(raw).map_err(|e| anyhow!("could not parse as number {raw:?}: {e}"))
        }
        InputType::Boolean => match raw {
            "true" => Ok(serde_json::Value::Bool(true)),
            "false" => Ok(serde_json::Value::Bool(false)),
            other => bail!("boolean must be 'true' or 'false', got {other:?}"),
        },
        InputType::Array => {
            serde_json::from_str(raw).map_err(|e| anyhow!("could not parse as array {raw:?}: {e}"))
        }
        InputType::Object => {
            serde_json::from_str(raw).map_err(|e| anyhow!("could not parse as object {raw:?}: {e}"))
        }
    }
}

/// Build a heal-patched copy of a do-step: replace `value` with
/// `{from: 'literal', literal: <corrected>}`. Check steps and
/// non-do shapes pass through unchanged — a heal override on a check
/// step is a no-op (we only patch values consumed by do verbs).
fn apply_heal_override(step: &Step, corrected: &str) -> Step {
    match step.clone() {
        Step::Do {
            id,
            intent,
            verb,
            on,
            value: _,
            save_as,
            params,
            context,
        } => Step::Do {
            id,
            intent,
            verb,
            on,
            value: Some(crate::scenario::Value::Literal {
                literal: serde_json::Value::String(corrected.to_string()),
            }),
            save_as,
            params,
            context,
        },
        other => other,
    }
}

/// Capture the post-step ARIA snapshot + screenshot keyed by literal
/// stepId. Best-effort: any failure logs to stderr but does not fail
/// the step. Mirrors the TS runner's sidecar-after-each-step shape.
fn capture_step_sidecars(run: &crate::sidecar::RunPaths, step_id: &str, session: &str) {
    use crate::sidecar::{ensure_kind_dir, step_sidecar_path, write_step_sidecar, SidecarKind};
    if !is_safe_step_id(step_id) {
        eprintln!("[v2-replay] skip sidecars for unsafe stepId {step_id:?}");
        return;
    }
    // Let the page settle after the step's action before capturing, so a
    // navigating click / async render is reflected in the screenshot + ARIA
    // snapshot rather than a half-loaded frame. Soft-fail: a page that is
    // already idle (or a networkidle timeout on a chatty page) must never
    // fail the run — this only governs artifact fidelity.
    let _ = browser::wait_for_load(session, "networkidle");
    match browser::snapshot_full(session) {
        Ok(text) => {
            if let Err(e) =
                write_step_sidecar(run, SidecarKind::Snapshots, step_id, text.as_bytes())
            {
                eprintln!("[v2-replay] snapshot sidecar failed for {step_id}: {e}");
            }
        }
        Err(e) => eprintln!("[v2-replay] snapshot {step_id} failed: {e}"),
    }
    if let Ok(dir) = ensure_kind_dir(run, SidecarKind::Screenshots) {
        let _ = dir; // keep the directory creation eager
    }
    if let Ok(path) = step_sidecar_path(run, SidecarKind::Screenshots, step_id) {
        match browser::screenshot(session, &path, true) {
            Ok(true) => {}
            Ok(false) => eprintln!("[v2-replay] screenshot {step_id} returned non-zero (lenient)"),
            Err(e) => eprintln!("[v2-replay] screenshot {step_id} failed: {e}"),
        }
    }
}

fn is_safe_step_id(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

// ---------- CLI verb ----------

pub fn cli(args: &[String]) -> Result<u8> {
    let (parsed, runs) = parse_args_with_runs(args)?;
    if runs <= 1 {
        let summary = run(&parsed)?;
        return Ok(if summary.ok { 0 } else { 1 });
    }
    let mut all_ok = true;
    for i in 1..=runs {
        eprintln!("[v2-replay] run {i}/{runs}");
        match run(&parsed) {
            Ok(summary) => {
                if !summary.ok {
                    all_ok = false;
                }
            }
            Err(e) => {
                eprintln!("[v2-replay] run {i}/{runs} errored: {e}");
                all_ok = false;
            }
        }
    }
    Ok(if all_ok { 0 } else { 1 })
}

/// Wrapper around [`parse_args`] that also peels off `--runs N` (a CLI
/// loop count) before resolution. Defaults to 1.
fn parse_args_with_runs(args: &[String]) -> Result<(RunOptions, u32)> {
    let mut filtered: Vec<String> = Vec::with_capacity(args.len());
    let mut runs: u32 = 1;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--runs" => {
                let n = it
                    .next()
                    .ok_or_else(|| anyhow!("--runs requires a positive integer"))?;
                runs = n
                    .parse::<u32>()
                    .map_err(|_| anyhow!("--runs must be a positive integer; got {n:?}"))?;
                if runs == 0 {
                    bail!("--runs must be >= 1");
                }
            }
            s if s.starts_with("--runs=") => {
                let n = &s["--runs=".len()..];
                runs = n
                    .parse::<u32>()
                    .map_err(|_| anyhow!("--runs must be a positive integer; got {n:?}"))?;
                if runs == 0 {
                    bail!("--runs must be >= 1");
                }
            }
            other => filtered.push(other.to_string()),
        }
    }
    let opts = parse_args(&filtered)?;
    Ok((opts, runs))
}

fn parse_args(args: &[String]) -> Result<RunOptions> {
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        eprintln!("{}", help_text());
        std::process::exit(0);
    }
    let mut positional: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut session: Option<String> = None;
    let mut heal_from_run: Option<String> = None;
    let mut dry_run = false;
    let mut no_sidecars = false;
    let mut quiet = false;
    let mut plain = false;
    let mut tag: Option<String> = None;
    let mut output_audit: Option<PathBuf> = None;
    let mut input_overrides: BTreeMap<String, String> = BTreeMap::new();
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--profile" => profile = it.next().cloned().or_else(|| bail_missing("--profile")),
            s if s.starts_with("--profile=") => profile = Some(s["--profile=".len()..].to_string()),
            "--session" => session = it.next().cloned().or_else(|| bail_missing("--session")),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            "--heal-from-run" => {
                heal_from_run = it
                    .next()
                    .cloned()
                    .or_else(|| bail_missing("--heal-from-run"))
            }
            s if s.starts_with("--heal-from-run=") => {
                heal_from_run = Some(s["--heal-from-run=".len()..].to_string())
            }
            "--dry-run" => dry_run = true,
            "--no-sidecars" => no_sidecars = true,
            "--quiet" | "-q" => quiet = true,
            "--plain" => plain = true,
            "--tag" => tag = it.next().cloned().or_else(|| bail_missing("--tag")),
            s if s.starts_with("--tag=") => tag = Some(s["--tag=".len()..].to_string()),
            "--output-audit" => {
                output_audit = it
                    .next()
                    .map(PathBuf::from)
                    .or_else(|| bail_missing("--output-audit").map(PathBuf::from))
            }
            s if s.starts_with("--output-audit=") => {
                output_audit = Some(PathBuf::from(&s["--output-audit=".len()..]))
            }
            "--param" | "-p" => {
                let pair = it
                    .next()
                    .ok_or_else(|| anyhow!("--param requires name=value"))?;
                let (k, v) = pair
                    .split_once('=')
                    .ok_or_else(|| anyhow!("--param expects name=value, got {pair:?}"))?;
                input_overrides.insert(k.to_string(), v.to_string());
            }
            s if s.starts_with("--param=") => {
                let pair = &s["--param=".len()..];
                let (k, v) = pair
                    .split_once('=')
                    .ok_or_else(|| anyhow!("--param= expects name=value, got {pair:?}"))?;
                input_overrides.insert(k.to_string(), v.to_string());
            }
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if positional.is_some() {
                    bail!("unexpected positional {other:?}; usage: replay <sid|path>");
                }
                positional = Some(other.to_string());
            }
        }
    }
    let target =
        positional.ok_or_else(|| anyhow!("usage: replay <sid | path/to/scenario.json>"))?;
    let source = if looks_like_path(&target) {
        ScenarioSource::Path(PathBuf::from(&target))
    } else {
        ScenarioSource::Sid(target)
    };
    let session_name = session.unwrap_or_else(|| {
        profile
            .as_ref()
            .map(|p| format!("{p}-session"))
            .unwrap_or_else(|| DEFAULT_SESSION.to_string())
    });
    Ok(RunOptions {
        source,
        profile,
        session_name,
        heal_from_run,
        input_overrides,
        dry_run,
        no_sidecars,
        quiet,
        plain,
        tag,
        output_audit,
    })
}

fn bail_missing(flag: &str) -> Option<String> {
    eprintln!("agent-qa replay: {flag} requires an argument");
    std::process::exit(2)
}

fn looks_like_path(s: &str) -> bool {
    s.contains('/') || s.ends_with(".json") || Path::new(s).is_file()
}

fn help_text() -> &'static str {
    "agent-qa replay — re-execute a scenario/2 document end-to-end

Usage:
  agent-qa replay <sid | path/to/scenario.json>
                  [--profile <p>] [--session <name>]
                  [--param name=value] [-p name=value]
                  [--heal-from-run <runId>] [--dry-run]
                  [--no-sidecars] [--quiet | -q] [--plain]
                  [--tag <label>] [--output-audit <path>]
                  [--runs <N>]

Loads + validates the scenario, mints a run id, prepares
<sid>/replays/<runId>/, writes audit.json, runs env.open, iterates
steps (do-verb + check-claim dispatch + per-step ARIA snapshot +
screenshot sidecars), runs env.close, prints SUMMARY: N/M, updates
replays/latest.txt.

--heal-from-run <runId>  pre-load heal-responses from a prior run
                         and override step values at dispatch time.
                         The resulting audit.json carries
                         healOverridesApplied[].
--param name=value, -p   Override a declared input value. Coerced to
                         the declared type (string/number/boolean/
                         array/object); JSON for non-string types.
                         Duplicate name is last-wins. Recorded in
                         audit.parameters[] (sensitive=true →
                         [REDACTED]).
--dry-run                load + validate + mint run id + write the
                         initial audit row, but skip env.open /
                         env.close and step dispatch entirely. The
                         run directory still gets created and
                         audit.json carries 'SUMMARY: 0/N (DRY-RUN)'.
--runs <N>               Repeat the replay N times in one invocation
                         (each run mints its own runId). Useful for
                         flake detection. Exit 0 iff every run is OK.
--no-sidecars            Skip per-step ARIA snapshot + screenshot
                         capture. audit.json is still written. Useful
                         when running with --runs N.
--quiet, -q              Suppress per-step progress lines. The failure
                         block and the final SUMMARY still print, so CI
                         logs stay scannable.
--plain                  Force plain, escape-code-free per-step output
                         even on a TTY (the default on a non-TTY/pipe).
                         Use when the live in-place '…' progress confuses
                         a wrapping tool.
--tag <label>            Free-form label stamped into audit.tag.
                         Useful for grouping runs across replays
                         (e.g. 'pre-deploy', 'nightly', 'smoke').
--output-audit <path>    Also write the final audit.json to this
                         additional path. The canonical copy still
                         lives under <sid>/replays/<runId>/audit.json;
                         this is for CI artifact upload or pipeline
                         convenience."
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::env;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    // env mutation isn't thread-safe; serialize via the shared lock.
    use crate::test_util::lock_env;

    fn write_exec(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    fn install_fake_browser(dir: &Path, log: &Path) -> PathBuf {
        let body = format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display());
        let bin = write_exec(dir, "agent-browser", &body);
        env::set_var(browser::BIN_ENV, &bin);
        browser::_reset_bin_cache_for_tests();
        bin
    }

    fn clear_fake_browser() {
        env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    fn minimal_scenario() -> &'static str {
        r#"{
            "schema": "scenario/2",
            "id": "smoke",
            "intent": "open the home page",
            "env": {
                "open": [
                    { "kind": "nav", "url": "https://example.com/", "intent": "land" }
                ]
            },
            "steps": [
                { "id": "s1", "intent": "noop check", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
            ]
        }"#
    }

    #[test]
    fn replay_writes_audit_and_latest_pointer() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        install_fake_browser(work.path(), &log);

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, minimal_scenario()).unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile.clone()),
            profile: None,
            session_name: "test".into(),
            heal_from_run: None,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
        };
        let summary = run(&opts).unwrap();
        assert_eq!(summary.total, 1);
        assert_eq!(summary.passed, 1);
        assert!(summary.ok);

        // latest.txt points at the minted run id.
        let latest = fs::read_to_string(jdir.join("replays").join("latest.txt")).unwrap();
        let run_id = latest.trim().to_string();
        assert!(!run_id.is_empty());

        // audit.json carries finishedAt + summary + scenarioContentHash.
        let audit_path = jdir.join("replays").join(&run_id).join("audit.json");
        let audit: serde_json::Value =
            serde_json::from_slice(&fs::read(audit_path).unwrap()).unwrap();
        assert_eq!(audit["runId"], run_id.as_str());
        assert_eq!(audit["scenarioId"], "smoke");
        assert!(audit["finishedAt"].is_string());
        assert!(audit["scenarioContentHash"].is_string());
        assert_eq!(audit["summary"], "SUMMARY: 1/1 (PASS)");
        assert_eq!(audit["exitCode"], 0);

        // Fake agent-browser invoked once for env.open nav.
        let ab = fs::read_to_string(&log).unwrap();
        assert!(
            ab.contains("--session test open https://example.com/"),
            "got: {ab}"
        );
        clear_fake_browser();
    }

    #[test]
    fn replay_invalid_scenario_errors_with_schema_messages() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        install_fake_browser(work.path(), &work.path().join("ab.log"));

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, r#"{ "schema": "scenario/2", "id": "x" }"#).unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "x".into(),
            heal_from_run: None,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
        };
        let err = format!("{:#}", run(&opts).unwrap_err());
        assert!(err.contains("schema error"), "got: {err}");
        clear_fake_browser();
    }

    #[test]
    fn replay_writes_per_step_sidecars() {
        // Per-step ARIA snapshot + screenshot land under
        // <run>/snapshots/<stepId>.txt and <run>/screenshots/<stepId>.png.
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        // Fake script that echoes args AND creates the requested
        // screenshot output file (mirrors what a real agent-browser
        // screenshot does).
        let body = format!(
            "#!/bin/sh\necho \"$@\" >> '{log}'\n\
if [ \"$3\" = 'screenshot' ]; then\n  shift 3\n  [ \"$1\" = '--full' ] && shift\n  : > \"$1\"\nfi\nexit 0\n",
            log = log.display()
        );
        let bin = write_exec(work.path(), "agent-browser", &body);
        std::env::set_var(crate::browser::BIN_ENV, &bin);
        crate::browser::_reset_bin_cache_for_tests();

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, minimal_scenario()).unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "sx".into(),
            heal_from_run: None,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
        };
        let summary = run(&opts).unwrap();
        assert!(summary.ok);

        let latest = fs::read_to_string(jdir.join("replays").join("latest.txt")).unwrap();
        let run_id = latest.trim();
        let snap = jdir
            .join("replays")
            .join(run_id)
            .join("snapshots")
            .join("s1.txt");
        let shot = jdir
            .join("replays")
            .join(run_id)
            .join("screenshots")
            .join("s1.png");
        assert!(snap.is_file(), "expected snapshot at {}", snap.display());
        assert!(shot.is_file(), "expected screenshot at {}", shot.display());

        std::env::remove_var(crate::browser::BIN_ENV);
        crate::browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn replay_env_cookie_now_runs_via_eval() {
        // Verify a scenario with a cookie env op completes
        // and invokes the fake agent-browser.
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        install_fake_browser(work.path(), &log);

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(
            &jfile,
            r#"{
                "schema": "scenario/2", "id": "x", "intent": "y",
                "env": { "open": [{ "kind": "cookie", "name": "c", "value": "v" }] },
                "steps": []
            }"#,
        )
        .unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "x".into(),
            heal_from_run: None,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
        };
        let summary = run(&opts).unwrap();
        assert!(summary.ok);
        let lines = fs::read_to_string(&log).unwrap();
        assert!(
            lines.contains("document.cookie"),
            "expected cookie eval, got: {lines}"
        );
        clear_fake_browser();
    }

    #[test]
    fn parse_args_param_overrides_collect() {
        let opts = parse_args(&[
            "./j.json".into(),
            "--param".into(),
            "name=alice".into(),
            "--param=count=3".into(),
            "-p".into(),
            "flag=true".into(),
        ])
        .unwrap();
        assert_eq!(opts.input_overrides.get("name").unwrap(), "alice");
        assert_eq!(opts.input_overrides.get("count").unwrap(), "3");
        assert_eq!(opts.input_overrides.get("flag").unwrap(), "true");
    }

    #[test]
    fn parse_args_param_rejects_missing_eq() {
        let err = parse_args(&["./j.json".into(), "--param".into(), "justname".into()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("name=value"));
    }

    #[test]
    fn resolve_inputs_default_and_override_redacts_sensitive() {
        let mut declared = BTreeMap::new();
        declared.insert(
            "name".to_string(),
            InputDecl {
                ty: InputType::String,
                default: Some(serde_json::json!("bob")),
                sensitive: None,
                items: None,
                properties: None,
                description: None,
            },
        );
        declared.insert(
            "secret".to_string(),
            InputDecl {
                ty: InputType::String,
                default: None,
                sensitive: Some(true),
                items: None,
                properties: None,
                description: None,
            },
        );
        let mut overrides = BTreeMap::new();
        overrides.insert("secret".to_string(), "hunter2".to_string());
        let (scope, params) = resolve_inputs(Some(&declared), &overrides).unwrap();
        assert_eq!(scope.get("name").unwrap(), &serde_json::json!("bob"));
        assert_eq!(scope.get("secret").unwrap(), &serde_json::json!("hunter2"));
        let secret_param = params.iter().find(|p| p.name == "secret").unwrap();
        assert_eq!(secret_param.value, serde_json::json!("[REDACTED]"));
    }

    #[test]
    fn resolve_inputs_rejects_undeclared_override() {
        let declared: BTreeMap<String, InputDecl> = BTreeMap::new();
        let mut overrides = BTreeMap::new();
        overrides.insert("nope".to_string(), "x".to_string());
        let err = resolve_inputs(Some(&declared), &overrides)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not declared") || err.contains("no inputs"));
    }

    #[test]
    fn coerce_input_types() {
        assert_eq!(
            coerce_input("true", InputType::Boolean).unwrap(),
            serde_json::json!(true)
        );
        coerce_input("yes", InputType::Boolean).unwrap_err();
        assert_eq!(
            coerce_input("42", InputType::Number).unwrap(),
            serde_json::json!(42)
        );
        assert_eq!(
            coerce_input(r#"[1,2]"#, InputType::Array).unwrap(),
            serde_json::json!([1, 2])
        );
        assert_eq!(
            coerce_input(r#"{"k":1}"#, InputType::Object).unwrap(),
            serde_json::json!({ "k": 1 })
        );
    }

    #[test]
    fn parse_args_with_runs_strips_and_parses() {
        let (opts, runs) =
            parse_args_with_runs(&["./j.json".into(), "--runs".into(), "5".into()]).unwrap();
        assert_eq!(runs, 5);
        assert!(matches!(opts.source, ScenarioSource::Path(_)));
    }

    #[test]
    fn parse_args_with_runs_eq_form() {
        let (_opts, runs) = parse_args_with_runs(&["./j.json".into(), "--runs=3".into()]).unwrap();
        assert_eq!(runs, 3);
    }

    #[test]
    fn parse_args_with_runs_default_is_one() {
        let (_opts, runs) = parse_args_with_runs(&["./j.json".into()]).unwrap();
        assert_eq!(runs, 1);
    }

    #[test]
    fn parse_args_with_runs_rejects_zero_and_non_int() {
        parse_args_with_runs(&["./j.json".into(), "--runs".into(), "0".into()]).unwrap_err();
        parse_args_with_runs(&["./j.json".into(), "--runs".into(), "x".into()]).unwrap_err();
    }

    #[test]
    fn flatten_groups_inlines_subdo_steps() {
        let body = serde_json::json!([
            { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
            {
                "id": "g1",
                "intent": "group",
                "kind": "do",
                "verb": "group",
                "params": { "steps": [
                    { "id": "sg1a", "intent": "a", "kind": "do", "verb": "reload" },
                    { "id": "sg1b", "intent": "b", "kind": "do", "verb": "reload" }
                ]}
            },
            { "id": "s2", "intent": "after", "kind": "do", "verb": "reload" }
        ]);
        let steps: Vec<Step> = serde_json::from_value(body).unwrap();
        let flat = flatten_groups(&steps).unwrap();
        let ids: Vec<&str> = flat.iter().map(|s| s.id()).collect();
        assert_eq!(ids, vec!["s0", "sg1a", "sg1b", "s2"]);
    }

    #[test]
    fn flatten_groups_recurses_through_nested_groups() {
        let body = serde_json::json!([
            {
                "id": "outer", "intent": "x", "kind": "do", "verb": "group",
                "params": { "steps": [
                    {
                        "id": "inner", "intent": "y", "kind": "do", "verb": "group",
                        "params": { "steps": [
                            { "id": "leaf", "intent": "z", "kind": "do", "verb": "reload" }
                        ]}
                    }
                ]}
            }
        ]);
        let steps: Vec<Step> = serde_json::from_value(body).unwrap();
        let flat = flatten_groups(&steps).unwrap();
        let ids: Vec<&str> = flat.iter().map(|s| s.id()).collect();
        assert_eq!(ids, vec!["leaf"]);
    }

    #[test]
    fn flatten_groups_errors_on_missing_steps_param() {
        let body = serde_json::json!([
            { "id": "g1", "intent": "x", "kind": "do", "verb": "group" }
        ]);
        let steps: Vec<Step> = serde_json::from_value(body).unwrap();
        let err = flatten_groups(&steps).unwrap_err().to_string();
        assert!(err.contains("params.steps"));
    }

    #[test]
    fn flatten_steps_inlines_use_template() {
        use crate::scenario::Template;
        let body = serde_json::json!([
            { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
            { "id": "u1", "intent": "use t", "kind": "do", "verb": "useTemplate",
              "params": { "template": "my-template" } },
            { "id": "s2", "intent": "after", "kind": "do", "verb": "reload" }
        ]);
        let steps: Vec<Step> = serde_json::from_value(body).unwrap();
        let template_body = serde_json::json!({
            "steps": [
                { "id": "t1", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "t2", "intent": "b", "kind": "do", "verb": "reload" }
            ]
        });
        let template: Template = serde_json::from_value(template_body).unwrap();
        let mut templates = std::collections::BTreeMap::new();
        templates.insert("my-template".to_string(), template);
        let flat = flatten_steps(&steps, Some(&templates)).unwrap();
        let ids: Vec<&str> = flat.iter().map(|s| s.id()).collect();
        assert_eq!(ids, vec!["s0", "t1", "t2", "s2"]);
    }

    #[test]
    fn flatten_steps_use_template_unknown_errors() {
        let body = serde_json::json!([
            { "id": "u1", "intent": "x", "kind": "do", "verb": "useTemplate",
              "params": { "template": "missing" } }
        ]);
        let steps: Vec<Step> = serde_json::from_value(body).unwrap();
        let templates: std::collections::BTreeMap<String, crate::scenario::Template> =
            std::collections::BTreeMap::new();
        let err = flatten_steps(&steps, Some(&templates))
            .unwrap_err()
            .to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn flatten_steps_use_template_without_scenario_templates_errors() {
        let body = serde_json::json!([
            { "id": "u1", "intent": "x", "kind": "do", "verb": "useTemplate",
              "params": { "template": "x" } }
        ]);
        let steps: Vec<Step> = serde_json::from_value(body).unwrap();
        let err = flatten_steps(&steps, None).unwrap_err().to_string();
        assert!(err.contains("no templates"));
    }

    #[test]
    fn flatten_steps_loop_literal_array_substitutes_vars() {
        let body = serde_json::json!([
            { "id": "loop1", "intent": "x", "kind": "do", "verb": "loop",
              "params": {
                "over": { "from": "literal", "literal": ["alice", "bob"] },
                "as": "who",
                "do": [
                  { "id": "greet", "intent": "greet {{vars.who}}", "kind": "do", "verb": "reload" }
                ]
              } }
        ]);
        let steps: Vec<Step> = serde_json::from_value(body).unwrap();
        let flat = flatten_steps(&steps, None).unwrap();
        // Two iterations × one substep → 2 flat steps.
        assert_eq!(flat.len(), 2);
        let intents: Vec<&str> = flat.iter().map(|s| s.intent()).collect();
        assert_eq!(intents, vec!["greet alice", "greet bob"]);
    }

    #[test]
    fn flatten_steps_loop_input_array_substitutes_vars() {
        let body = serde_json::json!([
            { "id": "loop1", "intent": "x", "kind": "do", "verb": "loop",
              "params": {
                "over": { "from": "input", "input": "users" },
                "as": "u",
                "do": [
                  { "id": "hi", "intent": "hi {{vars.u}}", "kind": "do", "verb": "reload" }
                ]
              } }
        ]);
        let steps: Vec<Step> = serde_json::from_value(body).unwrap();
        let mut inputs: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::new();
        inputs.insert("users".into(), serde_json::json!(["a", "b", "c"]));
        let flat = flatten_steps_with_scope(&steps, None, Some(&inputs)).unwrap();
        let intents: Vec<&str> = flat.iter().map(|s| s.intent()).collect();
        assert_eq!(intents, vec!["hi a", "hi b", "hi c"]);
    }

    #[test]
    fn flatten_steps_loop_unsupported_over_form_errors() {
        let body = serde_json::json!([
            { "id": "loop1", "intent": "x", "kind": "do", "verb": "loop",
              "params": {
                "over": { "from": "step", "stepId": "earlier" },
                "as": "x",
                "do": []
              } }
        ]);
        let steps: Vec<Step> = serde_json::from_value(body).unwrap();
        let err = flatten_steps(&steps, None).unwrap_err().to_string();
        assert!(err.contains("not yet supported"), "got: {err}");
    }

    #[test]
    fn parse_args_output_audit_flag() {
        let opts = parse_args(&[
            "./j.json".into(),
            "--output-audit".into(),
            "/tmp/a.json".into(),
        ])
        .unwrap();
        assert_eq!(
            opts.output_audit.as_deref(),
            Some(std::path::Path::new("/tmp/a.json"))
        );
        let opts = parse_args(&["./j.json".into(), "--output-audit=/tmp/b.json".into()]).unwrap();
        assert_eq!(
            opts.output_audit.as_deref(),
            Some(std::path::Path::new("/tmp/b.json"))
        );
        let opts = parse_args(&["./j.json".into()]).unwrap();
        assert!(opts.output_audit.is_none());
    }

    #[test]
    fn parse_args_tag_flag() {
        let opts = parse_args(&["./j.json".into(), "--tag".into(), "nightly".into()]).unwrap();
        assert_eq!(opts.tag.as_deref(), Some("nightly"));
        let opts = parse_args(&["./j.json".into(), "--tag=pre-deploy".into()]).unwrap();
        assert_eq!(opts.tag.as_deref(), Some("pre-deploy"));
        let opts = parse_args(&["./j.json".into()]).unwrap();
        assert!(opts.tag.is_none());
    }

    #[test]
    fn parse_args_quiet_flag() {
        let opts = parse_args(&["./j.json".into(), "--quiet".into()]).unwrap();
        assert!(opts.quiet);
        let opts = parse_args(&["./j.json".into(), "-q".into()]).unwrap();
        assert!(opts.quiet);
        let opts = parse_args(&["./j.json".into()]).unwrap();
        assert!(!opts.quiet);
    }

    #[test]
    fn parse_args_no_sidecars_flag() {
        let opts = parse_args(&["./j.json".into(), "--no-sidecars".into()]).unwrap();
        assert!(opts.no_sidecars);
        let opts = parse_args(&["./j.json".into()]).unwrap();
        assert!(!opts.no_sidecars);
    }

    #[test]
    fn parse_args_dry_run_flag() {
        let opts = parse_args(&["./j.json".into(), "--dry-run".into()]).unwrap();
        assert!(opts.dry_run);
        let opts = parse_args(&["./j.json".into()]).unwrap();
        assert!(!opts.dry_run);
    }

    #[test]
    fn replay_dry_run_skips_dispatch_and_env_phases() {
        // Build a scenario with env.open nav + one check step. With
        // --dry-run the fake browser should NEVER be invoked.
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        install_fake_browser(work.path(), &log);

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, minimal_scenario()).unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "sx".into(),
            heal_from_run: None,
            input_overrides: BTreeMap::new(),
            dry_run: true,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
        };
        let summary = run(&opts).unwrap();
        assert!(summary.ok);
        assert_eq!(summary.passed, 0);
        assert_eq!(summary.total, 1);

        // Fake browser MUST NOT have been invoked.
        let invocation = fs::read_to_string(&log).unwrap_or_default();
        assert!(
            invocation.is_empty(),
            "dry-run leaked agent-browser invocation: {invocation}"
        );

        // audit.json carries DRY-RUN in the summary.
        let latest = fs::read_to_string(jdir.join("replays").join("latest.txt")).unwrap();
        let run_id = latest.trim();
        let audit: serde_json::Value = serde_json::from_slice(
            &fs::read(jdir.join("replays").join(run_id).join("audit.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(audit["summary"], "SUMMARY: 0/1 (DRY-RUN)");
        clear_fake_browser();
    }

    #[test]
    fn parse_args_heal_from_run_long_and_eq_forms() {
        let opts =
            parse_args(&["./j.json".into(), "--heal-from-run".into(), "rABC".into()]).unwrap();
        assert_eq!(opts.heal_from_run.as_deref(), Some("rABC"));
        let opts = parse_args(&["./j.json".into(), "--heal-from-run=rXYZ".into()]).unwrap();
        assert_eq!(opts.heal_from_run.as_deref(), Some("rXYZ"));
    }

    #[test]
    fn replay_with_heal_from_run_overrides_value_at_dispatch() {
        // Build a scenario whose s1 is a `do/type` with a literal value;
        // place a heal-response in a prior run that corrects that value;
        // run replay --heal-from-run <prior>; assert the fake browser
        // received the corrected literal in the find-fill invocation.
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        install_fake_browser(work.path(), &log);

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(
            &jfile,
            r#"{
              "schema": "scenario/2", "id": "hf", "intent": "heal-from-run",
              "steps": [
                { "id": "s1", "intent": "fill", "kind": "do", "verb": "type",
                  "on": { "role": "textbox", "name": "Email" },
                  "value": { "from": "literal", "literal": "rejected@e.com" } }
              ]
            }"#,
        )
        .unwrap();

        // Stage a heal-response under prior run rPRIOR.
        let prior = jdir.join("replays/rPRIOR/heal-responses");
        fs::create_dir_all(&prior).unwrap();
        fs::write(
            prior.join("s1.json"),
            r#"{"stepId":"s1","mode":"value-correction","value":"corrected@e.com","recordedAt":"now"}"#,
        )
        .unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "sx".into(),
            heal_from_run: Some("rPRIOR".into()),
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
        };
        let summary = run(&opts).unwrap();
        assert!(summary.ok);

        let invocation = fs::read_to_string(&log).unwrap();
        assert!(
            invocation.contains("--session sx find role textbox fill --name Email corrected@e.com"),
            "expected the corrected literal to land in agent-browser invocation, got: {invocation}"
        );
        // Original literal must NOT have been used.
        assert!(
            !invocation.contains("rejected@e.com"),
            "original literal leaked into invocation: {invocation}"
        );

        // Audit records which stepIds got overridden.
        let latest = fs::read_to_string(jdir.join("replays").join("latest.txt")).unwrap();
        let run_id = latest.trim();
        let audit: serde_json::Value = serde_json::from_slice(
            &fs::read(jdir.join("replays").join(run_id).join("audit.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(audit["healOverridesApplied"], serde_json::json!(["s1"]));
        clear_fake_browser();
    }

    #[test]
    fn parse_args_path_form() {
        let opts = parse_args(&["./j.json".into()]).unwrap();
        match &opts.source {
            ScenarioSource::Path(p) => assert_eq!(p, &PathBuf::from("./j.json")),
            _ => panic!("expected Path"),
        }
        assert_eq!(opts.session_name, "default");
    }

    #[test]
    fn parse_args_sid_with_profile_derives_session() {
        let opts = parse_args(&["mysid".into(), "--profile".into(), "admin".into()]).unwrap();
        match &opts.source {
            ScenarioSource::Sid(j) => assert_eq!(j, "mysid"),
            _ => panic!("expected Sid"),
        }
        assert_eq!(opts.profile.as_deref(), Some("admin"));
        assert_eq!(opts.session_name, "admin-session");
    }

    #[test]
    fn parse_args_session_overrides_profile_derivation() {
        let opts = parse_args(&[
            "mysid".into(),
            "--profile=admin".into(),
            "--session=explicit".into(),
        ])
        .unwrap();
        assert_eq!(opts.session_name, "explicit");
    }

    #[test]
    fn parse_args_unknown_flag_errors() {
        let err = parse_args(&["--what".into(), "x".into()]).unwrap_err();
        assert!(err.to_string().contains("unknown flag"));
    }

    #[test]
    fn render_summary_formats_pass_and_fail() {
        let s = RunSummary {
            passed: 3,
            total: 3,
            ok: true,
        };
        assert_eq!(s.render(), "SUMMARY: 3/3 (PASS)");
        let s = RunSummary {
            passed: 1,
            total: 3,
            ok: false,
        };
        assert_eq!(s.render(), "SUMMARY: 1/3 (FAIL)");
    }

    // ----- event stream (events.jsonl + status.json) -----

    /// Read every row of `<run>/events.jsonl` as parsed JSON values.
    fn read_events(run_dir: &Path) -> Vec<serde_json::Value> {
        let body = fs::read_to_string(run_dir.join("events.jsonl")).unwrap_or_default();
        body.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).expect("events.jsonl row is valid JSON"))
            .collect()
    }

    fn run_dir_for(jdir: &Path) -> PathBuf {
        let latest = fs::read_to_string(jdir.join("replays").join("latest.txt")).unwrap();
        jdir.join("replays").join(latest.trim())
    }

    #[test]
    fn replay_emits_event_stream_on_pass() {
        // Acceptance (pass half): after a replay, events.jsonl has one
        // terminal row per executed step with the correct status, and
        // status.json ends in { state: done, ok: true }.
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        install_fake_browser(work.path(), &log);

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, minimal_scenario()).unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "evt".into(),
            heal_from_run: None,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
        };
        let summary = run(&opts).unwrap();
        assert!(summary.ok);

        let run_dir = run_dir_for(&jdir);
        let events = read_events(&run_dir);
        // Lifecycle: a `running` row then a terminal `pass` row for s1.
        let terminal: Vec<&serde_json::Value> = events
            .iter()
            .filter(|e| e["status"] == "pass" || e["status"] == "fail")
            .collect();
        assert_eq!(
            terminal.len(),
            1,
            "one terminal row per executed step; got {events:?}"
        );
        assert_eq!(terminal[0]["status"], "pass");
        assert_eq!(terminal[0]["id"], "s1");
        assert_eq!(terminal[0]["idx"], 1);
        assert_eq!(terminal[0]["total"], 1);
        assert_eq!(terminal[0]["kind"], "check");
        assert!(terminal[0]["ms"].is_number());
        assert_eq!(terminal[0]["screenshot"], "screenshots/s1.png");
        assert_eq!(terminal[0]["snapshot"], "snapshots/s1.txt");
        // A `running` row preceded the terminal row.
        assert!(
            events
                .iter()
                .any(|e| e["status"] == "running" && e["id"] == "s1"),
            "expected a running row for s1; got {events:?}"
        );

        // status.json ends done + ok:true.
        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(run_dir.join("status.json")).unwrap()).unwrap();
        assert_eq!(status["state"], "done");
        assert_eq!(status["ok"], serde_json::json!(true));
        assert_eq!(status["currentIdx"], 1);
        assert_eq!(status["total"], 1);
        clear_fake_browser();
    }

    #[test]
    fn replay_emits_fail_event_with_error_and_screenshot_paths() {
        // Acceptance (fail half): on the failing step, the terminal row
        // carries `error` + `screenshot`/`snapshot` paths, and status.json
        // ends in { state: done, ok: false }. s1 (url exists) passes; s2
        // (data binding that was never saved) fails at dispatch.
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        install_fake_browser(work.path(), &log);

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(
            &jfile,
            r#"{
                "schema": "scenario/2", "id": "evt-fail", "intent": "fail stream",
                "env": { "open": [{ "kind": "nav", "url": "https://example.com/", "intent": "land" }] },
                "steps": [
                    { "id": "s1", "intent": "url exists", "kind": "check",
                      "claim": { "subject": { "url": true }, "predicate": "exists" } },
                    { "id": "s2", "intent": "missing data", "kind": "check",
                      "claim": { "subject": { "data": "nope" }, "predicate": "exists" } }
                ]
            }"#,
        )
        .unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "evtf".into(),
            heal_from_run: None,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
        };
        // The run bails at the failing step; events/status are written
        // before the bail.
        let err = run(&opts).unwrap_err();
        assert!(format!("{err}").contains("s2"), "got: {err}");

        let run_dir = run_dir_for(&jdir);
        let events = read_events(&run_dir);
        let terminal: Vec<&serde_json::Value> = events
            .iter()
            .filter(|e| e["status"] == "pass" || e["status"] == "fail")
            .collect();
        // s1 pass + s2 fail; the run stopped at s2 (one terminal row each).
        assert_eq!(terminal.len(), 2, "got {events:?}");
        assert_eq!(terminal[0]["id"], "s1");
        assert_eq!(terminal[0]["status"], "pass");
        let fail = terminal[1];
        assert_eq!(fail["id"], "s2");
        assert_eq!(fail["status"], "fail");
        assert_eq!(fail["idx"], 2);
        assert_eq!(fail["total"], 2);
        assert!(
            fail["error"].as_str().unwrap().contains("nope"),
            "fail row must carry the dispatch error; got {fail:?}"
        );
        assert_eq!(fail["screenshot"], "screenshots/s2.png");
        assert_eq!(fail["snapshot"], "snapshots/s2.txt");

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(run_dir.join("status.json")).unwrap()).unwrap();
        assert_eq!(status["state"], "done");
        assert_eq!(status["ok"], serde_json::json!(false));
        assert_eq!(status["currentIdx"], 2);
        assert_eq!(status["total"], 2);
        clear_fake_browser();
    }

    #[test]
    fn replay_no_sidecars_omits_artifact_paths_but_keeps_events() {
        // --no-sidecars still emits the event stream + status, but the
        // terminal rows omit screenshot/snapshot (no capture happened).
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        install_fake_browser(work.path(), &work.path().join("ab.log"));

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, minimal_scenario()).unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "evtn".into(),
            heal_from_run: None,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: true,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
        };
        run(&opts).unwrap();

        let run_dir = run_dir_for(&jdir);
        let events = read_events(&run_dir);
        let terminal = events.iter().find(|e| e["status"] == "pass").unwrap();
        assert!(terminal.get("screenshot").is_none() || terminal["screenshot"].is_null());
        assert!(terminal.get("snapshot").is_none() || terminal["snapshot"].is_null());
        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(run_dir.join("status.json")).unwrap()).unwrap();
        assert_eq!(status["state"], "done");
        assert_eq!(status["ok"], serde_json::json!(true));
        clear_fake_browser();
    }

    #[test]
    fn replay_dry_run_emits_no_event_stream() {
        // --dry-run skips dispatch entirely, so there is no event stream
        // or status file (meaningful absence per the sidecar spec).
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        install_fake_browser(work.path(), &work.path().join("ab.log"));

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, minimal_scenario()).unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "evtd".into(),
            heal_from_run: None,
            input_overrides: BTreeMap::new(),
            dry_run: true,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
        };
        run(&opts).unwrap();
        let run_dir = run_dir_for(&jdir);
        assert!(!run_dir.join("events.jsonl").exists());
        assert!(!run_dir.join("status.json").exists());
        clear_fake_browser();
    }

    // ----- legible terminal: pure formatters -----

    fn parse_step(json: &str) -> Step {
        serde_json::from_str(json).expect("valid step json")
    }

    #[test]
    fn fmt_counter_right_aligns_to_total_width() {
        assert_eq!(fmt_counter(5, 12), "[ 5/12]");
        assert_eq!(fmt_counter(12, 12), "[12/12]");
        assert_eq!(fmt_counter(1, 9), "[1/9]");
        // width tracks the widest index so the column stays fixed.
        assert_eq!(fmt_counter(3, 100), "[  3/100]");
    }

    #[test]
    fn fmt_duration_splits_at_one_second() {
        assert_eq!(fmt_duration(850), "850ms");
        assert_eq!(fmt_duration(0), "0ms");
        assert_eq!(fmt_duration(1000), "1.0s");
        assert_eq!(fmt_duration(1234), "1.2s");
    }

    #[test]
    fn progress_label_uses_verb_and_targeted_name() {
        let step = parse_step(
            r#"{ "kind": "do", "id": "s", "intent": "open the menu",
                "verb": "click", "on": { "role": "button", "name": "Submit" } }"#,
        );
        assert_eq!(progress_label(&step), "click \"Submit\"");
    }

    #[test]
    fn progress_label_falls_back_to_intent_when_untargeted() {
        let step = parse_step(
            r#"{ "kind": "do", "id": "s", "intent": "reload the page", "verb": "reload" }"#,
        );
        assert_eq!(progress_label(&step), "reload — reload the page");
    }

    #[test]
    fn progress_label_for_check_uses_intent() {
        let step = parse_step(
            r#"{ "kind": "check", "id": "c", "intent": "url is the home page",
                "claim": { "subject": { "url": true }, "predicate": "exists" } }"#,
        );
        assert_eq!(progress_label(&step), "check — url is the home page");
    }

    #[test]
    fn fmt_progress_glyphs_differ_by_mode() {
        // Pretty mode uses glyphs + a duration suffix.
        assert_eq!(
            fmt_progress(
                ProgressMode::Pretty,
                5,
                12,
                StepState::Pass,
                "click \"Login\"",
                Some(1234)
            ),
            "[ 5/12] ✓ click \"Login\"  (1.2s)"
        );
        assert_eq!(
            fmt_progress(
                ProgressMode::Pretty,
                6,
                12,
                StepState::Running,
                "type \"email\"",
                None
            ),
            "[ 6/12] … type \"email\""
        );
        // Plain mode stays ASCII so piped/CI logs are clean + greppable.
        assert_eq!(
            fmt_progress(
                ProgressMode::Plain,
                5,
                12,
                StepState::Fail,
                "submit",
                Some(800)
            ),
            "[ 5/12] FAIL submit  (800ms)"
        );
    }

    #[test]
    fn render_failure_block_has_aligned_labels_and_paths() {
        let block = render_failure_block(&FailurePointer {
            idx: 5,
            total: 12,
            intent: "click the Login button",
            kind: "do:click",
            reason: "no element matched role=button name=\"Login\"",
            screenshot: Some("/abs/replays/r1/screenshots/openDialog.png"),
            snapshot: Some("/abs/replays/r1/snapshots/openDialog.txt"),
            run_dir: "/abs/replays/r1",
        });
        assert!(block.contains("✗ FAILED at step 5/12  \"click the Login button\"  (do:click)"));
        assert!(block.contains("\n  reason:     no element matched"));
        assert!(block.contains("\n  screenshot: /abs/replays/r1/screenshots/openDialog.png"));
        assert!(block.contains("\n  snapshot:   /abs/replays/r1/snapshots/openDialog.txt"));
        assert!(block.contains("\n  run dir:    /abs/replays/r1"));
        // Label columns line up under the longest key ("screenshot:").
        for key in ["reason:", "screenshot:", "snapshot:", "run dir:"] {
            let line = block
                .lines()
                .find(|l| l.trim_start().starts_with(key))
                .unwrap();
            assert_eq!(&line[..2], "  ", "two-space indent for {key}");
            let colon = line.find(':').unwrap();
            // value starts at a fixed column (12 from the indent).
            assert_eq!(
                line[colon + 1..].find(|c: char| c != ' ').unwrap() + colon + 1,
                14
            );
        }
    }

    #[test]
    fn render_failure_block_marks_missing_capture() {
        let block = render_failure_block(&FailurePointer {
            idx: 1,
            total: 1,
            intent: "do a thing",
            kind: "do:click",
            reason: "boom",
            screenshot: None,
            snapshot: None,
            run_dir: "/abs/r",
        });
        assert!(block.contains("screenshot: (not captured"));
        assert!(block.contains("snapshot:   (not captured"));
    }

    #[test]
    fn render_failure_block_collapses_reason_whitespace() {
        // A trailing newline / inner wrap (common from agent-browser stderr)
        // must not punch a blank gap into the block.
        let block = render_failure_block(&FailurePointer {
            idx: 1,
            total: 1,
            intent: "do a thing",
            kind: "do:click",
            reason: "element not found.\n  verify the selector\n",
            screenshot: None,
            snapshot: None,
            run_dir: "/abs/r",
        });
        assert!(block.contains("reason:     element not found. verify the selector\n"));
        assert!(!block.contains("reason:     element not found.\n"));
    }

    #[test]
    fn resolve_progress_mode_honours_quiet_and_plain() {
        let mk = |quiet: bool, plain: bool| RunOptions {
            source: ScenarioSource::Path("j.json".into()),
            profile: None,
            session_name: "s".into(),
            heal_from_run: None,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet,
            plain,
            tag: None,
            output_audit: None,
        };
        assert_eq!(resolve_progress_mode(&mk(true, false)), ProgressMode::Quiet);
        // quiet wins over plain.
        assert_eq!(resolve_progress_mode(&mk(true, true)), ProgressMode::Quiet);
        // --plain forces Plain even if stderr were a TTY.
        assert_eq!(resolve_progress_mode(&mk(false, true)), ProgressMode::Plain);
    }

    #[test]
    fn parse_args_plain_flag() {
        let opts = parse_args(&["./j.json".into(), "--plain".into()]).unwrap();
        assert!(opts.plain);
        let opts = parse_args(&["./j.json".into()]).unwrap();
        assert!(!opts.plain);
    }
}
