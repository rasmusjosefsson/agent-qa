//! External surfaces (desktop, and later ios / android).
//!
//! The browser runner never calls this module. `runner::run` checks
//! `surface` once after schema validation and returns here only when
//! the value is present and not `"web"`. Provider resolution, process
//! spawn, and accessibility snapshots all live on this side of that
//! check.
//!
//! The provider is a separate process speaking JSON over stdio. This
//! crate does not ship a desktop, iOS, or Android driver. See
//! `docs/surfaces.md`.

mod host;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Map, Value as Json};
use std::sync::OnceLock;

use crate::runner::{self, RunOptions, RunSummary};
use crate::scenario::{
    Claim, ClaimSubject, Locator, NameMatch, Predicate, Scenario, Step, SurfaceTarget, Verb,
};
use crate::sidecar::{
    self, hash_scenario_bytes, mint_run_id, prepare_run_root, update_latest_pointer, RunAudit,
    RunPaths, SidecarKind, StepEvent,
};
use crate::value::{resolve_value, value_to_string, ValueScope};

/// `true` when replay must leave the browser runner. Missing and
/// `"web"` are the built-in surface.
pub(crate) fn is_external(parsed: &Json) -> bool {
    match parsed.get("surface").and_then(Json::as_str) {
        None | Some("web") => false,
        Some(_) => true,
    }
}

pub(crate) fn replay(
    opts: &RunOptions,
    scenario_file: &Path,
    scenario_dir: &Path,
    bytes: &[u8],
    parsed: Json,
) -> Result<RunSummary> {
    let scenario: Scenario = serde_json::from_value(parsed).context("parse scenario")?;
    let surface = scenario
        .surface
        .clone()
        .unwrap_or_else(|| "web".to_string());
    if scenario.steps.is_empty() {
        bail!(
            "{}: scenario has no steps — nothing to replay",
            scenario_file.display()
        );
    }
    warn_ignored_browser_flags(opts, &surface);
    if scenario
        .env
        .as_ref()
        .and_then(|e| e.open.as_ref())
        .is_some()
    {
        eprintln!("[surface] {surface}: env.open is web setup and was not run");
    }

    let local = runner::load_local_inputs(scenario_dir);
    let (resolved_inputs, audit_params) =
        runner::resolve_inputs(scenario.inputs.as_ref(), &opts.input_overrides, &local)?;
    let mut scope = ValueScope::new(resolved_inputs);
    let flat = runner::flatten_steps_with_scope(
        &scenario.steps,
        scenario.templates.as_ref(),
        Some(&scope.inputs),
        Some(scenario_dir),
    )?;
    let flat = apply_window(&flat, opts.from_step.as_deref(), opts.until_step.as_deref())?;

    let hash = hash_scenario_bytes(bytes);
    let run_id = mint_run_id(opts.profile.as_deref());
    let run = prepare_run_root(scenario_dir, &run_id)?;
    let started_at = now_iso();
    let mut audit = RunAudit {
        schema: RunAudit::SCHEMA_ID.to_string(),
        run_id: run.run_id.clone(),
        scenario_id: scenario.id.clone(),
        started_at,
        finished_at: None,
        exit_code: None,
        summary: None,
        profile: opts.profile.clone(),
        session_name: Some(opts.session_name.clone()),
        scenario_content_hash: hash,
        parameters: if audit_params.is_empty() {
            None
        } else {
            Some(audit_params)
        },
        heal_overrides_applied: None,
        auto_healed: None,
        tag: opts.tag.clone(),
        quarantined: None,
        window_from: opts.from_step.clone(),
        window_until: opts.until_step.clone(),
    };
    sidecar::write_run_audit(&run, &audit)?;

    let total = u32::try_from(flat.len()).unwrap_or(u32::MAX);
    if opts.dry_run {
        let summary = RunSummary {
            passed: 0,
            total,
            ok: true,
            quarantined: false,
        };
        return finish(
            opts,
            &run,
            &mut audit,
            &summary,
            &format!("SUMMARY: 0/{total} (DRY-RUN)"),
            None,
        );
    }

    let provider = resolve_provider(&surface)?;
    let pong = ping(&provider)?;
    if !pong.surfaces.iter().any(|s| s == &surface) {
        bail!(
            "surface provider `{}` ({}) does not serve {surface:?} — it serves {}",
            provider.display(),
            pong.name,
            if pong.surfaces.is_empty() {
                "(none)".to_string()
            } else {
                pong.surfaces.join(", ")
            }
        );
    }
    eprintln!(
        "[surface] {surface}: provider {} ({})",
        provider.display(),
        pong.name
    );

    let target = scenario.target.clone().unwrap_or(SurfaceTarget {
        app: None,
        window: None,
        device: None,
    });
    if target.app.is_none() && target.device.is_none() {
        bail!(
            "surface {surface:?} needs target.app or target.device — the provider has nothing to launch"
        );
    }
    let target_body = target_json(&target);
    invoke(
        &provider,
        "open",
        json!({ "surface": surface, "target": target_body }),
        Duration::from_secs(60),
    )
    .context("surface open")?;

    let mut summary = RunSummary {
        passed: 0,
        total,
        ok: true,
        quarantined: false,
    };
    let mut first_failure: Option<String> = None;
    let mut stop = false;
    let mut out = Emit {
        run: &run,
        idx: 0,
        total,
        no_sidecars: opts.no_sidecars,
    };
    for (i, step) in flat.iter().enumerate() {
        if stop {
            break;
        }
        let idx = u32::try_from(i + 1).unwrap_or(u32::MAX);
        out.idx = idx;
        if let Step::Check {
            enabled: Some(false),
            ..
        } = step
        {
            emit(&out, step, "skip", None, None, None);
            continue;
        }
        if gated_off(step, &opts.run_for) {
            emit(&out, step, "skip", None, None, None);
            continue;
        }
        if step.context().and_then(|c| c.when.as_ref()).is_some() {
            let msg = format!(
                "step {}: conditional when-blocks are not replayed on the {surface} surface",
                step.id()
            );
            summary.ok = false;
            first_failure.get_or_insert(msg.clone());
            emit(&out, step, "fail", Some(msg), None, None);
            if !opts.keep_going {
                stop = true;
            }
            continue;
        }
        let started = Instant::now();
        let outcome = dispatch_step(
            &Dispatch {
                provider: &provider,
                surface: &surface,
                target: &target_body,
                run: &run,
                scenario_dir,
                no_sidecars: opts.no_sidecars,
            },
            step,
            &mut scope,
        );
        let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match outcome {
            Ok(evidence) => {
                summary.passed += 1;
                emit(
                    &out,
                    step,
                    "pass",
                    None,
                    evidence.snapshot_rel,
                    evidence.screenshot_rel,
                );
                eprintln!(
                    "[surface] {surface} {idx}/{total} {} ok ({ms}ms)",
                    step.id()
                );
            }
            Err(e) => {
                summary.ok = false;
                let msg = format!("{:#}", e);
                eprintln!(
                    "[surface] {surface} {idx}/{total} {} FAIL: {msg}",
                    step.id()
                );
                first_failure.get_or_insert_with(|| format!("step {}: {msg}", step.id()));
                emit(&out, step, "fail", Some(msg), None, None);
                if !opts.keep_going {
                    stop = true;
                }
            }
        }
    }

    if let Err(e) = invoke(
        &provider,
        "close",
        json!({ "surface": surface, "target": target_body }),
        Duration::from_secs(20),
    ) {
        eprintln!("[surface] {surface}: close: {e:#}");
    }

    let today = now_iso();
    let today = today.get(..10).unwrap_or("");
    summary.quarantined = scenario
        .quarantine
        .as_ref()
        .is_some_and(|q| q.active() && !q.expired(today));
    let line = summary.render();
    finish(opts, &run, &mut audit, &summary, &line, first_failure)
}

struct Evidence {
    snapshot_rel: Option<String>,
    screenshot_rel: Option<String>,
}

struct Dispatch<'a> {
    provider: &'a Path,
    surface: &'a str,
    target: &'a Json,
    run: &'a RunPaths,
    scenario_dir: &'a Path,
    no_sidecars: bool,
}

fn dispatch_step(ctx: &Dispatch<'_>, step: &Step, scope: &mut ValueScope) -> Result<Evidence> {
    match step {
        Step::Do {
            verb,
            on,
            value,
            save_as,
            params,
            ..
        } => {
            let wire = wire_verb(verb).ok_or_else(|| {
                anyhow!(
                    "verb {verb:?} is a web-surface action and is not sent to the {} provider",
                    ctx.surface
                )
            })?;
            let on_body = match on {
                Some(loc) => Some(locator_wire(loc)?),
                None => None,
            };
            let resolved = match value {
                Some(v) => Some(resolve_value(v, scope)?),
                None => None,
            };
            let value_text = resolved.as_ref().map(value_to_string);
            let obs = invoke(
                ctx.provider,
                "act",
                json!({
                    "surface": ctx.surface,
                    "target": ctx.target,
                    "verb": wire,
                    "on": on_body,
                    "value": value_text,
                    "params": params.clone().unwrap_or_default(),
                }),
                Duration::from_secs(60),
            )?;
            let obs: Observation = serde_json::from_value(obs).context("act response")?;
            if let Some(name) = save_as {
                let saved = value_text
                    .clone()
                    .or_else(|| matched_text(&obs, on.as_ref()).ok().flatten());
                if let Some(saved) = saved {
                    scope.saved_steps.insert(name.clone(), Json::String(saved));
                }
            }
            Ok(write_evidence(ctx.run, step.id(), &obs, ctx.no_sidecars)?)
        }
        Step::Check { claim, .. } => {
            let obs = invoke(
                ctx.provider,
                "observe",
                json!({ "surface": ctx.surface, "target": ctx.target }),
                Duration::from_secs(30),
            )?;
            let obs: Observation = serde_json::from_value(obs).context("observe response")?;
            let evidence = write_evidence(ctx.run, step.id(), &obs, ctx.no_sidecars)?;
            check_claim(claim, &obs, ctx.run, ctx.scenario_dir)?;
            Ok(evidence)
        }
    }
}

fn write_evidence(
    run: &RunPaths,
    step_id: &str,
    obs: &Observation,
    no_sidecars: bool,
) -> Result<Evidence> {
    if no_sidecars {
        return Ok(Evidence {
            snapshot_rel: None,
            screenshot_rel: None,
        });
    }
    let mut snapshot_rel = None;
    if !obs.snapshot.is_empty() {
        sidecar::write_step_sidecar(
            run,
            SidecarKind::Snapshots,
            step_id,
            obs.snapshot.as_bytes(),
        )?;
        snapshot_rel = Some(format!("snapshots/{step_id}.txt"));
    }
    let mut screenshot_rel = None;
    if let Some(b64) = obs.screenshot_png.as_deref().filter(|s| !s.is_empty()) {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .context("screenshotPng is not base64")?;
        sidecar::write_step_sidecar(run, SidecarKind::Screenshots, step_id, &bytes)?;
        screenshot_rel = Some(format!("screenshots/{step_id}.png"));
    }
    Ok(Evidence {
        snapshot_rel,
        screenshot_rel,
    })
}

fn check_claim(
    claim: &Claim,
    obs: &Observation,
    run: &RunPaths,
    scenario_dir: &Path,
) -> Result<()> {
    match &claim.subject {
        ClaimSubject::Element {
            element, attribute, ..
        } => check_element(
            element,
            attribute.as_deref(),
            &claim.predicate,
            claim.value.as_ref(),
            obs,
        ),
        ClaimSubject::Shot { shot, clip, .. } => {
            if clip.is_some() {
                bail!("shot clip is a web-surface locator");
            }
            if claim.predicate != Predicate::Matches {
                bail!("shot subject only supports predicate 'matches'");
            }
            check_shot(shot, claim.tolerance.as_ref(), run, scenario_dir)
        }
        ClaimSubject::Domshot { domshot, skip } => {
            if claim.predicate != Predicate::Matches {
                bail!("domshot subject only supports predicate 'matches'");
            }
            check_domshot(domshot, skip, run, scenario_dir)
        }
        other => bail!("claim {other:?} is a web-surface check"),
    }
}

fn check_element(
    locator: &Locator,
    attribute: Option<&str>,
    predicate: &Predicate,
    expected: Option<&Json>,
    obs: &Observation,
) -> Result<()> {
    let hits = matching_elements(locator, obs)?;
    let present = !hits.is_empty();
    match predicate {
        Predicate::Exists | Predicate::IsVisible => {
            if present {
                Ok(())
            } else {
                bail!("element not found")
            }
        }
        Predicate::NotExists | Predicate::IsHidden => {
            if present {
                bail!("element is present")
            } else {
                Ok(())
            }
        }
        Predicate::IsEnabled
        | Predicate::IsDisabled
        | Predicate::IsChecked
        | Predicate::IsUnchecked
        | Predicate::Equals
        | Predicate::Contains
        | Predicate::Matches
        | Predicate::StartsWith
        | Predicate::EndsWith => {
            let el = single(&hits)?;
            match predicate {
                Predicate::IsEnabled => {
                    if el.enabled {
                        Ok(())
                    } else {
                        bail!("element is disabled")
                    }
                }
                Predicate::IsDisabled => {
                    if el.enabled {
                        bail!("element is enabled")
                    } else {
                        Ok(())
                    }
                }
                Predicate::IsChecked => {
                    if el.checked {
                        Ok(())
                    } else {
                        bail!("element is not checked")
                    }
                }
                Predicate::IsUnchecked => {
                    if el.checked {
                        bail!("element is checked")
                    } else {
                        Ok(())
                    }
                }
                _ => {
                    let actual = element_fact(el, attribute);
                    let want = expected
                        .map(value_to_string)
                        .ok_or_else(|| anyhow!("predicate {predicate:?} needs a value"))?;
                    text_predicate(predicate, &actual, &want)
                }
            }
        }
        other => bail!("element claim does not support predicate {other:?} on this surface"),
    }
}

fn element_fact(el: &ObservedElement, attribute: Option<&str>) -> String {
    match attribute {
        Some("focused") => el.focused.to_string(),
        Some("enabled") => el.enabled.to_string(),
        Some("checked") => el.checked.to_string(),
        Some("value") => el.value.clone().unwrap_or_default(),
        Some("name") => el.name.clone().unwrap_or_default(),
        _ => el
            .text
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| el.value.clone())
            .or_else(|| el.name.clone())
            .unwrap_or_default(),
    }
}

fn text_predicate(predicate: &Predicate, actual: &str, want: &str) -> Result<()> {
    let ok = match predicate {
        Predicate::Equals => actual == want,
        Predicate::Contains => actual.contains(want),
        Predicate::StartsWith => actual.starts_with(want),
        Predicate::EndsWith => actual.ends_with(want),
        Predicate::Matches => {
            let re = Regex::new(want).with_context(|| format!("bad matches regex {want:?}"))?;
            re.is_match(actual)
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        bail!("element text {actual:?} failed {predicate:?} {want:?}")
    }
}

fn matching_elements<'a>(
    locator: &Locator,
    obs: &'a Observation,
) -> Result<Vec<&'a ObservedElement>> {
    let (role, name) = match locator {
        Locator::Role(r) => {
            let name = match &r.name {
                None => None,
                Some(NameMatch::Plain(s)) => Some(s.as_str()),
                Some(_) => bail!("surface providers match a plain accessible name"),
            };
            (r.role.as_str(), name)
        }
        Locator::Raw(_) => bail!("raw locators are a web-surface escape hatch"),
    };
    Ok(obs
        .elements
        .iter()
        .filter(|el| el.role.eq_ignore_ascii_case(role))
        .filter(|el| match name {
            None => true,
            Some(want) => el.name.as_deref() == Some(want),
        })
        .collect())
}

fn single<'a>(hits: &[&'a ObservedElement]) -> Result<&'a ObservedElement> {
    match hits {
        [one] => Ok(one),
        [] => bail!("element not found"),
        many => bail!("element matches {} nodes", many.len()),
    }
}

fn matched_text(obs: &Observation, on: Option<&Locator>) -> Result<Option<String>> {
    let Some(loc) = on else {
        return Ok(None);
    };
    let hits = matching_elements(loc, obs)?;
    let el = single(&hits)?;
    Ok(Some(element_fact(el, None)))
}

fn check_shot(
    shot: &str,
    tolerance: Option<&BTreeMap<String, Json>>,
    run: &RunPaths,
    scenario_dir: &Path,
) -> Result<()> {
    let tol = tolerance
        .and_then(|t| t.get("pixels"))
        .and_then(Json::as_f64)
        .unwrap_or(0.01);
    let baseline = scenario_dir.join("baselines").join(format!("{shot}.png"));
    if !baseline.is_file() {
        bail!(
            "no baseline for shot '{shot}' at {} — mint one with: agent-qa shot-accept <sid> --steps {shot}",
            baseline.display()
        );
    }
    let current = run.run_root.join("screenshots").join(format!("{shot}.png"));
    if !current.is_file() {
        bail!("no screenshot for step '{shot}' in this run");
    }
    let a = crate::compare::screenshots::decode_png(&baseline)?;
    let b = crate::compare::screenshots::decode_png(&current)?;
    if a.dimensions() != b.dimensions() {
        bail!(
            "shot '{shot}' size {}x{} vs {}x{}",
            a.width(),
            a.height(),
            b.width(),
            b.height()
        );
    }
    let (frac, diff) = crate::compare::screenshots::pixel_diff(&a, &b);
    if frac > tol {
        let dir = run.run_root.join("shots-diff");
        fs::create_dir_all(&dir)?;
        diff.save(dir.join(format!("{shot}.diff.png")))
            .context("write shot diff")?;
        bail!("shot '{shot}' differs ({frac:.4} > {tol})");
    }
    Ok(())
}

fn check_domshot(
    step_id: &str,
    skip: &[String],
    run: &RunPaths,
    scenario_dir: &Path,
) -> Result<()> {
    let baseline = scenario_dir
        .join("baselines")
        .join(format!("{step_id}.snap.txt"));
    if !baseline.is_file() {
        bail!(
            "no baseline for domshot '{step_id}' at {} — mint one with: agent-qa domshot-accept <sid> --steps {step_id}",
            baseline.display()
        );
    }
    let current = run
        .run_root
        .join("snapshots")
        .join(format!("{step_id}.txt"));
    if !current.is_file() {
        bail!("no snapshot for step '{step_id}' in this run");
    }
    let left = normalize_tree(&fs::read_to_string(&baseline)?, skip);
    let right = normalize_tree(&fs::read_to_string(&current)?, skip);
    if left == right {
        return Ok(());
    }
    let diff = similar::TextDiff::from_lines(&left, &right)
        .unified_diff()
        .context_radius(2)
        .header("baseline", "replay")
        .to_string();
    let dir = run.run_root.join("domshots-diff");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(format!("{step_id}.diff.txt")), &diff)?;
    bail!("domshot '{step_id}' differs");
}

fn normalize_tree(text: &str, skip: &[String]) -> String {
    let mut out = String::new();
    for line in text.lines() {
        if skip
            .iter()
            .any(|pat| Regex::new(pat).ok().is_some_and(|re| re.is_match(line)))
        {
            continue;
        }
        let line = ref_re().replace_all(line, "@e");
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn ref_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(r"@[A-Za-z0-9_]+:e\d+|@e\d+").expect("ref regex"))
}

fn locator_wire(loc: &Locator) -> Result<Json> {
    match loc {
        Locator::Role(r) => {
            let name = match &r.name {
                None => None,
                Some(NameMatch::Plain(s)) => Some(s.clone()),
                Some(_) => bail!("surface providers take a plain accessible name"),
            };
            let mut body = Map::new();
            body.insert("role".into(), json!(r.role));
            if let Some(name) = name {
                body.insert("name".into(), json!(name));
            }
            Ok(Json::Object(body))
        }
        Locator::Raw(_) => bail!("raw locators are a web-surface escape hatch"),
    }
}

fn wire_verb(verb: &Verb) -> Option<&'static str> {
    Some(match verb {
        Verb::Click => "click",
        Verb::DblClick => "dblclick",
        Verb::RightClick => "rightclick",
        Verb::Type => "type",
        Verb::Clear => "clear",
        Verb::Press => "press",
        Verb::Hover => "hover",
        Verb::Select => "select",
        Verb::Check => "check",
        Verb::Uncheck => "uncheck",
        Verb::ScrollTo => "scrollTo",
        Verb::Focus => "focus",
        Verb::Wait => "wait",
        Verb::Read => "read",
        _ => return None,
    })
}

fn target_json(target: &SurfaceTarget) -> Json {
    let mut body = Map::new();
    if let Some(app) = &target.app {
        body.insert("app".into(), json!(app));
    }
    if let Some(window) = &target.window {
        body.insert("window".into(), json!(window));
    }
    if let Some(device) = &target.device {
        body.insert("device".into(), json!(device));
    }
    Json::Object(body)
}

fn gated_off(step: &Step, run_tags: &[String]) -> bool {
    let Some(ctx) = step.context() else {
        return false;
    };
    if let Some(tags) = ctx.run_for.as_ref().filter(|t| !t.is_empty()) {
        if !tags.iter().any(|t| run_tags.iter().any(|r| r == t)) {
            return true;
        }
    }
    if let Some(tags) = ctx.skip_for.as_ref().filter(|t| !t.is_empty()) {
        if tags.iter().any(|t| run_tags.iter().any(|r| r == t)) {
            return true;
        }
    }
    false
}

fn apply_window(steps: &[Step], from: Option<&str>, until: Option<&str>) -> Result<Vec<Step>> {
    let start = match from {
        Some(id) => steps
            .iter()
            .position(|s| s.id() == id)
            .ok_or_else(|| anyhow!("--from {id} matches no step"))?,
        None => 0,
    };
    let end = match until {
        Some(id) => {
            steps
                .iter()
                .position(|s| s.id() == id)
                .ok_or_else(|| anyhow!("--until {id} matches no step"))?
                + 1
        }
        None => steps.len(),
    };
    if end < start {
        bail!("--until is before --from");
    }
    Ok(steps[start..end].to_vec())
}

struct Emit<'a> {
    run: &'a RunPaths,
    idx: u32,
    total: u32,
    no_sidecars: bool,
}

fn emit(
    out: &Emit<'_>,
    step: &Step,
    status: &str,
    error: Option<String>,
    snapshot: Option<String>,
    screenshot: Option<String>,
) {
    let kind = match step {
        Step::Do { verb, .. } => format!("do:{}", wire_verb(verb).unwrap_or("web")),
        Step::Check { .. } => "check".to_string(),
    };
    let _ = sidecar::append_event(
        out.run,
        &StepEvent {
            idx: out.idx,
            total: out.total,
            id: step.id().to_string(),
            intent: step.intent().to_string(),
            kind,
            status: status.to_string(),
            ms: None,
            error,
            screenshot: if out.no_sidecars { None } else { screenshot },
            snapshot: if out.no_sidecars { None } else { snapshot },
        },
    );
}

fn finish(
    opts: &RunOptions,
    run: &RunPaths,
    audit: &mut RunAudit,
    summary: &RunSummary,
    line: &str,
    first_failure: Option<String>,
) -> Result<RunSummary> {
    eprintln!("{line}");
    audit.finished_at = Some(now_iso());
    audit.summary = Some(line.to_string());
    audit.exit_code = Some(if summary.gates_ok() { 0 } else { 1 });
    if !summary.ok && summary.quarantined {
        audit.quarantined = Some(true);
    }
    sidecar::write_run_audit(run, audit)?;
    update_latest_pointer(&run.scenario_dir, &run.run_id)?;
    if let Some(out) = &opts.output_audit {
        let body = serde_json::to_vec_pretty(audit)?;
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).ok();
        }
        sidecar::atomic_write_file(out, &body)
            .with_context(|| format!("--output-audit write {}", out.display()))?;
    }
    if let Some(msg) = first_failure {
        if !summary.gates_ok() {
            bail!(msg);
        }
    }
    Ok(summary.clone())
}

fn warn_ignored_browser_flags(opts: &RunOptions, surface: &str) {
    let ignored = opts.base_url.is_some()
        || opts.har
        || opts.mock_from.is_some()
        || opts.offline
        || opts.fresh_browser
        || opts.freeze.is_some()
        || opts.heal_from_run.is_some()
        || opts.record_video.is_some()
        || opts.headed
        || opts.browser_profile.is_some()
        || opts.persona.is_some()
        || opts.environment.is_some();
    if ignored {
        eprintln!("[surface] {surface}: browser-only replay flags were ignored");
    }
}

fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

fn invoke(binary: &Path, op: &str, request: Json, timeout: Duration) -> Result<Json> {
    host::invoke(binary, op, request, timeout)
}

fn ping(binary: &Path) -> Result<Pong> {
    let response = invoke(binary, "ping", json!({}), Duration::from_secs(5))?;
    let pong: Pong = serde_json::from_value(response).context("ping response")?;
    if pong.protocol_version > host::PROTOCOL_VERSION {
        bail!(
            "surface provider speaks protocol {}, this build speaks {}",
            pong.protocol_version,
            host::PROTOCOL_VERSION
        );
    }
    Ok(pong)
}

#[derive(Debug, Deserialize)]
struct Pong {
    #[serde(rename = "protocolVersion")]
    protocol_version: u32,
    name: String,
    surfaces: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Observation {
    #[serde(default)]
    snapshot: String,
    #[serde(default, rename = "screenshotPng")]
    screenshot_png: Option<String>,
    #[serde(default)]
    elements: Vec<ObservedElement>,
}

#[derive(Debug, Clone, Deserialize)]
struct ObservedElement {
    role: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    focused: bool,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
    #[serde(default)]
    checked: bool,
}

fn enabled_by_default() -> bool {
    true
}

// ---------- provider resolution (only when a non-web scenario replays) ----------

fn resolve_provider(surface: &str) -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("get cwd")?;
    resolve_provider_from(surface, &cwd)
}

fn resolve_provider_from(surface: &str, cwd: &Path) -> Result<PathBuf> {
    let env_key = surface_env_key(surface);
    if let Ok(spec) = std::env::var(&env_key) {
        if !spec.is_empty() {
            return resolve_spec(&spec, cwd);
        }
    }
    if let Some((toml_path, spec)) = surface_from_configs(cwd, surface)? {
        let base = toml_path.parent().unwrap_or(cwd);
        return resolve_spec(&spec, base);
    }
    bail!(
        "no provider for surface {surface:?}. Set {env_key} or add it to agent-qa.toml:\n  [surfaces]\n  {surface} = \"/path/to/provider\""
    )
}

fn surface_env_key(surface: &str) -> String {
    let mut key = String::from("AGENT_QA_SURFACE_");
    for c in surface.chars() {
        if c.is_ascii_alphanumeric() {
            key.push(c.to_ascii_uppercase());
        } else {
            key.push('_');
        }
    }
    key
}

#[derive(Debug, Deserialize)]
struct SurfaceConfig {
    #[serde(default)]
    surfaces: Option<BTreeMap<String, String>>,
}

fn surface_from_configs(cwd: &Path, surface: &str) -> Result<Option<(PathBuf, String)>> {
    let mut files = Vec::new();
    let mut cur: Option<&Path> = Some(cwd);
    while let Some(dir) = cur {
        for name in ["agent-qa.toml", ".agent-qa.toml"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                files.push(candidate);
                cur = None;
                break;
            }
        }
        if cur.is_some() {
            cur = dir.parent();
        }
    }
    files.extend(crate::global_config::existing_global_config_files());
    for path in files {
        let bytes =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let cfg: SurfaceConfig =
            toml::from_str(&bytes).with_context(|| format!("parse {}", path.display()))?;
        if let Some(spec) = cfg.surfaces.as_ref().and_then(|t| t.get(surface)) {
            return Ok(Some((path, spec.clone())));
        }
    }
    Ok(None)
}

fn resolve_spec(spec: &str, base: &Path) -> Result<PathBuf> {
    let expanded = crate::global_config::expand_tilde(spec);
    // A leading slash is absolute on Unix and root-relative on Windows.
    // Joining it would pin it to the toml file's drive (`C:/from/env`).
    if expanded.is_absolute() || expanded.has_root() {
        return Ok(expanded);
    }
    if spec.contains('/') || spec.contains('\\') {
        return Ok(base.join(expanded));
    }
    which::which(spec).with_context(|| format!("resolve surface provider `{spec}` on $PATH"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[cfg(unix)]
    use crate::browser;
    #[cfg(unix)]
    use crate::runner::{self, RunOptions, ScenarioSource};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn web_and_absent_stay_on_the_browser_surface() {
        assert!(!is_external(&json!({"schema": "scenario/2"})));
        assert!(!is_external(&json!({"surface": "web"})));
        assert!(is_external(&json!({"surface": "desktop"})));
        assert!(is_external(&json!({"surface": "ios"})));
        assert!(is_external(&json!({"surface": "android"})));
    }

    #[test]
    fn provider_env_wins_over_toml() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("agent-qa.toml"),
            "[surfaces]\ndesktop = \"/from/toml\"\n",
        )
        .unwrap();
        std::env::set_var("AGENT_QA_SURFACE_DESKTOP", "/from/env");
        let got = resolve_provider_from("desktop", tmp.path()).unwrap();
        assert_eq!(got, PathBuf::from("/from/env"));
        std::env::remove_var("AGENT_QA_SURFACE_DESKTOP");
        let got = resolve_provider_from("desktop", tmp.path()).unwrap();
        assert_eq!(got, PathBuf::from("/from/toml"));
    }

    #[test]
    fn missing_provider_names_the_setup() {
        let _g = lock_env();
        std::env::remove_var("AGENT_QA_SURFACE_DESKTOP");
        let tmp = TempDir::new().unwrap();
        let err = resolve_provider_from("desktop", tmp.path())
            .unwrap_err()
            .to_string();
        assert!(err.contains("AGENT_QA_SURFACE_DESKTOP"), "{err}");
        assert!(err.contains("[surfaces]"), "{err}");
    }

    #[cfg(unix)]
    fn write_exec(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    #[cfg(unix)]
    fn opts(path: PathBuf, dry_run: bool) -> RunOptions {
        RunOptions {
            source: ScenarioSource::Path(path),
            profile: None,
            persona: None,
            environment: None,
            session_name: "surface-test".into(),
            heal_from_run: None,
            headed: false,
            browser_profile: None,
            input_overrides: BTreeMap::new(),
            dry_run,
            no_sidecars: false,
            quiet: true,
            plain: true,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            run_for: Vec::new(),
            baseline_sync: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
            fresh_browser: false,
        }
    }

    #[cfg(unix)]
    fn provider_script(log: &Path, png_b64: &str) -> String {
        format!(
            r#"#!/bin/sh
echo "$1" >> '{log}'
cat >/dev/null
case "$1" in
  ping)
    printf '%s\n' '{{"ok":true,"response":{{"protocolVersion":1,"name":"fake-desktop","surfaces":["desktop"]}}}}'
    ;;
  open|close)
    printf '%s\n' '{{"ok":true,"response":{{"ready":true}}}}'
    ;;
  act|observe)
    printf '%s\n' '{{"ok":true,"response":{{"snapshot":"- button \"Save\"","screenshotPng":"{png}","elements":[{{"role":"button","name":"Save","text":"Save","enabled":true,"focused":true}}]}}}}'
    ;;
  *)
    printf '%s\n' '{{"ok":false,"error":{{"code":"unsupported","message":"'"$1"'"}}}}'
    ;;
esac
exit 0
"#,
            log = log.display(),
            png = png_b64,
        )
    }

    #[cfg(unix)]
    fn png_bytes() -> Vec<u8> {
        let mut buf = Vec::new();
        let enc = image::codecs::png::PngEncoder::new(&mut buf);
        use image::ImageEncoder;
        enc.write_image(&[9, 8, 7, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        buf
    }

    #[cfg(unix)]
    #[test]
    fn desktop_replay_uses_the_provider_and_not_the_browser() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let browser_log = work.path().join("browser.log");
        let browser_bin = write_exec(
            work.path(),
            "agent-browser",
            &format!(
                "#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n",
                browser_log.display()
            ),
        );
        std::env::set_var(browser::BIN_ENV, &browser_bin);
        browser::_reset_bin_cache_for_tests();

        let png = png_bytes();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        let provider_log = work.path().join("provider.log");
        let provider = write_exec(
            work.path(),
            "provider",
            &provider_script(&provider_log, &b64),
        );
        std::env::set_var("AGENT_QA_SURFACE_DESKTOP", &provider);

        let sid = work.path().join("sid");
        fs::create_dir_all(sid.join("baselines")).unwrap();
        fs::write(sid.join("baselines").join("s0.png"), &png).unwrap();
        fs::write(
            sid.join("baselines").join("s0.snap.txt"),
            "- button \"Save\"\n",
        )
        .unwrap();
        let scenario = sid.join("scenario.json");
        fs::write(
            &scenario,
            r#"{
                "schema": "scenario/2",
                "id": "desk",
                "intent": "save the note",
                "surface": "desktop",
                "target": { "app": "TextEdit", "window": "Untitled" },
                "steps": [
                    {"id":"s0","intent":"save","kind":"do","verb":"click","on":{"role":"button","name":"Save"}},
                    {"id":"s1","intent":"save is there","kind":"check","claim":{"subject":{"element":{"role":"button","name":"Save"}},"predicate":"isVisible"}},
                    {"id":"s2","intent":"pixels","kind":"check","claim":{"subject":{"shot":"s0"},"predicate":"matches"}},
                    {"id":"s3","intent":"tree","kind":"check","claim":{"subject":{"domshot":"s0"},"predicate":"matches"}}
                ]
            }"#,
        )
        .unwrap();

        let summary = runner::run(&opts(scenario, false)).unwrap();
        assert!(summary.ok, "summary={summary:?}");
        assert_eq!(summary.passed, 4);
        assert_eq!(summary.total, 4);

        let ops = fs::read_to_string(&provider_log).unwrap();
        assert!(ops.contains("ping"), "{ops}");
        assert!(ops.contains("open"), "{ops}");
        assert!(ops.contains("act"), "{ops}");
        assert!(ops.contains("observe"), "{ops}");
        assert!(ops.contains("close"), "{ops}");
        let browser = fs::read_to_string(&browser_log).unwrap_or_default();
        assert!(browser.is_empty(), "browser was invoked: {browser}");

        std::env::remove_var("AGENT_QA_SURFACE_DESKTOP");
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    #[cfg(unix)]
    #[test]
    fn web_surface_still_drives_the_browser_when_a_desktop_provider_is_configured() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let browser_log = work.path().join("browser.log");
        let browser_bin = write_exec(
            work.path(),
            "agent-browser",
            &format!(
                "#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n",
                browser_log.display()
            ),
        );
        std::env::set_var(browser::BIN_ENV, &browser_bin);
        browser::_reset_bin_cache_for_tests();
        let marker = work.path().join("provider-was-spawned");
        let provider = write_exec(
            work.path(),
            "provider",
            &format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display()),
        );
        std::env::set_var("AGENT_QA_SURFACE_DESKTOP", &provider);

        let sid = work.path().join("sid");
        fs::create_dir_all(&sid).unwrap();
        let scenario = sid.join("scenario.json");
        fs::write(
            &scenario,
            r#"{
                "schema": "scenario/2",
                "id": "smoke",
                "intent": "open the home page",
                "surface": "web",
                "env": { "open": [ { "kind": "nav", "url": "https://example.com/" } ] },
                "steps": [
                    { "id": "s1", "intent": "url", "kind": "check",
                      "claim": { "subject": { "url": true }, "predicate": "exists" } }
                ]
            }"#,
        )
        .unwrap();

        let summary = runner::run(&opts(scenario, false)).unwrap();
        assert!(summary.ok);
        assert!(!marker.exists(), "web replay spawned the desktop provider");
        let browser = fs::read_to_string(&browser_log).unwrap();
        assert!(
            browser.contains("open https://example.com/"),
            "got: {browser}"
        );

        std::env::remove_var("AGENT_QA_SURFACE_DESKTOP");
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    #[cfg(unix)]
    #[test]
    fn desktop_dry_run_does_not_spawn_a_provider() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let marker = work.path().join("provider-was-spawned");
        let provider = write_exec(
            work.path(),
            "provider",
            &format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display()),
        );
        std::env::set_var("AGENT_QA_SURFACE_DESKTOP", &provider);
        let sid = work.path().join("sid");
        fs::create_dir_all(&sid).unwrap();
        let scenario = sid.join("scenario.json");
        fs::write(
            &scenario,
            r#"{
                "schema": "scenario/2",
                "id": "desk",
                "intent": "save",
                "surface": "desktop",
                "target": { "app": "TextEdit" },
                "steps": [
                    {"id":"s0","intent":"save","kind":"do","verb":"click","on":{"role":"button","name":"Save"}}
                ]
            }"#,
        )
        .unwrap();
        let summary = runner::run(&opts(scenario.clone(), true)).unwrap();
        assert!(summary.ok);
        assert_eq!(summary.passed, 0);
        assert!(!marker.exists());
        let latest = fs::read_to_string(sid.join("replays").join("latest.txt")).unwrap();
        let audit: Json = serde_json::from_slice(
            &fs::read(sid.join("replays").join(latest.trim()).join("audit.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(audit["summary"], "SUMMARY: 0/1 (DRY-RUN)");
        std::env::remove_var("AGENT_QA_SURFACE_DESKTOP");
    }

    #[cfg(unix)]
    #[test]
    fn web_verb_on_desktop_fails_without_the_browser() {
        let _g = lock_env();
        let work = TempDir::new().unwrap();
        let browser_log = work.path().join("browser.log");
        let browser_bin = write_exec(
            work.path(),
            "agent-browser",
            &format!(
                "#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n",
                browser_log.display()
            ),
        );
        std::env::set_var(browser::BIN_ENV, &browser_bin);
        browser::_reset_bin_cache_for_tests();
        let provider_log = work.path().join("provider.log");
        let provider = write_exec(work.path(), "provider", &provider_script(&provider_log, ""));
        std::env::set_var("AGENT_QA_SURFACE_DESKTOP", &provider);
        let sid = work.path().join("sid");
        fs::create_dir_all(&sid).unwrap();
        let scenario = sid.join("scenario.json");
        fs::write(
            &scenario,
            r#"{
                "schema": "scenario/2",
                "id": "desk",
                "intent": "navigate",
                "surface": "desktop",
                "target": { "app": "TextEdit" },
                "steps": [
                    {"id":"s0","intent":"open","kind":"do","verb":"goto",
                     "value":{"from":"literal","literal":"https://example.com/"}}
                ]
            }"#,
        )
        .unwrap();
        let err = runner::run(&opts(scenario, false)).unwrap_err().to_string();
        assert!(err.contains("web-surface"), "{err}");
        let browser = fs::read_to_string(&browser_log).unwrap_or_default();
        assert!(browser.is_empty(), "browser was invoked: {browser}");
        std::env::remove_var("AGENT_QA_SURFACE_DESKTOP");
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }
}
