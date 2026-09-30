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

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

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
    /// `--persona <id>` — a `_personas/<id>/persona.json` record under the
    /// scenarios root; resolved before dispatch into `profile` + credential
    /// env vars (CLI mirror of the workbench's persona picker).
    pub persona: Option<String>,
    /// `--environment <id>` — a `_environments/<id>/environment.json`
    /// record; its `params`/`baseUrl` merge under `--param` overrides and
    /// its `auth.*` config lands as `AGENT_QA_ENV_*` vars.
    pub environment: Option<String>,
    pub session_name: String,
    /// `--heal-from-run <runId>` — pre-load caller-driven heal overrides
    /// from a prior run's `replays/<runId>/heal-responses/<stepId>.json`
    /// files. At step dispatch time, any do-step whose id appears in
    /// the map has its `value` replaced with the corrected literal.
    pub heal_from_run: Option<String>,
    /// `--headed` launches the browser with a visible window; `--headless`
    /// (the default) keeps it hidden. Applied to a freshly-launched session;
    /// a reused warm session keeps whatever mode it launched in.
    pub headed: bool,
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
    /// `--from <stepId>` — start dispatch at this step instead of the
    /// first. Steps before it are skipped entirely (no dispatch, no
    /// events). Only meaningful against a warm session already parked
    /// at that step's expected page state.
    pub from_step: Option<String>,
    /// `--until <stepId>` — stop dispatch after this step (inclusive).
    /// Steps after it never run; env.close still executes.
    pub until_step: Option<String>,
    /// `--update-baselines` — after the run finishes, mint every captured
    /// screenshot as a `baselines/<stepId>.png` (same copy `shot-accept`
    /// performs). Runs even when shot claims FAILED: intentional UI
    /// changes are exactly the case where a failing diff needs re-minting.
    pub update_baselines: bool,
    /// `--keep-going` — dispatch every step even after a failure instead
    /// of stopping at the first one (the default). Later steps often
    /// cascade-fail from the broken page state, but a repair sweep wants
    /// the complete failure list in one run's events/audit rather than
    /// re-running once per step.
    pub keep_going: bool,

    /// `--record-video [path]` — record the browser to video for the
    /// whole run (agent-browser `record start/stop`; needs ffmpeg on the
    /// runner). Bare flag writes `<run>/run.webm`; `=<path>` writes that
    /// path (.webm/.mp4). Start happens right before the step loop so
    /// env.open navigation is captured; stop after env.close.
    pub record_video: Option<PathBuf>,
    /// `--junit [path]` — write the run's terminal step outcomes as JUnit
    /// XML after the run. Bare `--junit` writes `<run>/junit.xml` (the
    /// empty-path sentinel); `--junit=<path>` writes that literal path.
    /// One <testcase> per step so any CI's test-result ingestion renders
    /// a replay like a unit-test run.
    pub junit: Option<PathBuf>,
    /// `--base-url <origin>` — retarget the scenario onto another deploy
    /// (e.g. a PR preview): every `env` nav url and `goto` literal rooted
    /// at the recorded origin is rewritten to this origin. See
    /// [`crate::scenario::retarget_origin`].
    pub base_url: Option<String>,
    /// After a passing run, apply this run's locator-correction heal
    /// patches back into scenario.json (heal-promote --apply for just this
    /// run). The content-hash guard still applies: a patch recorded
    /// against an older scenario is refused with a warning, never written.
    pub auto_promote: bool,
    /// `--freeze <iso>` — pin `Date.now()`/`new Date()` to the instant and
    /// replace `Math.random` with a seeded LCG via a page init script, so
    /// rendered timestamps and random ordering can't flake a golden diff.
    pub freeze: Option<String>,
    /// `--har` — record a HAR file for the run via `agent-browser network
    /// har start|stop` and write `<run>/network.har`. Unlike network.json
    /// (urls + statuses) a HAR carries response bodies — the artifact you
    /// open in DevTools/Charles when a claim needs the payload.
    pub har: bool,
    /// `--mock-from <runId>` — seed mock rules from that run's
    /// `network.har` (recorded via `--har`): the app's fetch/XHR calls get
    /// the recorded status + body instead of the real backend. Fails the
    /// run when the HAR is absent — a partial stub silently hitting the
    /// real backend is worse than no run.
    pub mock_from: Option<String>,
    /// `--offline` — fetch/XHR requests matching no mock rule get a network
    /// rejection instead of reaching the real backend. With `--mock-from`
    /// this is the full hermetic guarantee: only recorded traffic replays.
    /// Alone it stubs every request (UI-only replays on static pages).
    pub offline: bool,
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

/// Emulation toggles applied via `agent-browser set` (a `do/emulate` step
/// or `set` CLI call) are daemon-side state: they survive a run and poison
/// the next replay on the reused session — a stale `offline on` silently
/// drops every request while a warm `goto` skip hides that anything is
/// wrong. Reset the toggles that have a documented off state at run start,
/// before `--offline`/`--mock-from` and `do/emulate` steps re-apply what
/// *this* run wants. `viewport`, `device`, `geo`, `credentials`, and
/// `media` have no `set`-level clear — they still carry over.
fn reset_persistent_emulation(session: &str) {
    for args in [["set", "offline", "off"], ["set", "headers", "{}"]] {
        if let Err(e) = crate::browser::run(
            session,
            args,
            crate::browser::RunOpts::new().lenient().capture(),
        ) {
            eprintln!(
                "[v2-replay] emulation reset ({}) skipped: {e}",
                args.join(" ")
            );
        }
    }
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
    // 0. Browser launch mode (headless by default; --headed shows the window).
    // Set before any agent-browser child is spawned so a freshly-launched
    // session picks it up.
    crate::browser::set_headed_mode(opts.headed);
    let connection = crate::browser::BrowserConnection::resolve()?;
    crate::browser::set_connection(&connection);
    // A reused session name may carry mock rules from a prior scenario
    // (`replay --all` suites, workbench runs) — start clean.
    crate::mock::clear(&opts.session_name, None);

    // The browser's request + console captures are per-session: a replayed
    // session still holds the previous run's traffic and messages. Clear
    // both so this run's network.json / console.json (and {"network"} /
    // {"console"} claims) see only its own activity. Best-effort — a
    // session that doesn't exist yet just warns.
    if !opts.dry_run {
        if let Err(e) = crate::browser::network_clear(&opts.session_name) {
            eprintln!("[v2-replay] network log clear skipped: {e}");
        }
        if let Err(e) = crate::browser::console_clear(&opts.session_name) {
            eprintln!("[v2-replay] console log clear skipped: {e}");
        }
        reset_persistent_emulation(&opts.session_name);
        // Own Network.* event capture — redirect-hop statuses and
        // in-flight tracking the daemon's netlog doesn't expose. A fresh
        // store per run; silently skipped without a CDP endpoint.
        crate::cdp_net::start(&opts.session_name);
    }

    // 1. Load + validate.
    let (scenario_file, scenario_dir) = resolve_source(&opts.source)?;
    let bytes =
        fs::read(&scenario_file).with_context(|| format!("read {}", scenario_file.display()))?;
    let parsed = schema::validate_bytes(&bytes)
        .with_context(|| format!("validate {}", scenario_file.display()))?;
    let mut scenario: Scenario =
        serde_json::from_value(parsed.clone()).context("parse scenario")?;
    if let Some(to) = &opts.base_url {
        match crate::scenario::retarget_origin(&mut scenario, to) {
            Some(from) => {
                eprintln!("[v2-replay] --base-url: retargeted {from} → {to}");
            }
            None => {
                eprintln!("[v2-replay] --base-url: scenario has no recorded origin — flag ignored")
            }
        }
    }
    let hash = hash_scenario_bytes(&bytes);
    // Union of `mask` selectors across the scenario's shot claims — hidden
    // (visibility:hidden) around every step screenshot so volatile UI
    // (timestamps, live badges) can't flake the visual diff. Precomputed
    // once: a claim's mask must already be in effect when the referenced
    // step's screenshot is captured, before the claim step itself runs.
    let shot_masks = scenario_shot_masks(&scenario);

    // Native-dialog steps need `alert`/`beforeunload` kept pending instead of
    // agent-browser's default auto-accept, or a recorded `dialog` step finds
    // nothing to resolve. The daemon reads the flag at launch; a warm-up eval
    // brings the daemon up before the first navigation (a launch-time `open`
    // can lose its navigation to the launch race).
    let uses_dialog = scenario_uses_dialog(&scenario);
    if uses_dialog && !opts.dry_run {
        crate::browser::set_no_auto_dialog(true);
        let _ = crate::browser::eval_expression(&opts.session_name, "1");
    }
    // Shot claims diff pixels — kill capture-time noise first (fonts still
    // decoding, images mid-fetch, layout mid-frame) rather than absorbing
    // it in tolerance. Bounded ~1.5s so it never meaningfully slows a run.
    let stabilize_shots = scenario_uses_shots(&scenario);

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
        auto_healed: None,
        tag: opts.tag.clone(),
        window_from: None,
        window_until: None,
    };
    write_run_audit(&run, &audit)?;

    // 4. Resolve inputs (declared defaults + --param overrides +
    //    inputs.local.json beside the scenario).
    let local_inputs = load_local_inputs(&scenario_dir);
    let (resolved_inputs, audit_params) = resolve_inputs(
        scenario.inputs.as_ref(),
        &opts.input_overrides,
        &local_inputs,
    )?;
    audit.parameters = if audit_params.is_empty() {
        None
    } else {
        Some(audit_params)
    };

    // Determinism layer: `--freeze <iso>` pins the clock + RNG via a page
    // init script (fresh sessions get it on every navigation; the live
    // eval covers a warm session's current document).
    if let Some(at) = &opts.freeze {
        if opts.dry_run {
            eprintln!("[v2-replay] --freeze ignored under --dry-run");
        } else {
            let js_path = run.run_root.join("freeze.js");
            fs::write(&js_path, freeze_js(at))
                .with_context(|| format!("write {}", js_path.display()))?;
            std::env::set_var("AGENT_BROWSER_INIT_SCRIPTS", &js_path);
            let src = fs::read_to_string(&js_path)?;
            let _ = browser::eval_expression(&opts.session_name, &src);
            eprintln!("[v2-replay] freeze {at} → {}", js_path.display());
        }
    }

    // 5. env.open setup. Skipped under --dry-run.
    //
    // A setup failure (e.g. a `useProfile` op whose profile bootstrap can't
    // authenticate — no credentials in env) must still leave a TERMINAL run.
    // Before, `?` propagated here BEFORE the first status.json write, so the
    // run had only audit.json and the viewer showed it "in flight" forever (a
    // ghost). Now we finalise a done/failed status + audit first, so the run
    // surfaces as FAIL with the setup error, then propagate.
    // Seed mock stubs BEFORE env.open launches the session: rules written
    // as a page init script install before the *first* navigation, so even
    // page-load fetches are stubbed. A warm session that already exists
    // ignores init scripts — the post-navigation re-apply still covers its
    // in-page XHR/fetch traffic.
    if opts.mock_from.is_some() || opts.offline {
        if opts.dry_run {
            eprintln!("[v2-replay] --mock-from/--offline ignored under --dry-run");
        } else {
            if opts.offline {
                crate::mock::set_strict(&opts.session_name, true);
            }
            if let Some(from) = &opts.mock_from {
                let n = crate::mock::seed_from_har(&opts.session_name, &scenario_dir, from)
                    .with_context(|| format!("--mock-from {from:?}"))?;
                eprintln!("[v2-replay] mock-from {from}: {n} stub(s) seeded");
            }
            let js_path = crate::mock::write_init_script(&opts.session_name, &run.run_root)
                .with_context(|| "mock seed: write init script")?;
            // Children spawned from this process inherit the var; the
            // daemon registers the script on session launch.
            std::env::set_var("AGENT_BROWSER_INIT_SCRIPTS", &js_path);
            eprintln!("[v2-replay] mock init script {}", js_path.display());
        }
    }

    let mut scope = ValueScope::new(resolved_inputs);
    // HAR recording starts before env.open so the open-phase navigation is
    // part of the captured traffic.
    if opts.har && !opts.dry_run {
        if let Err(e) = crate::browser::network_har_start(&opts.session_name) {
            eprintln!("[v2-replay] har start skipped: {e}");
        }
    }
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
    // stepIds that self-healed via an inline locator correction (auto-heal).
    let mut healed_steps: Vec<String> = Vec::new();
    // Ids of steps the --from/--until window excluded — surfaced to --junit
    // as <skipped/> cases so CI sees the full scenario, not just the slice.
    let mut skipped_step_ids: Vec<String> = Vec::new();
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
        crate::verbs::clear_dismissed();
        let do_ctx = DoContext {
            session: &opts.session_name,
            scenario_dir: &scenario_dir,
            visual_checks: scenario_has_shot_claims(&scenario),
            uses_dialog,
        };
        let check_ctx = CheckContext {
            session: &opts.session_name,
            scenario_dir: &scenario_dir,
            run_dir: Some(&run.run_root),
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
        // `--record-video`: start filming before the first step so the
        // whole interaction lands in <run>/run.webm. Best-effort — a
        // missing ffmpeg is a warning, not a failed run.
        if let Some(vid) = &opts.record_video {
            let dest = if vid.as_os_str().is_empty() {
                run.run_root.join("run.webm")
            } else {
                vid.clone()
            };
            match browser::record_video_start(&opts.session_name, &dest) {
                Ok(()) => eprintln!("[v2-replay] recording → {}", dest.display()),
                Err(e) => eprintln!("[v2-replay] record start skipped: {e}"),
            }
        }
        // `--from`/`--until` narrow the dispatch window. Steps outside
        // the window never dispatch — no events, no summary rows — but
        // their ids are remembered so `--junit` can emit <skipped/>.
        let flat_all_ids: Vec<String> = flat.iter().map(|s| s.id().to_string()).collect();
        let flat: Vec<Step> =
            match apply_step_window(flat, opts.from_step.as_ref(), opts.until_step.as_ref()) {
                Ok(v) => v,
                Err(e) => {
                    summary.ok = false;
                    first_failure.get_or_insert(format!("step window: {e}"));
                    Vec::new()
                }
            };
        skipped_step_ids = if opts.from_step.is_some() || opts.until_step.is_some() {
            let kept: std::collections::BTreeSet<&str> = flat.iter().map(|s| s.id()).collect();
            flat_all_ids
                .into_iter()
                .filter(|id| !kept.contains(id.as_str()))
                .collect()
        } else {
            Vec::new()
        };
        if opts.from_step.is_some() || opts.until_step.is_some() {
            eprintln!(
                "[v2-replay] step window: {}..{} — dispatching {} step(s)",
                opts.from_step.as_deref().unwrap_or("<start>"),
                opts.until_step.as_deref().unwrap_or("<end>"),
                flat.len()
            );
        }
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
        // The last click dispatched — re-fired to reopen a popup that a
        // subsequent option/menuitem click finds dismissed. See the
        // transient-popup recovery in the dispatch match below.
        let mut prev_click: Option<Step> = None;
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
                Step::Do { save_as, .. } => {
                    let mut outcome = dispatch_do(&patched_step, &do_ctx, &mut scope);
                    // Per-step retry: params.retry re-dispatches the step
                    // on failure — cheap flake absorption for one known-
                    // flaky interaction without --retry's whole-run cost.
                    // params.retryMs sets the inter-attempt delay
                    // (default 300ms). Caveat: verbs whose side effect
                    // isn't idempotent (e.g. `type` appends) can apply it
                    // per attempt when the first attempt half-dispatched.
                    if let Step::Do {
                        params: Some(p), ..
                    } = &patched_step
                    {
                        let retries = p.get("retry").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        let delay = p.get("retryMs").and_then(|v| v.as_u64()).unwrap_or(300);
                        let mut attempt = 0u32;
                        while outcome.is_err() && attempt < retries {
                            attempt += 1;
                            eprintln!("[v2-replay] step {id}: retry {attempt}/{retries}");
                            std::thread::sleep(std::time::Duration::from_millis(delay));
                            outcome = dispatch_do(&patched_step, &do_ctx, &mut scope);
                        }
                    }
                    // Transient-popup recovery: an option/menuitem click that
                    // failed usually means the popup was dismissed between steps
                    // (inter-step keyframe capture, a re-render). Re-fire the
                    // previous opener click to reopen it, then retry once.
                    if outcome.is_err() && crate::verbs::is_popup_content_click(&patched_step) {
                        if let Some(opener) = &prev_click {
                            eprintln!(
                                "[v2-replay] popup content click failed; re-opening via previous opener and retrying"
                            );
                            let _ = dispatch_do(opener, &do_ctx, &mut scope);
                            std::thread::sleep(std::time::Duration::from_millis(300));
                            outcome = dispatch_do(&patched_step, &do_ctx, &mut scope);
                        }
                    }
                    // Inline auto-heal: a locator miss whose recorded
                    // role+name resolves to exactly one live candidate under
                    // a looser strategy retries once with the corrected
                    // locator — drift is corrected in-run and persisted
                    // (heal.jsonl row + diffs/<stepId>.patch.json). Only
                    // find-family failures count as a miss — anything else
                    // may have dispatched partially, and retrying could
                    // double-fire the action.
                    if outcome.is_err() && crate::auto_heal::enabled() {
                        let miss =
                            matches!(&outcome, Err(e) if crate::auto_heal::is_locator_miss(e));
                        match if miss {
                            crate::auto_heal::attempt(&patched_step, &opts.session_name, &mut scope)
                        } else {
                            Ok(None)
                        } {
                            Ok(Some(heal)) => {
                                eprintln!(
                                    "[v2-replay] auto-heal: step '{id}' locator '{}' → '{}' ({}); retrying once",
                                    heal.from, heal.to, heal.strategy
                                );
                                let corrected =
                                    crate::auto_heal::apply_correction(&patched_step, &heal);
                                match dispatch_do(&corrected, &do_ctx, &mut scope) {
                                    Ok(saved) => {
                                        healed_steps.push(id.to_string());
                                        if let Err(e) = crate::auto_heal::persist_correction(
                                            &run, &hash, id, &heal,
                                        ) {
                                            eprintln!(
                                                "[v2-replay] heal audit write failed (continuing): {e}"
                                            );
                                        }
                                        outcome = Ok(saved);
                                    }
                                    Err(retry_err) => {
                                        outcome = Err(retry_err.context(format!(
                                            "auto-heal via {} did not fix step {id}",
                                            heal.strategy
                                        )));
                                    }
                                }
                            }
                            Ok(None) => {
                                // Not a healable locator miss — check for a
                                // value rejection and surface it in the audit
                                // trail (classified, never retried). Three
                                // channels: global alert surfaces, field-level
                                // constraint validation around the step's own
                                // locator, and the error itself (HTTP 4xx /
                                // GraphQL errors[] carry the refusal when the
                                // page shows nothing).
                                let mut ev = crate::auto_heal::rejection_evidence(
                                    &opts.session_name,
                                    step_css_hint(&patched_step).as_deref(),
                                );
                                if let Err(e) = &outcome {
                                    if let Some(sig) = crate::auto_heal::error_signal(e) {
                                        if !ev.contains(&sig) {
                                            ev.push(sig);
                                        }
                                    }
                                }
                                if !ev.is_empty() {
                                    eprintln!(
                                        "[v2-replay] step '{id}' looks like a value rejection: {}",
                                        ev.join(" | ")
                                    );
                                    let _ = crate::auto_heal::persist_rejection(&run, id, &ev);
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "[v2-replay] auto-heal probe failed (keeping original error): {e}"
                                );
                            }
                        }
                    }
                    match outcome {
                        Ok(saved) => {
                            if let (Some(name), Some(value)) = (save_as.as_deref(), saved) {
                                scope.saved_steps.insert(name.to_string(), value);
                            }
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
                Step::Check { claim, context, .. } => dispatch_check(
                    claim,
                    &check_ctx,
                    &mut scope,
                    context
                        .as_ref()
                        .and_then(|c| c.timeout_ms)
                        .map(Duration::from_millis),
                ),
            };
            // Navigation wipes the page's JS world — reinstall registered
            // network mocks after navigation verbs so stubs survive loads.
            if result.is_ok() && crate::verbs::is_navigation_step(&patched_step) {
                if let Err(e) = crate::mock::reapply_if_any(&opts.session_name) {
                    eprintln!("[v2-replay] mock re-apply failed (continuing): {e}");
                }
            }
            // Remember the last click as a potential popup opener for the
            // transient-popup recovery above.
            if crate::verbs::is_click_step(&patched_step) {
                prev_click = Some(patched_step.clone());
            }
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
                        capture_step_sidecars(
                            &run,
                            id,
                            &opts.session_name,
                            stabilize_shots,
                            &shot_masks,
                        );
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
                        capture_step_sidecars(
                            &run,
                            id,
                            &opts.session_name,
                            stabilize_shots,
                            &shot_masks,
                        );
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
                    if !opts.keep_going {
                        break;
                    }
                    eprintln!("[v2-replay] --keep-going: continuing after step {id}'s failure");
                }
            }
        }
        // Strict mode: a run that needed any heal exits non-zero even
        // though every step passed — drift surfaces for review instead
        // of silently self-correcting.
        if crate::auto_heal::strict() && !healed_steps.is_empty() {
            summary.ok = false;
            eprintln!(
                "[v2-replay] auto-heal STRICT: {} step(s) self-healed ({}); failing the run so drift gets reviewed",
                healed_steps.len(),
                healed_steps.join(", ")
            );
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

        // Persist the session's captured network requests as
        // <run>/network.json — the traffic list {"network"} claims queried
        // mid-run, kept for post-hoc review/diff. Best-effort, and only
        // when the run actually executed (a dry-run session has no traffic).
        if !opts.dry_run {
            crate::netlog::write_network_log_warn(&run, &opts.session_name);
        }

        // Flush the HAR recording before env.close (teardown traffic —
        // profile saves, cleanup navigations — isn't part of the scenario's
        // network evidence).
        if opts.har {
            let dest = run.run_root.join("network.har");
            if let Err(e) = crate::browser::network_har_stop(&opts.session_name, &dest) {
                eprintln!("[v2-replay] har stop skipped: {e}");
            }
        }

        // Persist the session's captured network requests as
        // <run>/network.json — the traffic list {"network"} claims queried
        // mid-run, kept for post-hoc review/diff. Best-effort, and only
        // when the run actually executed (a dry-run session has no traffic).
        if !opts.dry_run {
            crate::netlog::write_network_log_warn(&run, &opts.session_name);
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
    // Suggested promotions: a run that self-healed locators leaves patch
    // files under replays/<run>/diffs — point at them + the command that
    // applies them. Skipped under strict mode (the run already failed and
    // the operator wants the drift reviewed, not promoted).
    if !healed_steps.is_empty() && !crate::auto_heal::strict() {
        eprintln!(
            "[v2-replay] auto-heal applied to {} step(s) ({}); review {} then promote with: agent-qa heal-promote {} --run {}",
            healed_steps.len(),
            healed_steps.join(", "),
            run.run_root.join("diffs").display(),
            scenario.id,
            run.run_id,
        );
    }
    audit.finished_at = Some(now_iso());
    audit.summary = Some(summary_line.clone());
    audit.exit_code = Some(if summary.ok { 0 } else { 1 });
    if opts.heal_from_run.is_some() {
        audit.heal_overrides_applied = Some(applied_overrides);
    }
    if !healed_steps.is_empty() {
        audit.auto_healed = Some(healed_steps.clone());
    }
    if let Some(f) = &opts.from_step {
        audit.window_from = Some(f.clone());
    }
    if let Some(u) = &opts.until_step {
        audit.window_until = Some(u.clone());
    }
    write_run_audit(&run, &audit)?;

    // 9.4. Persist the session console log alongside audit.json — the
    // `{"console"}` claims evaluated live during the run; landing the
    // captured messages in the run dir keeps the evidence after the
    // session closes (mirrors how the request log lands via --har).
    if !opts.dry_run {
        match browser::console_messages(&opts.session_name) {
            Ok(msgs) => {
                let body = serde_json::json!({
                    "session": opts.session_name,
                    "count": msgs.len(),
                    "messages": msgs.iter().map(|m| serde_json::json!({
                        "type": m.level,
                        "text": m.text,
                    })).collect::<Vec<_>>(),
                });
                match serde_json::to_vec_pretty(&body) {
                    Ok(b) => {
                        if let Err(e) = crate::sidecar::atomic_write_file(
                            &run.run_root.join("console.json"),
                            &b,
                        ) {
                            eprintln!("[v2-replay] console.json write failed: {e}");
                        }
                    }
                    Err(e) => eprintln!("[v2-replay] console.json serialize failed: {e}"),
                }
            }
            Err(e) => eprintln!("[v2-replay] console.json skipped: {e}"),
        }
    }

    // 9.5. Optional --output-audit duplicate (atomic write).
    if let Some(out) = &opts.output_audit {
        let body = serde_json::to_vec_pretty(&audit)?;
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        crate::sidecar::atomic_write_file(out, &body)
            .with_context(|| format!("--output-audit write {}", out.display()))?;
    }

    // 8.5. Stop the video recording (started under `--record-video`).
    // Runs before audit finalize so the artifact exists even on FAIL.
    if opts.record_video.is_some() && !opts.dry_run {
        match browser::record_video_stop(&opts.session_name) {
            Ok(()) => {}
            Err(e) => eprintln!("[v2-replay] record stop skipped: {e}"),
        }
    }

    // 9. Latest pointer.
    update_latest_pointer(&scenario_dir, &run.run_id)?;

    // 9.6. `--auto-promote` — self-healing write-back. Only on a passing
    // run: locator-correction patches this run produced are applied to
    // scenario.json via the same hash-guarded plan `heal-promote --apply`
    // uses. A refusal (the file changed since the run started) is a
    // warning, never a run failure. Skipped for --dry-run (no patches can
    // exist) and runs that wrote no corrections.
    if opts.auto_promote && !opts.dry_run && summary.ok && !healed_steps.is_empty() {
        match crate::heal_promote::promote_run(&scenario_file, &run.run_id) {
            Ok(0) => {}
            Ok(n) => eprintln!(
                "[v2-replay] auto-promoted {n} locator patch(es) into {}",
                scenario_file.display()
            ),
            Err(err) => eprintln!("[v2-replay] --auto-promote skipped: {err}"),
        }
    }

    // 10. `--update-baselines` — mint from this run's capture set. Runs
    // before the failure bail on purpose: a shot claim that correctly
    // flagged a real UI change is the re-mint case, and requiring a
    // passing run first would force a shot-accept round trip.
    if opts.update_baselines {
        match crate::shot_accept::mint_baselines(&scenario_dir, &run.run_id, None, false, false) {
            Ok(minted) => eprintln!(
                "baselines: minted {} shot(s) from run {}: {}",
                minted.len(),
                run.run_id,
                minted.join(", ")
            ),
            Err(err) => eprintln!("baselines: skipped ({err})"),
        }
    }

    // 11. `--junit` — XML report for CI test-result ingestion. Written
    // before the failure bail so a FAIL run still produces its report
    // (that's the case CI actually needs to render).
    if let Some(dest) = &opts.junit {
        let dest = crate::junit::resolve_dest(&run.run_root, dest);
        match crate::junit::write(&run.run_root, &scenario.id, &dest, &skipped_step_ids) {
            Ok((t, f)) => eprintln!(
                "[v2-replay] junit → {} ({t} tests, {f} failures)",
                dest.display()
            ),
            Err(err) => eprintln!("[v2-replay] junit skipped: {err}"),
        }
    }

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
/// Read `<scenario_dir>/inputs.local.json` — the gitignored file `flush`
/// writes recorded secrets into so a committed scenario still replays on
/// the machine that recorded it. Missing or unparsable → empty map.
fn load_local_inputs(scenario_dir: &std::path::Path) -> BTreeMap<String, serde_json::Value> {
    let p = scenario_dir.join("inputs.local.json");
    match fs::read(&p) {
        Ok(bytes) => serde_json::from_slice::<BTreeMap<String, serde_json::Value>>(&bytes)
            .unwrap_or_else(|e| {
                eprintln!("[v2-replay] {} unparsable, ignoring: {e}", p.display());
                BTreeMap::new()
            }),
        Err(_) => BTreeMap::new(),
    }
}

fn resolve_inputs(
    declared: Option<&BTreeMap<String, InputDecl>>,
    overrides: &BTreeMap<String, String>,
    local: &BTreeMap<String, serde_json::Value>,
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
                None => match local.get(name) {
                    Some(v) => (v.clone(), ParameterSource::Local),
                    None => continue,
                },
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

/// A css selector for the step's `on` locator when one exists — the field
/// probe checks that element's own constraint-validation state first.
/// Role/text/xpath locators have no css equivalent; return None.
fn step_css_hint(step: &Step) -> Option<String> {
    let Step::Do {
        on: Some(Locator::Raw(raw)),
        ..
    } = step
    else {
        return None;
    };
    match &raw.raw.kind {
        crate::scenario::RawLocatorKind::Css => Some(raw.raw.value.clone()),
        crate::scenario::RawLocatorKind::TestId => {
            Some(format!("[data-testid=\"{}\"]", raw.raw.value))
        }
        _ => None,
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
fn capture_step_sidecars(
    run: &crate::sidecar::RunPaths,
    step_id: &str,
    session: &str,
    stabilize_shots: bool,
    shot_masks: &[String],
) {
    use crate::sidecar::{ensure_kind_dir, step_sidecar_path, write_step_sidecar, SidecarKind};
    if !is_safe_step_id(step_id) {
        eprintln!("[v2-replay] skip sidecars for unsafe stepId {step_id:?}");
        return;
    }
    // A pending native dialog blocks the page: snapshots time out and the
    // screenshot captures the dialog's dimmed backdrop. The next step
    // (a `dialog` accept/dismiss) restores the page — capture after that.
    if browser::dialog_pending(session) {
        eprintln!("[v2-replay] skip sidecars for {step_id} — native dialog pending");
        return;
    }
    // Let the page settle after the step's action before capturing, so a
    // navigating click / async render is reflected in the screenshot + ARIA
    // snapshot rather than a half-loaded frame. Soft-fail: a page that is
    // already idle (or a networkidle timeout on a chatty page) must never
    // fail the run — this only governs artifact fidelity. BOUNDED: a chatty SPA
    // (e.g. a dashboard that polls) never reaches networkidle, so an unbounded
    // settle would stall each step for the full agent-browser timeout (~30s).
    // 3s is plenty for the common "settling after a nav/click" case.
    const SETTLE_CAP_MS: u64 = 3000;
    const SNAPSHOT_CAP_MS: u64 = 4000;
    // The first capture of a run also pays browser cold start: measured ~9.9s
    // on a fresh session against a trivial page, vs ~0.2s for every capture
    // after it. Under one flat 4s cap the first sidecar of EVERY run times
    // out, prints a failure, and then lands anyway once agent-browser catches
    // up — noise that trains the reader to ignore real capture failures. One
    // wider budget for the cold capture keeps the tight steady-state cap.
    const FIRST_CAPTURE_CAP_MS: u64 = 20_000;
    static FIRST_CAPTURE_DONE: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    let cap_ms = if FIRST_CAPTURE_DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        SNAPSHOT_CAP_MS
    } else {
        FIRST_CAPTURE_CAP_MS
    };
    let _ = browser::wait_for_load_capped(session, "networkidle", SETTLE_CAP_MS);
    // The settle wait can leave a dialog pending (e.g. a beforeunload fired
    // during it) — check again right before snapshotting.
    if browser::dialog_pending(session) {
        eprintln!("[v2-replay] skip sidecars for {step_id} — native dialog pending");
        return;
    }
    match browser::snapshot_full_capped(session, cap_ms) {
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
    // Shot claims diff this image pixel-wise — wait for fonts/images/
    // layout to settle first so a mid-decode frame doesn't read as drift.
    if stabilize_shots {
        stabilize_visual(session, 1500);
    }
    if let Ok(path) = step_sidecar_path(run, SidecarKind::Screenshots, step_id) {
        let masked = !shot_masks.is_empty() && apply_shot_mask(session, shot_masks);
        match browser::screenshot(session, &path, true, Some(cap_ms)) {
            Ok(true) => {}
            Ok(false) => eprintln!(
                "[v2-replay] screenshot {step_id} exited non-zero within the {cap_ms}ms cap (lenient — artifact may be missing or partial)"
            ),
            Err(e) => eprintln!("[v2-replay] screenshot {step_id} failed: {e}"),
        }
        if masked {
            clear_shot_mask(session);
        }
    }
}

/// Serialized `subject` objects of every `{"shot": ...}` claim in the
/// scenario. Walks the serialized steps so claims nested inside group/loop
/// `params.steps` or `useTemplate` bodies count too — the same trick
/// `scenario_uses_dialog` uses.
fn scenario_shot_subjects(scenario: &Scenario) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = Vec::new();
    let v = serde_json::to_value(&scenario.steps).unwrap_or(serde_json::Value::Null);
    fn walk(v: &serde_json::Value, out: &mut Vec<serde_json::Value>) {
        match v {
            serde_json::Value::Object(map) => {
                if let Some(subject) = map.get("claim").and_then(|c| c.get("subject")) {
                    if subject.get("shot").and_then(|s| s.as_str()).is_some() {
                        out.push(subject.clone());
                    }
                }
                for v in map.values() {
                    walk(v, out);
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    walk(v, out);
                }
            }
            _ => {}
        }
    }
    walk(&v, &mut out);
    out
}

/// True when the scenario carries any `{"shot": ...}` claim. Visual scenarios
/// opt out of the warm-page `goto` reuse: a reused session otherwise diffs a
/// stale document (old bundle, settled live data) against a baseline minted
/// from a fresh load — the mask injects into the wrong DOM and the shot
/// compares apples to oranges.
fn scenario_has_shot_claims(scenario: &Scenario) -> bool {
    !scenario_shot_subjects(scenario).is_empty()
}

/// Union of `mask` selectors declared on the scenario's `{"shot": ...}`
/// claims.
fn scenario_shot_masks(scenario: &Scenario) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for subject in scenario_shot_subjects(scenario) {
        if let Some(mask) = subject.get("mask").and_then(|m| m.as_array()) {
            for sel in mask.iter().filter_map(|m| m.as_str()) {
                if !out.iter().any(|o| o == sel) {
                    out.push(sel.to_string());
                }
            }
        }
    }
    out
}

/// Hide the shot-mask selectors by injecting a `<style>` tag —
/// `visibility:hidden !important` keeps layout put and, unlike inline
/// styles, keeps hiding nodes that remount between the eval and the
/// capture (live-updating lists reconcile into fresh DOM).
/// `clear_shot_mask` removes the tag. Returns true when the mask was
/// applied (restore is owed).
fn apply_shot_mask(session: &str, masks: &[String]) -> bool {
    let sels = serde_json::to_string(masks).unwrap_or_else(|_| "[]".to_string());
    let js = format!(
        "(() => {{ const sels = {sels}; document.getElementById('__qa_shot_mask')?.remove(); const st = document.createElement('style'); st.id = '__qa_shot_mask'; st.textContent = sels.map(s => s + ' {{ visibility: hidden !important; }}').join('\\n'); document.head.appendChild(st); return sels.length; }})()"
    );
    match browser::eval_expression(session, &js) {
        Ok(out) => {
            eprintln!("[v2-replay] shot mask applied: {out}");
            true
        }
        Err(e) => {
            eprintln!("[v2-replay] shot mask hide failed (capturing unmasked): {e}");
            false
        }
    }
}

fn clear_shot_mask(session: &str) {
    let js = "(() => { const st = document.getElementById('__qa_shot_mask'); if (st) { st.remove(); return 1; } return 0; })()";
    if let Err(e) = browser::eval_expression(session, js) {
        eprintln!(
            "[v2-replay] shot mask restore failed (page keeps hidden elements until next nav): {e}"
        );
    }
}

/// Poll the page until fonts have decoded, all `<img>`s have completed,
/// and `document.readyState` is `complete` — or `cap_ms` elapses. Sync-eval
/// based (agent-browser `eval` doesn't await Promises): each poll returns a
/// JSON status, and a settled status ends the wait. Soft-fail: any error
/// or timeout just stops waiting — artifact fidelity only.
fn stabilize_visual(session: &str, cap_ms: u64) {
    let deadline = Instant::now() + Duration::from_millis(cap_ms);
    loop {
        let status = browser::eval_expression(
            session,
            "(() => { const imgs = Array.from(document.images || []).filter(i => !i.complete).length; return JSON.stringify({fonts: document.fonts ? document.fonts.status : 'loaded', imgs, ready: document.readyState}); })()",
        );
        // In-flight fetches don't show up in readyState/images — a lazy data
        // load can still repaint after the screenshot. Require the session's
        // request log to be quiet too.
        let pending = browser::network_requests(session)
            .map(|rs| rs.iter().filter(|r| r.status.is_none()).count())
            .unwrap_or(0);
        match status {
            Ok(raw) => {
                // eval stdout may wrap the JSON in a quoted string — peel once.
                let peeled = raw.trim().trim_matches('"').replace("\\\"", "\"");
                let settled = serde_json::from_str::<serde_json::Value>(raw.trim())
                    .or_else(|_| serde_json::from_str::<serde_json::Value>(&peeled))
                    .map(|v| {
                        v.get("fonts").and_then(|f| f.as_str()) == Some("loaded")
                            && v.get("imgs").and_then(|i| i.as_u64()) == Some(0)
                            && v.get("ready").and_then(|r| r.as_str()) == Some("complete")
                    })
                    .unwrap_or(false);
                if settled && pending == 0 {
                    return;
                }
            }
            Err(_) => return,
        }
        if Instant::now() >= deadline {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// The `--freeze` init script: pin `Date.now`/`new Date()` to `at` and
/// replace `Math.random` with a seeded LCG (mulberry32-style) — same
/// instant, same random stream, every run.
fn freeze_js(at: &str) -> String {
    format!(
        r#"(() => {{
  const T = Date.parse({});
  const R = Date;
  class F extends R {{
    constructor(...a) {{ super(...(a.length ? a : [T])); }}
    static now() {{ return T; }}
    static parse(s) {{ return R.parse(s); }}
    static UTC(...a) {{ return R.UTC(...a); }}
  }}
  window.Date = F;
  let s = 0x9E3779B9;
  Math.random = () => {{
    s = (s + 0x6D2B79F5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }};
}})()"#,
        serde_json::to_string(at).unwrap_or_else(|_| "\"2026-01-01\"".into())
    )
}

/// Whether the scenario contains a `check` claim on the `shot` subject
/// (`{"shot": "<stepId>"}`) — anywhere, including steps nested in
/// `group`/`loop` params and `useTemplate` bodies. Serialized-JSON walk
/// like [`scenario_uses_dialog`] so nesting can't hide it.
fn scenario_uses_shots(scenario: &Scenario) -> bool {
    fn contains_shot_marker(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Object(map) => {
                if map.contains_key("shot") {
                    return true;
                }
                map.values().any(contains_shot_marker)
            }
            serde_json::Value::Array(items) => items.iter().any(contains_shot_marker),
            _ => false,
        }
    }
    let steps = serde_json::to_value(&scenario.steps).unwrap_or(serde_json::Value::Null);
    let templates = serde_json::to_value(&scenario.templates).unwrap_or(serde_json::Value::Null);
    contains_shot_marker(&steps) || contains_shot_marker(&templates)
}

/// Whether the scenario contains a `do` step with `verb: dialog` or a
/// `check` claim on the `dialog` subject — anywhere, including steps nested
/// in `group`/`loop` params and `useTemplate` bodies. Implemented on the
/// serialized JSON so it stays correct no matter how steps nest.
fn scenario_uses_dialog(scenario: &Scenario) -> bool {
    fn contains_dialog_marker(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Object(map) => {
                if map.get("verb").and_then(|v| v.as_str()) == Some("dialog") {
                    return true;
                }
                if map.get("dialog").and_then(|v| v.as_bool()) == Some(true) {
                    return true;
                }
                map.values().any(contains_dialog_marker)
            }
            serde_json::Value::Array(items) => items.iter().any(contains_dialog_marker),
            _ => false,
        }
    }
    let steps = serde_json::to_value(&scenario.steps).unwrap_or(serde_json::Value::Null);
    let templates = serde_json::to_value(&scenario.templates).unwrap_or(serde_json::Value::Null);
    contains_dialog_marker(&steps) || contains_dialog_marker(&templates)
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
    let flags = parse_args_cli(args)?;
    if flags.all {
        return cli_all(
            &flags.filtered,
            flags.runs,
            flags.shard,
            flags.filter.as_deref(),
            &flags.tags,
            flags.report.as_deref(),
            flags.retry,
            flags.jobs,
        );
    }
    if flags.shard.is_some() {
        bail!("--shard requires --all");
    }
    if flags.filter.is_some() {
        bail!("--filter requires --all");
    }
    if !flags.tags.is_empty() {
        bail!("--tags requires --all");
    }
    if flags.report.is_some() {
        bail!("--report requires --all");
    }
    if flags.jobs > 1 {
        bail!("--jobs requires --all");
    }
    if flags.watch && flags.runs > 1 {
        bail!("--watch already re-runs on every save; --runs N inside it is redundant");
    }

    let mut parsed = parse_args(&flags.filtered)?;
    crate::run_auth::apply(&mut parsed)?;
    if flags.watch {
        return cli_watch(&parsed, &flags);
    }
    if flags.until_fail > 0 {
        // --until-fail N: re-run until the first failure or N clean passes —
        // flake reproduction. Exit 1 on the failing run (its dir stays for
        // audit/compare); exit 0 when every pass was clean.
        for attempt in 1..=flags.until_fail {
            eprintln!("[v2-replay] until-fail run {attempt}/{}", flags.until_fail);
            match run(&parsed) {
                Ok(summary) if !summary.ok => {
                    eprintln!(
                        "[v2-replay] failed on run {attempt} — flake reproduced; \
                         run dir kept for audit/compare"
                    );
                    return Ok(1);
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[v2-replay] until-fail run {attempt} errored: {e}");
                    return Err(e);
                }
            }
        }
        eprintln!(
            "[v2-replay] {} consecutive pass(es) — no flake observed",
            flags.until_fail
        );
        return Ok(0);
    }
    if flags.retry > 1 {
        // --retry N: re-run until a pass or N attempts spent. Each attempt is
        // its own replay dir, so a pass-after-retries leaves flake evidence in
        // `audit list`/`audit flaky` instead of hiding it.
        for attempt in 1..=flags.retry {
            eprintln!("[v2-replay] attempt {attempt}/{}", flags.retry);
            match run(&parsed) {
                Ok(summary) if summary.ok => {
                    if attempt > 1 {
                        eprintln!(
                            "[v2-replay] flaky — passed on attempt {attempt}/{} after {} failure(s)",
                            flags.retry,
                            attempt - 1
                        );
                    }
                    return Ok(0);
                }
                Ok(_) => {}
                Err(e) => eprintln!("[v2-replay] attempt {attempt}/{} errored: {e}", flags.retry),
            }
        }
        eprintln!("[v2-replay] failed all {} attempt(s)", flags.retry);
        return Ok(1);
    }
    let (code, _) = run_n(&parsed, flags.runs, None)?;
    Ok(code)
}

/// `replay <sid> --watch`: run once, then re-run every time the scenario
/// file's mtime changes. Poll-based (700ms) — no watcher dependency, works
/// the same on every OS. Ctrl-C exits. The save→replay cycle is the dev
/// loop; combine with `--auto-promote`/`--update-baselines` for a
/// self-healing golden refresh.
fn cli_watch(parsed: &RunOptions, flags: &CliFlags) -> Result<u8> {
    let (file, _) = resolve_source(&parsed.source)?;
    let stamp = |f: &std::path::Path| std::fs::metadata(f).and_then(|m| m.modified()).ok();
    let mut last = stamp(&file);
    eprintln!(
        "[v2-replay] watch: re-running when {} changes (ctrl-c to stop)",
        file.display()
    );
    let mut cycle = 0u32;
    loop {
        cycle += 1;
        eprintln!("[v2-replay] watch cycle {cycle}");
        let res = if flags.retry > 1 {
            // Same retry-until-pass ladder as the non-watch path.
            let mut code = 1u8;
            for attempt in 1..=flags.retry {
                match run(parsed) {
                    Ok(s) if s.ok => {
                        code = 0;
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("[v2-replay] attempt {attempt} errored: {e}"),
                }
            }
            Ok(code)
        } else {
            run_n(parsed, flags.runs, None).map(|(c, _)| c)
        };
        match res {
            Ok(0) => eprintln!("[v2-replay] cycle {cycle}: PASS"),
            Ok(_) => eprintln!("[v2-replay] cycle {cycle}: FAIL"),
            Err(e) => eprintln!("[v2-replay] cycle {cycle} errored: {e}"),
        }
        // Wait for the file's mtime to advance before the next cycle. A
        // save lands mid-write occasionally — re-check settles it.
        loop {
            std::thread::sleep(std::time::Duration::from_millis(700));
            let now = stamp(&file);
            if now != last {
                last = now;
                break;
            }
        }
    }
}

/// Replay `parsed` `runs` times; `label` prefixes the per-run banner.
/// Returns the exit code (0 iff every run is ok) and the last run's
/// summary for reporting (None when every run errored before producing
/// one).
fn run_n(parsed: &RunOptions, runs: u32, label: Option<&str>) -> Result<(u8, Option<RunSummary>)> {
    if runs <= 1 {
        let summary = run(parsed)?;
        let code = if summary.ok { 0 } else { 1 };
        return Ok((code, Some(summary)));
    }
    let mut all_ok = true;
    let mut last: Option<RunSummary> = None;
    for i in 1..=runs {
        eprintln!("[v2-replay]{} run {i}/{runs}", label.unwrap_or(""));
        match run(parsed) {
            Ok(summary) => {
                if !summary.ok {
                    all_ok = false;
                }
                last = Some(summary);
            }
            Err(e) => {
                eprintln!(
                    "[v2-replay]{} run {i}/{runs} errored: {e}",
                    label.unwrap_or("")
                );
                all_ok = false;
            }
        }
    }
    Ok((if all_ok { 0 } else { 1 }, last))
}

/// `replay --all`: every scenario under the scenarios root, optionally
/// sharded (`--shard k/n` keeps the sids whose sorted index % n == k-1)
/// and/or name-filtered (`--filter <substr>`).
/// Drop a caller's `--session`/`--session=` from forwarded args: under
/// --jobs>1 every scenario must drive its own browser session or parallel
/// workers would fight over one tab set.
fn strip_session_flag(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        if a == "--session" {
            it.next(); // swallow the value
            continue;
        }
        if a.starts_with("--session=") {
            continue;
        }
        out.push(a.clone());
    }
    out
}

/// One sid through its full run_n loop (parse → replay → retry ladder).
/// Returns the exit code + last summary, same contract as the serial path.
fn cli_all_one(
    filtered: &[String],
    sid: &str,
    session_override: Option<&str>,
    runs: u32,
    retry: u32,
) -> Result<(u8, Option<RunSummary>)> {
    let mut per = filtered.to_vec();
    if let Some(sess) = session_override {
        per.push("--session".to_string());
        per.push(sess.to_string());
    }
    per.push(sid.to_string());
    let parsed = parse_args(&per)?;
    // --retry under --all: re-run THIS scenario until pass or N attempts;
    // a pass on attempt 2 still leaves the earlier failing run dir as
    // flake evidence. (runs>1 && retry>1 is rejected at parse.)
    let mut attempt = 0u32;
    let res = loop {
        attempt += 1;
        let r = run_n(&parsed, runs, Some(&format!(" {sid}")))?;
        if r.0 == 0 || attempt >= retry {
            break r;
        }
        eprintln!("[v2-replay] {sid}: retry {attempt}/{retry}");
    };
    Ok(res)
}

#[allow(clippy::too_many_arguments)]
fn cli_all(
    filtered: &[String],
    runs: u32,
    shard: Option<(u32, u32)>,
    filter: Option<&str>,
    tags: &[String],
    report: Option<&Path>,
    retry: u32,
    jobs: u32,
) -> Result<u8> {
    let root = crate::paths::scenarios_root();
    let mut sids = crate::scenario_cli::all_sids(&root, filter);
    if !tags.is_empty() {
        sids.retain(|sid| scenario_has_any_tag(&root, sid, tags));
    }
    if let Some((k, n)) = shard {
        sids = sids
            .into_iter()
            .enumerate()
            .filter(|(i, _)| (*i as u32) % n == k - 1)
            .map(|(_, sid)| sid)
            .collect();
    }
    if sids.is_empty() {
        bail!("replay --all: no scenarios under {}", root.display());
    }
    eprintln!(
        "[v2-replay] --all{}: {} scenario(s){}",
        shard
            .map(|(k, n)| format!(" --shard {k}/{n}"))
            .unwrap_or_default(),
        sids.len(),
        filter
            .map(|f| format!(" matching {f:?}"))
            .unwrap_or_default(),
    );
    // Rows arrive in completion order under --jobs; report + failed list
    // are re-sorted by sid afterwards so output stays deterministic.
    let mut results: Vec<(String, u8, Option<RunSummary>)> = Vec::new();
    if jobs > 1 {
        let filtered = strip_session_flag(filtered);
        eprintln!(
            "[v2-replay] --jobs {jobs}: {} workers, per-scenario sessions (suite-<sid>)",
            jobs.min(sids.len() as u32)
        );
        let queue: Mutex<VecDeque<String>> = Mutex::new(sids.iter().cloned().collect());
        let results_mtx: Mutex<Vec<(String, u8, Option<RunSummary>)>> = Mutex::new(Vec::new());
        let worker_count = (jobs as usize).min(sids.len());
        thread::scope(|scope| {
            for _ in 0..worker_count {
                let queue = &queue;
                let results_mtx = &results_mtx;
                let filtered = &filtered;
                scope.spawn(move || loop {
                    let sid = match queue.lock().unwrap().pop_front() {
                        Some(s) => s,
                        None => break,
                    };
                    let session = format!("suite-{sid}");
                    let res = cli_all_one(filtered, &sid, Some(&session), runs, retry);
                    let (code, summary) = match res {
                        Ok(pair) => pair,
                        Err(e) => {
                            eprintln!("[v2-replay] {sid}: errored: {e:#}");
                            (1, None)
                        }
                    };
                    results_mtx.lock().unwrap().push((sid, code, summary));
                });
            }
        });
        results = results_mtx.into_inner().unwrap();
        results.sort_by(|a, b| a.0.cmp(&b.0));
    } else {
        for sid in &sids {
            match cli_all_one(filtered, sid, None, runs, retry) {
                Ok((code, summary)) => results.push((sid.clone(), code, summary)),
                Err(e) => {
                    eprintln!("[v2-replay] {sid}: errored: {e:#}");
                    results.push((sid.clone(), 1, None));
                }
            }
        }
    }
    let mut all_ok = true;
    let mut failed: Vec<String> = Vec::new();
    let mut rows: Vec<(String, Option<RunSummary>)> = Vec::new();
    for (sid, code, summary) in results {
        if code != 0 {
            all_ok = false;
            failed.push(sid.clone());
        }
        rows.push((sid, summary));
    }
    if let Some(path) = report {
        write_report(path, &rows)?;
        eprintln!("[v2-replay] report → {}", path.display());
    }
    eprintln!(
        "[v2-replay] --all done: {} passed, {} failed{}",
        sids.len() - failed.len(),
        failed.len(),
        if failed.is_empty() {
            String::new()
        } else {
            format!(": {}", failed.join(", "))
        }
    );
    Ok(if all_ok { 0 } else { 1 })
}

/// `--report` — a markdown verdict table for the suite run, the shape a
/// CI step drops into a PR comment: header counts, one row per scenario,
/// and a failing-scenario list at the bottom.
fn write_report(path: &Path, rows: &[(String, Option<RunSummary>)]) -> Result<()> {
    let passed = rows
        .iter()
        .filter(|(_, s)| s.as_ref().map(|s| s.ok).unwrap_or(false))
        .count();
    let failed = rows.len() - passed;
    let mut out = String::new();
    out.push_str("### agent-qa replay\n\n");
    if failed == 0 {
        out.push_str(&format!("✅ {passed}/{} scenarios pass.\n\n", rows.len()));
    } else {
        out.push_str(&format!(
            "❌ {failed}/{} scenarios fail ({passed} pass).\n\n",
            rows.len()
        ));
    }
    out.push_str("| scenario | result | steps |\n| --- | --- | --- |\n");
    for (sid, summary) in rows {
        let (verdict, steps) = match summary {
            Some(s) if s.ok => ("PASS".to_string(), format!("{}/{}", s.passed, s.total)),
            Some(s) => ("FAIL".to_string(), format!("{}/{}", s.passed, s.total)),
            None => ("ERROR".to_string(), "—".to_string()),
        };
        out.push_str(&format!("| `{sid}` | {verdict} | {steps} |\n"));
    }
    let failing: Vec<&str> = rows
        .iter()
        .filter(|(_, s)| !s.as_ref().map(|s| s.ok).unwrap_or(false))
        .map(|(sid, _)| sid.as_str())
        .collect();
    if !failing.is_empty() {
        out.push_str(&format!(
            "\nFailing: {} — run artifacts live under `scenarios/<sid>/replays/`.\n",
            failing.join(", ")
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(path, out).with_context(|| format!("write report {}", path.display()))
}

/// CLI-level flags peeled off before [`parse_args`] sees `args`.
#[derive(Debug)]
struct CliFlags {
    /// Remaining args (positional sid + per-run flags) for parse_args.
    filtered: Vec<String>,
    /// `--runs N` repeat count.
    runs: u32,
    /// `--all` — replay every scenario under the root.
    all: bool,
    /// `--shard k/n` — 1-based shard of the sorted sid list.
    shard: Option<(u32, u32)>,
    /// `--filter <substr>` — sid substring filter for --all.
    filter: Option<String>,
    /// `--tags <a,b>` — keep scenarios carrying any of these tags (OR).
    tags: Vec<String>,
    /// `--report <path>` — write a markdown verdict table for --all.
    report: Option<PathBuf>,
    /// `--jobs N` — run --all scenarios on N parallel workers, each with
    /// its own `suite-<sid>` browser session (any --session is ignored:
    /// parallel workers sharing one session would collide).
    jobs: u32,
    /// `--retry N` — re-run until a pass, at most N attempts (mutually
    /// exclusive with --runs). Under --all it applies per scenario.
    retry: u32,
    /// `--watch` — re-run whenever scenario.json's mtime changes
    /// (the save-driven dev loop; pairs with --auto-promote /
    /// --update-baselines for hands-free goldens).
    watch: bool,
    /// `--until-fail N` — re-run until the FIRST failure or N passes
    /// (the inverse of --retry): flake reproduction. The failing run
    /// dir stays on disk for `audit`/`compare` inspection.
    until_fail: u32,
}

/// Peel the CLI-level flags `--runs N`, `--retry N`, `--all`,
/// `--shard k/n`, `--filter <substr>`, `--tags`, `--report` off `args`;
/// the rest feed [`parse_args`].
fn parse_args_cli(args: &[String]) -> Result<CliFlags> {
    let mut filtered: Vec<String> = Vec::with_capacity(args.len());
    let mut runs: u32 = 1;
    let mut retry: u32 = 1;
    let mut all = false;
    let mut shard: Option<(u32, u32)> = None;
    let mut filter: Option<String> = None;
    let mut tags: Vec<String> = Vec::new();
    let mut report: Option<PathBuf> = None;
    let mut jobs: u32 = 1;
    let mut watch = false;
    let mut until_fail: u32 = 0;
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
            "--all" => all = true,
            "--shard" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--shard requires k/n (e.g. --shard 2/4)"))?;
                shard = Some(parse_shard(v)?);
            }
            s if s.starts_with("--shard=") => {
                shard = Some(parse_shard(&s["--shard=".len()..])?);
            }
            "--filter" => {
                filter = Some(
                    it.next()
                        .cloned()
                        .ok_or_else(|| anyhow!("--filter requires a value"))?,
                );
            }
            s if s.starts_with("--filter=") => {
                filter = Some(s["--filter=".len()..].to_string());
            }
            "--tags" => {
                let v = it
                    .next()
                    .cloned()
                    .ok_or_else(|| anyhow!("--tags requires a comma-separated value"))?;
                tags.extend(
                    v.split(',')
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty()),
                );
            }
            s if s.starts_with("--tags=") => {
                tags.extend(
                    s["--tags=".len()..]
                        .split(',')
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty()),
                );
            }
            "--jobs" => {
                let n = it
                    .next()
                    .ok_or_else(|| anyhow!("--jobs requires a positive integer"))?;
                jobs = n
                    .parse::<u32>()
                    .map_err(|_| anyhow!("--jobs must be a positive integer; got {n:?}"))?;
                if jobs == 0 {
                    bail!("--jobs must be >= 1");
                }
            }
            s if s.starts_with("--jobs=") => {
                let n = &s["--jobs=".len()..];
                jobs = n
                    .parse::<u32>()
                    .map_err(|_| anyhow!("--jobs must be a positive integer; got {n:?}"))?;
                if jobs == 0 {
                    bail!("--jobs must be >= 1");
                }
            }
            "--report" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--report requires a file path"))?;
                report = Some(PathBuf::from(v));
            }
            s if s.starts_with("--report=") => {
                report = Some(PathBuf::from(&s["--report=".len()..]));
            }
            "--retry" => {
                let n = it
                    .next()
                    .ok_or_else(|| anyhow!("--retry requires a positive integer"))?;
                retry = n
                    .parse::<u32>()
                    .map_err(|_| anyhow!("--retry must be a positive integer; got {n:?}"))?;
                if retry == 0 {
                    bail!("--retry must be >= 1");
                }
            }
            s if s.starts_with("--retry=") => {
                let n = &s["--retry=".len()..];
                retry = n
                    .parse::<u32>()
                    .map_err(|_| anyhow!("--retry must be a positive integer; got {n:?}"))?;
                if retry == 0 {
                    bail!("--retry must be >= 1");
                }
            }
            "--until-fail" => {
                let n = it
                    .next()
                    .ok_or_else(|| anyhow!("--until-fail requires a positive integer"))?;
                until_fail = n
                    .parse::<u32>()
                    .map_err(|_| anyhow!("--until-fail must be a positive integer; got {n:?}"))?;
                if until_fail == 0 {
                    bail!("--until-fail must be >= 1");
                }
            }
            s if s.starts_with("--until-fail=") => {
                let n = &s["--until-fail=".len()..];
                until_fail = n
                    .parse::<u32>()
                    .map_err(|_| anyhow!("--until-fail must be a positive integer; got {n:?}"))?;
                if until_fail == 0 {
                    bail!("--until-fail must be >= 1");
                }
            }
            "--watch" => watch = true,
            other => filtered.push(other.to_string()),
        }
    }
    if runs > 1 && retry > 1 {
        bail!("--runs and --retry are mutually exclusive (repeat-N vs until-pass)");
    }
    if until_fail > 0 && (runs > 1 || retry > 1) {
        bail!("--until-fail is mutually exclusive with --runs/--retry (until-fail vs repeat/until-pass)");
    }
    // AGENT_QA_REPLAY_ARGS: whitespace-separated flags applied BEFORE the
    // command line, so an explicit argv flag still overrides it. Lets
    // harnesses (golden libs, CI jobs) force flags like --har without
    // editing every replay call site.
    if let Ok(extra) = std::env::var("AGENT_QA_REPLAY_ARGS") {
        let extra = extra.trim();
        if !extra.is_empty() {
            let mut merged: Vec<String> = extra.split_whitespace().map(str::to_string).collect();
            merged.extend(filtered);
            filtered = merged;
            eprintln!("[v2-replay] AGENT_QA_REPLAY_ARGS applied: {extra}");
        }
    }
    Ok(CliFlags {
        filtered,
        runs,
        retry,
        all,
        shard,
        filter,
        tags,
        report,
        jobs,
        watch,
        until_fail,
    })
}

/// True when `<root>/<sid>/scenario.json` declares any of `tags`. A scenario
/// with no `tags` field — or one that fails to parse — never matches.
fn scenario_has_any_tag(root: &std::path::Path, sid: &str, tags: &[String]) -> bool {
    let bytes = match std::fs::read(root.join(sid).join("scenario.json")) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let doc: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(d) => d,
        Err(_) => return false,
    };
    doc.get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str())
                .any(|t| tags.iter().any(|want| want == t))
        })
        .unwrap_or(false)
}

/// `k/n` — k is 1-based and must be <= n; both must be positive.
fn parse_shard(v: &str) -> Result<(u32, u32)> {
    let (k, n) = v
        .split_once('/')
        .ok_or_else(|| anyhow!("--shard expects k/n (e.g. 2/4); got {v:?}"))?;
    let k: u32 = k
        .parse()
        .map_err(|_| anyhow!("--shard k must be a positive integer; got {v:?}"))?;
    let n: u32 = n
        .parse()
        .map_err(|_| anyhow!("--shard n must be a positive integer; got {v:?}"))?;
    if n == 0 || k == 0 || k > n {
        bail!("--shard expects 1 <= k <= n; got {v:?}");
    }
    Ok((k, n))
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
    let mut persona: Option<String> = None;
    let mut environment: Option<String> = None;
    let mut session: Option<String> = None;
    let mut heal_from_run: Option<String> = None;
    let mut headed = false;
    let mut dry_run = false;
    let mut no_sidecars = false;
    let mut quiet = false;
    let mut plain = false;
    let mut tag: Option<String> = None;
    let mut output_audit: Option<PathBuf> = None;
    let mut from_step: Option<String> = None;
    let mut until_step: Option<String> = None;
    let mut update_baselines = false;
    let mut keep_going = false;
    let mut record_video: Option<PathBuf> = None;
    let mut junit: Option<PathBuf> = None;
    let mut base_url: Option<String> = None;
    let mut auto_promote = false;
    let mut freeze: Option<String> = None;
    let mut har = false;
    let mut mock_from: Option<String> = None;
    let mut offline = false;
    let mut input_overrides: BTreeMap<String, String> = BTreeMap::new();
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--profile" => profile = it.next().cloned().or_else(|| bail_missing("--profile")),
            s if s.starts_with("--profile=") => profile = Some(s["--profile=".len()..].to_string()),
            "--persona" => persona = it.next().cloned().or_else(|| bail_missing("--persona")),
            s if s.starts_with("--persona=") => persona = Some(s["--persona=".len()..].to_string()),
            "--environment" | "--env" => {
                environment = it.next().cloned().or_else(|| bail_missing("--environment"))
            }
            s if s.starts_with("--environment=") => {
                environment = Some(s["--environment=".len()..].to_string())
            }
            s if s.starts_with("--env=") => environment = Some(s["--env=".len()..].to_string()),
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
            "--headed" => headed = true,
            "--headless" => headed = false,
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
            "--from" => from_step = it.next().cloned().or_else(|| bail_missing("--from")),
            s if s.starts_with("--from=") => from_step = Some(s["--from=".len()..].to_string()),
            "--until" => until_step = it.next().cloned().or_else(|| bail_missing("--until")),
            s if s.starts_with("--until=") => until_step = Some(s["--until=".len()..].to_string()),
            "--update-baselines" => update_baselines = true,
            "--keep-going" => keep_going = true,

            "--record-video" => record_video = Some(PathBuf::new()),
            s if s.starts_with("--record-video=") => {
                record_video = Some(PathBuf::from(&s["--record-video=".len()..]))
            }
            "--junit" => junit = Some(PathBuf::new()),
            s if s.starts_with("--junit=") => junit = Some(PathBuf::from(&s["--junit=".len()..])),
            "--base-url" => base_url = it.next().cloned().or_else(|| bail_missing("--base-url")),
            s if s.starts_with("--base-url=") => {
                base_url = Some(s["--base-url=".len()..].to_string())
            }
            "--auto-promote" => auto_promote = true,
            "--freeze" => freeze = it.next().cloned().or_else(|| bail_missing("--freeze")),
            s if s.starts_with("--freeze=") => freeze = Some(s["--freeze=".len()..].to_string()),
            "--har" => har = true,
            "--offline" => offline = true,
            "--mock-from" => mock_from = it.next().cloned().or_else(|| bail_missing("--mock-from")),
            s if s.starts_with("--mock-from=") => {
                mock_from = Some(s["--mock-from=".len()..].to_string())
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
        persona,
        environment,
        session_name,
        heal_from_run,
        headed,
        input_overrides,
        dry_run,
        no_sidecars,
        quiet,
        plain,
        tag,
        output_audit,
        from_step,
        until_step,
        update_baselines,
        keep_going,
        record_video,
        junit,
        base_url: normalize_base_url(base_url)?,
        auto_promote,
        freeze,
        har,
        mock_from,
        offline,
    })
}

/// `--base-url` must be a bare origin — `scheme://host[:port]`, no path or
/// query: a path here would silently corrupt every rewritten URL.
fn normalize_base_url(raw: Option<String>) -> Result<Option<String>> {
    let Some(u) = raw else { return Ok(None) };
    let u = u.trim_end_matches('/').to_string();
    let Some((scheme, hostport)) = u.split_once("://") else {
        bail!("--base-url expects scheme://host[:port], got {u:?}");
    };
    if scheme.is_empty() || hostport.is_empty() || hostport.contains(['/', '?']) {
        bail!("--base-url expects scheme://host[:port], got {u:?}");
    }
    Ok(Some(u))
}

/// Narrow a flattened step list to the `--from`/`--until` dispatch window.
/// `--from` keeps steps from the first id match onward; `--until` keeps
/// steps through the first id match (inclusive). Unknown ids and inverted
/// windows are hard errors — silently dispatching the wrong slice would be
/// worse than bailing.
fn apply_step_window(
    steps: Vec<Step>,
    from: Option<&String>,
    until: Option<&String>,
) -> Result<Vec<Step>> {
    if from.is_none() && until.is_none() {
        return Ok(steps);
    }
    let ids: Vec<String> = steps.iter().map(|s| s.id().to_string()).collect();
    let lo = match from {
        Some(f) => ids.iter().position(|id| id == f).ok_or_else(|| {
            anyhow!(
                "--from {f:?}: no step with that id (have {})",
                ids.join(", ")
            )
        })?,
        None => 0,
    };
    let hi = match until {
        Some(u) => ids.iter().position(|id| id == u).ok_or_else(|| {
            anyhow!(
                "--until {u:?}: no step with that id (have {})",
                ids.join(", ")
            )
        })?,
        None => steps.len().saturating_sub(1),
    };
    if lo > hi {
        bail!(
            "--from {:?} is after --until {:?} — empty window",
            from.unwrap(),
            until.unwrap()
        );
    }
    Ok(steps[lo..=hi].to_vec())
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
                  [--profile <p>] [--persona <id>] [--environment <id>]
                  [--session <name>]
                  [--param name=value] [-p name=value]
                  [--heal-from-run <runId>] [--dry-run]
                  [--no-sidecars] [--quiet | -q] [--plain]
                  [--tag <label>] [--output-audit <path>]
                  [--from <stepId>] [--until <stepId>]
                  [--update-baselines] [--freeze <iso>] [--base-url <origin>]
                  [--runs <N>] [--keep-going] [--junit [path]] [--record-video [path]]

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
--persona <id>           Replay as a persona — reads
                         <scenarios>/_personas/<id>/persona.json, injects
                         its credentials (literal or vault: refs resolved
                         via $VAULT_ADDR) into env, and sets the profile
                         (session <profile>-session) so env.open's
                         useProfile op authenticates. CLI mirror of the
                         workbench persona picker.
--environment <id>, --env <id>
                         Replay against a named environment —
                         <scenarios>/_environments/<id>/environment.json.
                         Its params + baseUrl merge under --param
                         overrides; auth.config lands as AGENT_QA_ENV_*
                         vars; auth.creds merge under the persona's
                         credentials. With --persona and no flag, the
                         default (or sole) environment is used.
--dry-run                load + validate + mint run id + write the
                         initial audit row, but skip env.open /
                         env.close and step dispatch entirely. The
                         run directory still gets created and
                         audit.json carries 'SUMMARY: 0/N (DRY-RUN)'.
--runs <N>               Repeat the replay N times in one invocation
                         (each run mints its own runId). Useful for
                         flake detection. Exit 0 iff every run is OK.
--watch                  Re-run every time scenario.json's mtime
                         changes — the save-driven dev loop. Combine
                         with --auto-promote/--update-baselines for
                         hands-free golden refreshes. Ctrl-C to stop.
--until-fail <N>         Re-run until the FIRST failure or N clean
                         passes — flake reproduction (the inverse of
                         --retry). The failing run dir stays on disk
                         for audit/compare. Exit 1 on a failure,
                         0 when all N passes were clean.
--all                    Replay every scenario under the scenarios root
                         (sorted sid order), printing a per-run banner
                         plus a final pass/fail rollup. Combines with
                         every flag except a positional sid.
--shard k/n              With --all: run only the sids whose index in
                         the sorted list mod n == k-1 (k is 1-based).
                         For CI matrix jobs, e.g. shard 1/4 + 2/4 + …
--filter <substr>        With --all: keep sids containing <substr>
                         (case-insensitive).
--tags <a,b>             With --all: keep scenarios whose `tags` list
                         contains any of the comma-separated names (OR).
--report <path>          With --all: write a markdown verdict table
                         (per-scenario PASS/FAIL + step counts) — the
                         shape a CI step drops into a PR comment.
--jobs N                 With --all: run scenarios on N parallel
                         workers. Each scenario gets its own browser
                         session (suite-<sid>) — a user --session is
                         ignored. Progress lines interleave; --report
                         output stays sorted.
                         (case-insensitive).

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
                         convenience.
--update-baselines       After the run, mint every captured screenshot
                         into <sid>/baselines/ (same copy shot-accept
                         performs). Runs even when shot claims fail —
                         intentional UI changes are the re-mint case.
                         Skips with a warning when the run captured no
                         screenshots (e.g. --no-sidecars).
--har                    Record a HAR file for the run and write
                         <sid>/replays/<runId>/network.har — request +
                         response bodies for DevTools/Charles-level
                         inspection. network.json (when present) stays
                         the lightweight status list; the HAR is the
                         deep dive.
--mock-from <runId>      Seed network stubs from <runId>'s
                         network.har (record it with --har first): the
                         page's fetch/XHR calls get the recorded
                         status+body — a hermetic, offline-capable
                         replay. Rules install via a page init script
                         on fresh sessions (covers page-load fetches),
                         else re-apply after every navigation
--freeze <iso>           Pin Date.now()/new Date() to <iso> and replace
                         Math.random with a seeded LCG via a page init
                         script — rendered timestamps and random ordering
                         can't flake a golden diff. Fresh sessions apply
                         it on every navigation; warm sessions get the
                         current document only
--record-video [path]    Record the browser to video for the whole run
                         (needs ffmpeg). Bare flag → <run>/run.webm;
                         =<path> picks the file (.webm/.mp4). Covers
                         env.open navigation through env.close.
--auto-promote           Self-healing write-back: when the run passed AND
                         auto-heal corrected locators this run, apply those
                         patches to scenario.json (the hash-guarded
                         heal-promote --apply path). A stale-hash refusal
                         is a warning, never a failure.
--base-url <origin>      Retarget the run onto another deploy (e.g. a
                         PR preview): every env nav url + goto literal
                         rooted at the recorded origin is rewritten to
                         <origin> (scheme://host[:port] only). Claim
                         patterns are left untouched — use inputs for
                         scenario-authored variability.
--keep-going             Dispatch every step even after a failure
                         (default: stop at the first). Later steps often
                         cascade-fail from the broken page state, but a
                         repair sweep wants the complete failure list in
                         one run's audit rather than one re-run per step.

--junit [path]           Write the run's terminal step outcomes as JUnit
                         XML — one <testcase> per step. Bare flag writes
                         <run>/junit.xml; --junit=<path> writes that
                         path. Any CI's standard test-result ingestion
                         (Jenkins/GitLab/Azure/GitHub reporters) renders
                         the replay like a unit-test run.
--offline                Reject every fetch/XHR that matches no mock rule
                         instead of reaching the real backend. With
                         --mock-from that's the full hermetic guarantee;
                         alone it stubs everything (static-page replays)."
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::env;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    // env mutation isn't thread-safe; serialize via the shared lock.
    use crate::test_util::lock_env;

    #[test]
    fn parse_shard_validates_bounds() {
        assert_eq!(parse_shard("1/4").unwrap(), (1, 4));
        assert_eq!(parse_shard("4/4").unwrap(), (4, 4));
        assert!(parse_shard("0/4").is_err());
        assert!(parse_shard("5/4").is_err());
        assert!(parse_shard("1/0").is_err());
        assert!(parse_shard("x").is_err());
        assert!(parse_shard("1/").is_err());
    }

    #[test]
    fn cli_shard_without_all_errors() {
        let args: Vec<String> = ["sid-x", "--shard", "1/2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(cli(&args).is_err());
    }

    #[test]
    fn write_report_renders_verdict_table() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("report.md");
        let rows = vec![
            (
                "alpha".to_string(),
                Some(RunSummary {
                    passed: 3,
                    total: 3,
                    ok: true,
                }),
            ),
            (
                "beta".to_string(),
                Some(RunSummary {
                    passed: 1,
                    total: 4,
                    ok: false,
                }),
            ),
            ("gamma".to_string(), None),
        ];
        write_report(&path, &rows).unwrap();
        let md = fs::read_to_string(&path).unwrap();
        assert!(md.contains("❌ 2/3 scenarios fail (1 pass)"));
        assert!(md.contains("| `alpha` | PASS | 3/3 |"));
        assert!(md.contains("| `beta` | FAIL | 1/4 |"));
        assert!(md.contains("| `gamma` | ERROR | — |"));
        assert!(md.contains("Failing: beta, gamma"));
    }

    #[test]
    fn cli_report_without_all_errors() {
        let args: Vec<String> = ["sid-x", "--report", "/tmp/r.md"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(cli(&args).is_err());
    }

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
            persona: None,
            environment: None,
            session_name: "test".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
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
    fn replay_clears_capture_once_and_resets_persistent_emulation() {
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
            persona: None,
            environment: None,
            session_name: "test".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
        };
        run(&opts).unwrap();

        // The reused-session hygiene at run start: request log cleared once,
        // and the emulation toggles that outlive a run (offline, headers)
        // reset so a previous scenario can't poison this one.
        let ab = fs::read_to_string(&log).unwrap();
        assert_eq!(
            ab.matches("network requests --clear").count(),
            1,
            "got: {ab}"
        );
        assert!(ab.contains("--session test set offline off"), "got: {ab}");
        assert!(ab.contains("--session test set headers {}"), "got: {ab}");
        clear_fake_browser();
    }

    #[test]
    fn replay_viewport_step_invokes_agent_browser_viewport() {
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
            "schema": "scenario/2",
            "id": "viewport-smoke",
            "intent": "resize then assert",
            "env": {
                "open": [
                    { "kind": "nav", "url": "https://example.com/", "intent": "land" }
                ]
            },
            "steps": [
                { "id": "s1", "intent": "resize to mobile", "kind": "do",
                  "verb": "viewport", "params": { "width": 375, "height": 812 } }
            ]
        }"#,
        )
        .unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            persona: None,
            environment: None,
            session_name: "vp".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: true,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
        };
        let summary = run(&opts).unwrap();
        assert!(summary.ok);
        let ab = fs::read_to_string(&log).unwrap();
        assert!(
            ab.contains("--session vp set viewport 375 812"),
            "got: {ab}"
        );
        clear_fake_browser();
    }

    #[test]
    fn replay_file_chooser_step_arms_hook_with_payload() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        install_fake_browser(work.path(), &log);

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        fs::write(jdir.join("note.txt"), "hi").unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(
            &jfile,
            r#"{
            "schema": "scenario/2",
            "id": "chooser-smoke",
            "intent": "arm chooser then click",
            "env": {
                "open": [
                    { "kind": "nav", "url": "https://example.com/", "intent": "land" }
                ]
            },
            "steps": [
                { "id": "s1", "intent": "arm", "kind": "do",
                  "verb": "fileChooser", "params": { "files": ["note.txt"] } }
            ]
        }"#,
        )
        .unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            persona: None,
            environment: None,
            session_name: "fc".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: true,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
        };
        let summary = run(&opts).unwrap();
        assert!(summary.ok);
        let ab = fs::read_to_string(&log).unwrap();
        assert!(ab.contains("--session fc eval"), "got: {ab}");
        assert!(ab.contains("__aqFileChooser"), "got: {ab}");
        // base64("hi") = "aGk=" in the armed payload
        assert!(ab.contains("aGk="), "got: {ab}");
        clear_fake_browser();
    }

    #[test]
    fn replay_emulate_step_invokes_agent_browser_set() {
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
            "schema": "scenario/2",
            "id": "emulate-smoke",
            "intent": "apply emulate params",
            "env": {
                "open": [
                    { "kind": "nav", "url": "https://example.com/", "intent": "land" }
                ]
            },
            "steps": [
                { "id": "s1", "intent": "emulate", "kind": "do",
                  "verb": "emulate", "params": {
                    "device": "iPhone 12",
                    "geo": { "lat": 37.7749, "lng": -122.4194 },
                    "colorScheme": "dark", "reducedMotion": true,
                    "headers": { "X-Test": "yes" } } }
            ]
        }"#,
        )
        .unwrap();

        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "em".into(),
            persona: None,
            environment: None,
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: true,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
        };
        let summary = run(&opts).unwrap();
        assert!(summary.ok);
        let ab = fs::read_to_string(&log).unwrap();
        assert!(
            ab.contains("--session em set device iPhone 12"),
            "got: {ab}"
        );
        assert!(ab.contains("set geo 37.7749 -122.4194"), "got: {ab}");
        assert!(ab.contains("set media dark reduced-motion"), "got: {ab}");
        assert!(ab.contains("set headers"), "got: {ab}");
        assert!(ab.contains("X-Test"), "got: {ab}");
        clear_fake_browser();
    }

    #[test]
    fn emulate_unknown_key_errors() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        install_fake_browser(work.path(), &work.path().join("ab.log"));
        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(
            &jfile,
            r#"{
            "schema": "scenario/2",
            "id": "emulate-bad",
            "intent": "bad key",
            "env": { "open": [ { "kind": "nav", "url": "https://example.com/", "intent": "land" } ] },
            "steps": [
                { "id": "s1", "intent": "emulate", "kind": "do",
                  "verb": "emulate", "params": { "timzone": "UTC" } }
            ]
        }"#,
        )
        .unwrap();
        let opts = RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            session_name: "em".into(),
            persona: None,
            environment: None,
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: true,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
        };
        let err = run(&opts).unwrap_err();
        assert!(
            err.to_string().contains("unknown emulate key"),
            "got: {err}"
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
            persona: None,
            environment: None,
            session_name: "x".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
        };
        let err = format!("{:#}", run(&opts).unwrap_err());
        assert!(err.contains("schema error"), "got: {err}");
        clear_fake_browser();
    }

    /// Fake agent-browser for auto-heal tests: the recorded name "Save"
    /// never resolves (find/eval miss), the collect probe reports the live
    /// name "Save changes", and an eval mentioning the corrected name
    /// activates. Mirrors the miss→probe→retry lifecycle end-to-end.
    fn install_heal_fake_browser(dir: &Path, log: &Path) -> PathBuf {
        let body = format!(
            "#!/bin/sh\necho \"$@\" >> '{log}'\n\
case \"$*\" in\n\
  *__aqCollectNames*) echo '{{\"names\":[\"Save changes\"]}}' ;;\n\
  *'Save changes'*) echo 'true' ;;\n\
  *eval*) echo 'false' ;;\n\
  *find*) exit 1 ;;\n\
  *snapshot*) echo '' ;;\n\
esac\nexit 0\n",
            log = log.display()
        );
        let bin = write_exec(dir, "agent-browser", &body);
        env::set_var(browser::BIN_ENV, &bin);
        browser::_reset_bin_cache_for_tests();
        bin
    }

    fn heal_scenario() -> &'static str {
        r#"{
            "schema": "scenario/2",
            "id": "heal-smoke",
            "intent": "click save",
            "env": {
                "open": [
                    { "kind": "nav", "url": "https://example.com/", "intent": "land" }
                ]
            },
            "steps": [
                { "id": "s1", "intent": "click Save", "kind": "do",
                  "verb": "click",
                  "on": { "role": "button", "name": "Save" } }
            ]
        }"#
    }

    fn heal_opts(jfile: PathBuf) -> RunOptions {
        RunOptions {
            source: ScenarioSource::Path(jfile),
            profile: None,
            persona: None,
            environment: None,
            session_name: "sx".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: true,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
        }
    }

    #[test]
    fn replay_auto_heals_locator_drift() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        install_heal_fake_browser(work.path(), &work.path().join("ab.log"));

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, heal_scenario()).unwrap();

        let summary = run(&heal_opts(jfile)).unwrap();
        assert_eq!(summary.passed, 1);
        assert!(summary.ok);

        let run_id = fs::read_to_string(jdir.join("replays").join("latest.txt")).unwrap();
        let run_id = run_id.trim();

        // Suggested patch written for heal-promote.
        let patch: serde_json::Value = serde_json::from_slice(
            &fs::read(
                jdir.join("replays")
                    .join(run_id)
                    .join("diffs")
                    .join("s1.patch.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(patch["schema"], "heal-patch/v1");
        assert_eq!(patch["newLocator"]["name"], "Save changes");
        assert!(patch["scenarioContentHash"].is_string());

        // Audit trail: heal.jsonl row + audit.autoHealed.
        let rows =
            fs::read_to_string(jdir.join("replays").join(run_id).join("heal.jsonl")).unwrap();
        let row: serde_json::Value = serde_json::from_str(rows.trim()).unwrap();
        assert_eq!(row["mode"], "locator-correction");
        assert_eq!(row["stepId"], "s1");
        assert_eq!(row["to"], "Save changes");

        let audit: serde_json::Value = serde_json::from_slice(
            &fs::read(jdir.join("replays").join(run_id).join("audit.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(audit["autoHealed"], serde_json::json!(["s1"]));
        clear_fake_browser();
    }

    #[test]
    fn replay_no_heal_env_fails_hard() {
        let _g = lock_env();
        env::set_var("AGENT_QA_NO_HEAL", "1");
        let work = TempDir::new().unwrap();
        install_heal_fake_browser(work.path(), &work.path().join("ab.log"));

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, heal_scenario()).unwrap();

        let err = format!("{:#}", run(&heal_opts(jfile)).unwrap_err());
        assert!(err.contains("step s1"), "got: {err}");

        let run_id = fs::read_to_string(jdir.join("replays").join("latest.txt")).unwrap();
        let run_dir = jdir.join("replays").join(run_id.trim());
        assert!(
            !run_dir.join("diffs").exists(),
            "no patch may be written when heal is disabled"
        );
        let audit: serde_json::Value =
            serde_json::from_slice(&fs::read(run_dir.join("audit.json")).unwrap()).unwrap();
        assert_eq!(audit["exitCode"], 1);
        assert!(audit.get("autoHealed").is_none());
        env::remove_var("AGENT_QA_NO_HEAL");
        clear_fake_browser();
    }

    #[test]
    fn replay_heal_strict_fails_passing_run() {
        let _g = lock_env();
        env::set_var("AGENT_QA_HEAL_STRICT", "1");
        let work = TempDir::new().unwrap();
        install_heal_fake_browser(work.path(), &work.path().join("ab.log"));

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, heal_scenario()).unwrap();

        let summary = run(&heal_opts(jfile)).unwrap();
        assert!(!summary.ok, "strict mode: healed run must fail");
        assert_eq!(summary.passed, 1, "steps still pass — drift was healed");

        let run_id = fs::read_to_string(jdir.join("replays").join("latest.txt")).unwrap();
        let audit: serde_json::Value = serde_json::from_slice(
            &fs::read(jdir.join("replays").join(run_id.trim()).join("audit.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(audit["exitCode"], 1);
        assert_eq!(audit["autoHealed"], serde_json::json!(["s1"]));
        env::remove_var("AGENT_QA_HEAL_STRICT");
        clear_fake_browser();
    }

    /// A do-step failure that is no healable miss (a raw-css click that
    /// exits non-zero) gets probed for a value rejection — here the
    /// field-level constraint probe reports the invalid input.
    #[test]
    fn replay_classifies_field_rejection() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        let body = format!(
            "#!/bin/sh\necho \"$@\" >> '{log}'\n\
case \"$*\" in\n\
  *__aqCollectNames*) echo '{{\"names\":[\"Cancel\"]}}' ;;\n\
  *__aqRejectProbe*) echo '[]' ;;\n\
  *__aqFieldProbe*) echo '[\"email: Please fill out this field\"]' ;;\n\
  *click*) exit 1 ;;\n\
  *snapshot*) echo '' ;;\n\
esac\nexit 0\n",
            log = log.display()
        );
        let bin = write_exec(work.path(), "agent-browser", &body);
        env::set_var(browser::BIN_ENV, &bin);
        browser::_reset_bin_cache_for_tests();

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(
            &jfile,
            r##"{
            "schema": "scenario/2",
            "id": "reject-smoke",
            "intent": "submit",
            "env": { "open": [ { "kind": "nav", "url": "https://example.com/", "intent": "land" } ] },
            "steps": [
                { "id": "s1", "intent": "submit", "kind": "do", "verb": "click",
                  "on": { "raw": { "kind": "css", "value": "#submit" }, "reason": "t" } }
            ]
        }"##,
        )
        .unwrap();

        run(&heal_opts(jfile)).unwrap_err();
        let run_id = fs::read_to_string(jdir.join("replays").join("latest.txt")).unwrap();
        let rows = fs::read_to_string(jdir.join("replays").join(run_id.trim()).join("heal.jsonl"))
            .unwrap();
        let row: serde_json::Value = serde_json::from_str(rows.trim()).unwrap();
        assert_eq!(row["mode"], "value-rejection");
        assert!(row["rationale"]
            .as_str()
            .unwrap()
            .contains("email: Please fill out this field"));
        clear_fake_browser();
    }

    /// A callGql 4xx carries the refusal in the error itself — no DOM
    /// evidence needed; the rejection is still classified and persisted.
    #[test]
    fn replay_classifies_gql_client_rejection() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        let body = format!(
            "#!/bin/sh\necho \"$@\" >> '{log}'\n\
case \"$*\" in\n\
  *__aqRejectProbe*) echo '[]' ;;\n\
  *__aqFieldProbe*) echo '[]' ;;\n\
  *fetch*) echo '{{\"status\":422,\"body\":\"{{\\\"error\\\":\\\"email taken\\\"}}\"}}' ;;\n\
  *snapshot*) echo '' ;;\n\
esac\nexit 0\n",
            log = log.display()
        );
        let bin = write_exec(work.path(), "agent-browser", &body);
        env::set_var(browser::BIN_ENV, &bin);
        browser::_reset_bin_cache_for_tests();

        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(
            &jfile,
            r#"{
            "schema": "scenario/2",
            "id": "gql-reject-smoke",
            "intent": "submit mutation",
            "env": { "open": [ { "kind": "nav", "url": "https://example.com/", "intent": "land" } ] },
            "steps": [
                { "id": "s1", "intent": "mutation", "kind": "do", "verb": "callGql",
                  "params": { "url": "https://example.com/graphql", "query": "{ noop }" } }
            ]
        }"#,
        )
        .unwrap();

        run(&heal_opts(jfile)).unwrap_err();
        let run_id = fs::read_to_string(jdir.join("replays").join("latest.txt")).unwrap();
        let rows = fs::read_to_string(jdir.join("replays").join(run_id.trim()).join("heal.jsonl"))
            .unwrap();
        let row: serde_json::Value = serde_json::from_str(rows.trim()).unwrap();
        assert_eq!(row["mode"], "value-rejection");
        assert!(row["rationale"].as_str().unwrap().contains("HTTP 422"));
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
            persona: None,
            environment: None,
            session_name: "sx".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
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
            persona: None,
            environment: None,
            session_name: "x".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
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
    fn parse_args_keep_going_flag() {
        assert!(!parse_args(&["./j.json".into()]).unwrap().keep_going);
        assert!(
            parse_args(&["./j.json".into(), "--keep-going".into()])
                .unwrap()
                .keep_going
        );
    }

    #[test]
    fn parse_args_record_video() {
        // bare flag → empty-path sentinel (runner resolves to <run>/run.webm)
        let o = parse_args(&["./j.json".into(), "--record-video".into()]).unwrap();
        assert_eq!(o.record_video.as_deref(), Some(std::path::Path::new("")));
        let o = parse_args(&["./j.json".into(), "--record-video=/tmp/x.mp4".into()]).unwrap();
        assert_eq!(
            o.record_video.as_deref(),
            Some(std::path::Path::new("/tmp/x.mp4"))
        );
        assert!(parse_args(&["./j.json".into()])
            .unwrap()
            .record_video
            .is_none());
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
        let (scope, params) =
            resolve_inputs(Some(&declared), &overrides, &BTreeMap::new()).unwrap();
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
        let err = resolve_inputs(Some(&declared), &overrides, &BTreeMap::new())
            .unwrap_err()
            .to_string();
        assert!(err.contains("not declared") || err.contains("no inputs"));
    }

    #[test]
    fn resolve_inputs_local_file_is_the_last_resort() {
        let mut declared = BTreeMap::new();
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
        declared.insert(
            "with_default".to_string(),
            InputDecl {
                ty: InputType::String,
                default: Some(serde_json::json!("decl")),
                sensitive: None,
                items: None,
                properties: None,
                description: None,
            },
        );
        let mut local = BTreeMap::new();
        local.insert("secret".to_string(), serde_json::json!("hunter2"));
        local.insert(
            "with_default".to_string(),
            serde_json::json!("local-ignored"),
        );
        let (scope, params) = resolve_inputs(Some(&declared), &BTreeMap::new(), &local).unwrap();
        // Declared default still wins over the local file; the file only
        // covers inputs with no default and no --param.
        assert_eq!(
            scope.get("with_default").unwrap(),
            &serde_json::json!("decl")
        );
        assert_eq!(scope.get("secret").unwrap(), &serde_json::json!("hunter2"));
        let secret_param = params.iter().find(|p| p.name == "secret").unwrap();
        assert_eq!(secret_param.value, serde_json::json!("[REDACTED]"));
        assert_eq!(secret_param.source, ParameterSource::Local);
        // --param still beats the local file
        let mut overrides = BTreeMap::new();
        overrides.insert("secret".to_string(), "cli-wins".to_string());
        let (scope, _) = resolve_inputs(Some(&declared), &overrides, &local).unwrap();
        assert_eq!(scope.get("secret").unwrap(), &serde_json::json!("cli-wins"));
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
    fn parse_args_cli_strips_runs_and_parses() {
        let f = parse_args_cli(&["./j.json".into(), "--runs".into(), "5".into()]).unwrap();
        assert_eq!(f.runs, 5);
        assert_eq!(f.retry, 1);
        assert!(!f.all && f.shard.is_none() && f.filter.is_none());
        let opts = parse_args(&f.filtered).unwrap();
        assert!(matches!(opts.source, ScenarioSource::Path(_)));
    }

    #[test]
    fn parse_args_cli_eq_form() {
        let f = parse_args_cli(&["./j.json".into(), "--runs=3".into()]).unwrap();
        assert_eq!(f.runs, 3);
        let f = parse_args_cli(&["./j.json".into(), "--retry=2".into()]).unwrap();
        assert_eq!(f.retry, 2);
        // --runs and --retry are mutually exclusive.
        parse_args_cli(&["./j.json".into(), "--runs=2".into(), "--retry=2".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_cli_default_is_one() {
        let f = parse_args_cli(&["./j.json".into()]).unwrap();
        assert_eq!(f.runs, 1);
        assert_eq!(f.retry, 1);
        assert!(!f.watch);
    }

    #[test]
    fn parse_args_cli_until_fail() {
        let f = parse_args_cli(&["./j.json".into(), "--until-fail".into(), "3".into()]).unwrap();
        assert_eq!(f.until_fail, 3);
        assert_eq!(f.retry, 1);
        let f = parse_args_cli(&["./j.json".into(), "--until-fail=5".into()]).unwrap();
        assert_eq!(f.until_fail, 5);
        // zero / non-int rejected; mutually exclusive with --runs / --retry
        parse_args_cli(&["./j.json".into(), "--until-fail".into(), "0".into()]).unwrap_err();
        parse_args_cli(&["./j.json".into(), "--until-fail=x".into()]).unwrap_err();
        parse_args_cli(&[
            "./j.json".into(),
            "--until-fail=2".into(),
            "--retry=2".into(),
        ])
        .unwrap_err();
        parse_args_cli(&[
            "./j.json".into(),
            "--until-fail=2".into(),
            "--runs=2".into(),
        ])
        .unwrap_err();
        let f = parse_args_cli(&["./j.json".into()]).unwrap();
        assert_eq!(f.until_fail, 0);
    }

    #[test]
    fn parse_args_cli_watch_flag() {
        let f = parse_args_cli(&["./j.json".into(), "--watch".into()]).unwrap();
        assert!(f.watch);
        assert_eq!(f.runs, 1);
        assert_eq!(f.retry, 1);
        // --watch + --runs is rejected at cli(), not parse time
        let f = parse_args_cli(&["./j.json".into(), "--watch".into(), "--runs=2".into()]).unwrap();
        assert!(f.watch && f.runs == 2);
    }

    #[test]
    fn parse_args_cli_rejects_zero_and_non_int() {
        parse_args_cli(&["./j.json".into(), "--runs".into(), "0".into()]).unwrap_err();
        parse_args_cli(&["./j.json".into(), "--runs".into(), "x".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_cli_all_shard_filter() {
        let f = parse_args_cli(&[
            "--all".into(),
            "--shard".into(),
            "2/4".into(),
            "--filter=login".into(),
            "--quiet".into(),
        ])
        .unwrap();
        assert!(f.all);
        assert_eq!(f.shard, Some((2, 4)));
        assert_eq!(f.filter.as_deref(), Some("login"));
        assert_eq!(f.filtered, vec!["--quiet".to_string()]);
    }

    #[test]
    fn parse_args_cli_tag_repeatable() {
        let f = parse_args_cli(&[
            "--all".into(),
            "--tags".into(),
            "smoke,checkout".into(),
            "--tags=nightly".into(),
        ])
        .unwrap();
        assert!(f.all);
        assert_eq!(
            f.tags,
            vec![
                "smoke".to_string(),
                "checkout".to_string(),
                "nightly".to_string()
            ]
        );
    }

    #[test]
    fn cli_tag_without_all_errors() {
        let args: Vec<String> = ["sid-x", "--tags", "smoke"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(cli(&args).is_err());
    }

    #[test]
    fn parse_args_cli_jobs() {
        let f = parse_args_cli(&["--all".into(), "--jobs".into(), "4".into()]).unwrap();
        assert_eq!(f.jobs, 4);
        let f = parse_args_cli(&["--all".into(), "--jobs=2".into()]).unwrap();
        assert_eq!(f.jobs, 2);
        parse_args_cli(&["--all".into(), "--jobs".into(), "0".into()]).unwrap_err();
        parse_args_cli(&["--all".into(), "--jobs".into(), "x".into()]).unwrap_err();
        let args: Vec<String> = ["sid-x", "--jobs", "2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(cli(&args).is_err(), "--jobs without --all must bail");
    }

    #[test]
    fn strip_session_flag_drops_both_forms() {
        let out = strip_session_flag(&[
            "--session".into(),
            "shared".into(),
            "--quiet".into(),
            "--session=other".into(),
            "--tags".into(),
            "smoke".into(),
        ]);
        assert_eq!(out, vec!["--quiet", "--tags", "smoke"]);
    }

    #[test]
    fn cli_all_jobs_runs_scenarios_in_parallel_sessions() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let log = work.path().join("ab.log");
        install_fake_browser(work.path(), &log);
        std::env::set_var(paths::SCENARIOS_DIR_ENV, work.path());
        for sid in ["za", "zb"] {
            let d = work.path().join(sid);
            fs::create_dir_all(&d).unwrap();
            fs::write(
                d.join("scenario.json"),
                minimal_scenario().replace("\"smoke\"", &format!("\"{sid}\"")),
            )
            .unwrap();
        }
        // A user --session is ignored under --jobs — each worker needs its
        // own browser session or the two runs would share one browser.
        let args: Vec<String> = ["--all", "--jobs", "2", "--session", "shared"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let code = cli(&args).unwrap();
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        clear_fake_browser();
        assert_eq!(code, 0);
        assert!(work.path().join("za/replays").is_dir());
        assert!(work.path().join("zb/replays").is_dir());
        let inv = fs::read_to_string(&log).unwrap();
        assert!(
            inv.contains("--session suite-za"),
            "za must run on its own session: {inv}"
        );
        assert!(
            inv.contains("--session suite-zb"),
            "zb must run on its own session: {inv}"
        );
        assert!(
            !inv.contains("--session shared"),
            "user --session must be stripped under --jobs: {inv}"
        );
    }

    #[test]
    fn scenario_has_any_tag_matches_declared_only() {
        let dir = tempfile::tempdir().unwrap();
        let sid = dir.path().join("s1");
        fs::create_dir_all(&sid).unwrap();
        fs::write(
            sid.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"s1","intent":"x","tags":["smoke","ci"],"steps":[]}"#,
        )
        .unwrap();
        let root = dir.path();
        let smoke = vec!["smoke".to_string()];
        let nope = vec!["nightly".to_string()];
        assert!(scenario_has_any_tag(root, "s1", &smoke));
        assert!(!scenario_has_any_tag(root, "s1", &nope));
        // no tags field → never matches
        fs::write(
            sid.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"s1","intent":"x","steps":[]}"#,
        )
        .unwrap();
        assert!(!scenario_has_any_tag(root, "s1", &smoke));
        // malformed json → never matches
        fs::write(sid.join("scenario.json"), b"not json").unwrap();
        assert!(!scenario_has_any_tag(root, "s1", &smoke));
    }

    #[test]
    fn replay_args_env_applies_and_argv_overrides() {
        std::env::set_var("AGENT_QA_REPLAY_ARGS", "--har --dry-run");
        let flags = parse_args_cli(&["./j.json".into()]).unwrap();
        std::env::remove_var("AGENT_QA_REPLAY_ARGS");
        let opts = parse_args(&flags.filtered).unwrap();
        assert!(opts.har, "--har from AGENT_QA_REPLAY_ARGS applied");
        assert!(opts.dry_run, "--dry-run from the env applied");
    }

    #[test]
    fn parse_retry_parses_both_forms() {
        let f = parse_args_cli(&["./j.json".into(), "--retry".into(), "3".into()]).unwrap();
        assert_eq!((f.runs, f.retry), (1, 3));
        let f = parse_args_cli(&["./j.json".into(), "--retry=2".into()]).unwrap();
        assert_eq!(f.retry, 2);
        let f = parse_args_cli(&["./j.json".into()]).unwrap();
        assert_eq!((f.runs, f.retry), (1, 1));
        parse_args_cli(&["./j.json".into(), "--retry".into(), "0".into()]).unwrap_err();
        parse_args_cli(&["./j.json".into(), "--retry".into(), "x".into()]).unwrap_err();
    }

    #[test]
    fn retry_cli_returns_1_after_all_attempts_error() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        install_fake_browser(work.path(), &work.path().join("ab.log"));
        // A scenario path that never resolves: run() errors every attempt.
        let missing = work.path().join("nope").join("scenario.json");
        let code = cli(&[
            missing.to_string_lossy().to_string(),
            "--retry".into(),
            "3".into(),
        ])
        .unwrap();
        assert_eq!(code, 1);
        clear_fake_browser();
    }

    #[test]
    fn retry_cli_exits_0_on_first_pass() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        install_fake_browser(work.path(), &work.path().join("ab.log"));
        let jdir = work.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        let jfile = jdir.join("scenario.json");
        fs::write(&jfile, minimal_scenario()).unwrap();
        let code = cli(&[
            jfile.to_string_lossy().to_string(),
            "--retry".into(),
            "3".into(),
        ])
        .unwrap();
        assert_eq!(code, 0);
        // exactly one attempt — no extra run dirs minted
        let runs = fs::read_dir(jdir.join("replays"))
            .map(|d| d.flatten().filter(|e| e.path().is_dir()).count())
            .unwrap_or(0);
        assert_eq!(runs, 1);
        clear_fake_browser();
    }

    #[test]
    fn runs_and_retry_are_mutually_exclusive() {
        parse_args_cli(&[
            "./j.json".into(),
            "--runs".into(),
            "2".into(),
            "--retry".into(),
            "3".into(),
        ])
        .unwrap_err();
        // retry alone with runs at its default is fine
        parse_args_cli(&["./j.json".into(), "--retry".into(), "3".into()]).unwrap();
    }

    #[test]
    fn scenario_uses_dialog_detects_do_and_check_including_nested() {
        let j = serde_json::json!({
            "schema": "scenario/2", "id": "d", "intent": "x",
            "steps": [
                { "id": "s0", "intent": "open", "kind": "do", "verb": "goto",
                  "value": { "from": "literal", "literal": "https://x" } },
                { "id": "g1", "intent": "grp", "kind": "do", "verb": "group",
                  "params": { "steps": [
                      { "id": "sg", "intent": "resolve", "kind": "do", "verb": "dialog",
                        "params": { "action": "accept" } }
                  ]}}
            ]
        });
        let s: Scenario = serde_json::from_value(j).unwrap();
        assert!(scenario_uses_dialog(&s));

        let j = serde_json::json!({
            "schema": "scenario/2", "id": "d", "intent": "x",
            "steps": [
                { "id": "c1", "intent": "dialog gone", "kind": "check",
                  "claim": { "subject": { "dialog": true }, "predicate": "notExists" } }
            ]
        });
        let s: Scenario = serde_json::from_value(j).unwrap();
        assert!(scenario_uses_dialog(&s));

        let j = serde_json::json!({
            "schema": "scenario/2", "id": "d", "intent": "x",
            "steps": [
                { "id": "s0", "intent": "reload", "kind": "do", "verb": "reload" }
            ]
        });
        let s: Scenario = serde_json::from_value(j).unwrap();
        assert!(!scenario_uses_dialog(&s));
    }

    #[test]
    fn scenario_shot_masks_unions_and_dedups_nested_claims() {
        let body = serde_json::json!([
            {
                "id": "s9", "intent": "visual a", "kind": "check",
                "claim": { "subject": { "shot": "s1", "mask": [".ts", ".live"] }, "predicate": "matches" }
            },
            {
                "id": "g1", "intent": "group", "kind": "do", "verb": "group",
                "params": { "steps": [
                    {
                        "id": "sg1", "intent": "nested", "kind": "check",
                        "claim": { "subject": { "shot": "sg0", "mask": [".ts", ".ad"] }, "predicate": "matches" }
                    }
                ]}
            },
            {
                "id": "s10", "intent": "no mask", "kind": "check",
                "claim": { "subject": { "shot": "s2" }, "predicate": "matches" }
            },
            {
                "id": "s11", "intent": "not a shot", "kind": "check",
                "claim": { "subject": { "url": true }, "predicate": "exists" }
            }
        ]);
        let scenario: Scenario = serde_json::from_value(serde_json::json!({
            "schema": "scenario/2", "id": "t", "intent": "t", "steps": body
        }))
        .unwrap();
        assert_eq!(scenario_shot_masks(&scenario), vec![".ts", ".live", ".ad"]);
        assert!(scenario_has_shot_claims(&scenario));
    }

    #[test]
    fn scenario_has_shot_claims_false_for_non_visual() {
        let scenario: Scenario = serde_json::from_value(serde_json::json!({
            "schema": "scenario/2", "id": "t", "intent": "t",
            "steps": [
                { "id": "s1", "intent": "go", "kind": "do", "verb": "goto", "value": {"from": "literal", "literal": "https://x"} },
                { "id": "s2", "intent": "url", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
            ]
        }))
        .unwrap();
        assert!(!scenario_has_shot_claims(&scenario));
    }

    #[test]
    fn scenario_uses_shots_detects_claim_including_nested() {
        let j = serde_json::json!({
            "schema": "scenario/2", "id": "d", "intent": "x",
            "steps": [
                { "id": "s0", "intent": "open", "kind": "do", "verb": "goto",
                  "value": { "from": "literal", "literal": "https://x" } },
                { "id": "g1", "intent": "grp", "kind": "do", "verb": "group",
                  "params": { "steps": [
                      { "id": "sc", "intent": "visual", "kind": "check",
                        "claim": { "subject": { "shot": "s0" }, "predicate": "matches" } }
                  ]}}
            ]
        });
        let s: Scenario = serde_json::from_value(j).unwrap();
        assert!(scenario_uses_shots(&s));

        let j = serde_json::json!({
            "schema": "scenario/2", "id": "d", "intent": "x",
            "steps": [
                { "id": "c1", "intent": "url", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
            ]
        });
        let s: Scenario = serde_json::from_value(j).unwrap();
        assert!(!scenario_uses_shots(&s));
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
            persona: None,
            environment: None,
            session_name: "sx".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: true,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
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
            persona: None,
            environment: None,
            session_name: "sx".into(),
            heal_from_run: Some("rPRIOR".into()),
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
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
    fn parse_args_persona_and_environment_flags() {
        let opts = parse_args(&[
            "mysid".into(),
            "--persona".into(),
            "admin".into(),
            "--environment".into(),
            "staging".into(),
        ])
        .unwrap();
        assert_eq!(opts.persona.as_deref(), Some("admin"));
        assert_eq!(opts.environment.as_deref(), Some("staging"));
        // = forms + --env alias
        let opts = parse_args(&[
            "mysid".into(),
            "--persona=viewer".into(),
            "--env=prod".into(),
        ])
        .unwrap();
        assert_eq!(opts.persona.as_deref(), Some("viewer"));
        assert_eq!(opts.environment.as_deref(), Some("prod"));
        // neither set by default
        let opts = parse_args(&["mysid".into()]).unwrap();
        assert!(opts.persona.is_none() && opts.environment.is_none());
    }

    #[test]
    fn parse_args_base_url_flag_and_origin_validation() {
        let opts =
            parse_args(&["x".into(), "--base-url".into(), "https://pr-7.io/".into()]).unwrap();
        assert_eq!(opts.base_url.as_deref(), Some("https://pr-7.io")); // trailing / stripped
        let opts = parse_args(&["x".into(), "--base-url=http://localhost:3000".into()]).unwrap();
        assert_eq!(opts.base_url.as_deref(), Some("http://localhost:3000"));
        assert!(parse_args(&["x".into()]).unwrap().base_url.is_none());
        // Path/query/no-scheme are rejected — they'd corrupt rewrites.
        parse_args(&["x".into(), "--base-url".into(), "https://a.io/p".into()]).unwrap_err();
        parse_args(&["x".into(), "--base-url".into(), "a.io".into()]).unwrap_err();
        parse_args(&["x".into(), "--base-url".into(), "https://a.io/?q=1".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_unknown_flag_errors() {
        let err = parse_args(&["--what".into(), "x".into()]).unwrap_err();
        assert!(err.to_string().contains("unknown flag"));
    }

    #[test]
    fn step_window_until_keeps_steps_through_match() {
        let steps: Vec<Step> = (0..4)
            .map(|i| parse_step(&format!(r#"{{"id":"s{i}","intent":"x","kind":"do","verb":"goto","value":{{"from":"literal","literal":"u"}}}}"#)))
            .collect();
        let out = apply_step_window(steps, None, Some(&"s2".to_string())).unwrap();
        let ids: Vec<&str> = out.iter().map(|s| s.id()).collect();
        assert_eq!(ids, ["s0", "s1", "s2"]);
    }

    #[test]
    fn step_window_from_keeps_steps_from_match() {
        let steps: Vec<Step> = (0..4)
            .map(|i| parse_step(&format!(r#"{{"id":"s{i}","intent":"x","kind":"do","verb":"goto","value":{{"from":"literal","literal":"u"}}}}"#)))
            .collect();
        let out = apply_step_window(steps, Some(&"s2".to_string()), None).unwrap();
        let ids: Vec<&str> = out.iter().map(|s| s.id()).collect();
        assert_eq!(ids, ["s2", "s3"]);
    }

    #[test]
    fn step_window_combined_slices_and_bails_on_bad_ids() {
        let mk = || -> Vec<Step> {
            (0..4)
                .map(|i| parse_step(&format!(r#"{{"id":"s{i}","intent":"x","kind":"do","verb":"goto","value":{{"from":"literal","literal":"u"}}}}"#)))
                .collect()
        };
        let out =
            apply_step_window(mk(), Some(&"s1".to_string()), Some(&"s2".to_string())).unwrap();
        let ids: Vec<&str> = out.iter().map(|s| s.id()).collect();
        assert_eq!(ids, ["s1", "s2"]);
        assert!(apply_step_window(mk(), Some(&"nope".to_string()), None).is_err());
        assert!(apply_step_window(mk(), None, Some(&"nope".to_string())).is_err());
        assert!(apply_step_window(mk(), Some(&"s3".to_string()), Some(&"s1".to_string())).is_err());
        // No window = pass-through.
        assert_eq!(apply_step_window(mk(), None, None).unwrap().len(), 4);
    }

    #[test]
    fn parse_args_from_until_flags() {
        let opts = parse_args(&[
            "x".into(),
            "--from".into(),
            "s2".into(),
            "--until".into(),
            "s4".into(),
        ])
        .unwrap();
        assert_eq!(opts.from_step.as_deref(), Some("s2"));
        assert_eq!(opts.until_step.as_deref(), Some("s4"));
        let opts = parse_args(&["x".into(), "--until=s3".into()]).unwrap();
        assert_eq!(opts.until_step.as_deref(), Some("s3"));
        assert_eq!(opts.from_step, None);
    }

    #[test]
    fn parse_args_freeze_flag() {
        let opts = parse_args(&["x".into(), "--freeze".into(), "2026-01-01".into()]).unwrap();
        assert_eq!(opts.freeze.as_deref(), Some("2026-01-01"));
        let opts = parse_args(&["x".into(), "--freeze=2026-06-30T00:00Z".into()]).unwrap();
        assert_eq!(opts.freeze.as_deref(), Some("2026-06-30T00:00Z"));
        let opts = parse_args(&["x".into()]).unwrap();
        assert_eq!(opts.freeze, None);
    }

    #[test]
    fn freeze_js_pins_clock_and_seeds_rng() {
        let js = freeze_js("2026-01-01T00:00:00Z");
        assert!(
            js.contains("Date.parse(\"2026-01-01T00:00:00Z\")"),
            "got: {js}"
        );
        assert!(js.contains("static now()"), "got: {js}");
        assert!(js.contains("Math.random ="), "got: {js}");
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
            persona: None,
            environment: None,
            session_name: "evt".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
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
            persona: None,
            environment: None,
            session_name: "evtf".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
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
            persona: None,
            environment: None,
            session_name: "evtn".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: true,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
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
            persona: None,
            environment: None,
            session_name: "evtd".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: true,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
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
            persona: None,
            environment: None,
            session_name: "s".into(),
            heal_from_run: None,
            headed: false,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet,
            plain,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
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

    #[test]
    fn parse_args_auto_promote_flag() {
        let opts = parse_args(&["sid".into(), "--auto-promote".into()]).unwrap();
        assert!(opts.auto_promote);
        let opts = parse_args(&["sid".into()]).unwrap();
        assert!(!opts.auto_promote);
    }
}
