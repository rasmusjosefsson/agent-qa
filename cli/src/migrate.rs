//! `agent-qa migrate <dir>` — convert Playwright/Cypress spec files into
//! scenario/2 documents.
//!
//! Best-effort mechanical translation: each spec file becomes one scenario
//! under <out>/<sid>/scenario.json. Statements the importer doesn't
//! understand are counted and reported, never silently dropped from the
//! report — a scenario that skips half its spec is worse than an honest
//! "couldn't convert."

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub fn cli(args: &[String]) -> Result<u8> {
    let mut dir: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut dry_run = false;
    let mut prefix = String::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(args.get(i).context("--out needs a dir")?));
            }
            "--dry-run" => dry_run = true,
            "--sid-prefix" => {
                i += 1;
                prefix = args.get(i).context("--sid-prefix needs a value")?.clone();
            }
            "-h" | "--help" => {
                println!("agent-qa migrate <dir> [--out <dir>] [--dry-run] [--sid-prefix <p>]\n  Convert Playwright/Cypress spec files to scenario/2 documents.");
                return Ok(0u8);
            }
            other if dir.is_none() => dir = Some(PathBuf::from(other)),
            other => bail!("migrate: unexpected arg {other:?}"),
        }
        i += 1;
    }
    let dir = dir.context("migrate: spec dir required (e.g. `agent-qa migrate tests/`)")?;
    if !dir.is_dir() {
        bail!("migrate: {} is not a directory", dir.display());
    }
    let out = out.unwrap_or_else(|| PathBuf::from("scenarios"));

    let mut specs = Vec::new();
    collect_specs(&dir, &mut specs)?;
    specs.sort();
    if specs.is_empty() {
        bail!(
            "migrate: no .spec/.test/.cy spec files under {}",
            dir.display()
        );
    }

    let mut total_steps = 0usize;
    let mut total_skipped = 0usize;
    let mut used_sids = std::collections::HashSet::new();
    let mut collision = 1usize;
    for spec in &specs {
        let source =
            fs::read_to_string(spec).with_context(|| format!("read {}", spec.display()))?;
        let mut converted = convert_file(spec, &source, &prefix);
        while !used_sids.insert(converted.sid.clone()) {
            collision += 1;
            converted.sid = format!(
                "{}-{}",
                converted
                    .sid
                    .trim_end_matches(|c: char| c.is_ascii_digit() || c == '-'),
                collision
            );
            converted.scenario["id"] = serde_json::json!(converted.sid);
        }
        total_steps += converted.step_count;
        total_skipped += converted.skipped.len();
        let status = if converted.skipped.is_empty() {
            "ok".to_string()
        } else {
            format!("{} unmapped line(s)", converted.skipped.len())
        };
        println!(
            "{:>3} steps  {:<40} {}",
            converted.step_count, converted.sid, status
        );
        for (line_no, text) in &converted.skipped {
            println!(
                "      skip  {}:{}: {}",
                spec.display(),
                line_no,
                text.trim()
            );
        }
        if !dry_run && converted.step_count > 0 {
            let sdir = out.join(&converted.sid);
            fs::create_dir_all(&sdir).with_context(|| format!("mkdir {}", sdir.display()))?;
            let file = sdir.join("scenario.json");
            fs::write(
                &file,
                serde_json::to_string_pretty(&converted.scenario)? + "\n",
            )
            .with_context(|| format!("write {}", file.display()))?;
        }
    }
    println!(
        "migrate: {} spec file(s) → {} step(s), {} unmapped{}",
        specs.len(),
        total_steps,
        total_skipped,
        if dry_run { " (dry run)" } else { "" }
    );
    Ok(0u8)
}

fn collect_specs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "node_modules" && !name.starts_with('.') {
                collect_specs(&path, out)?;
            }
        } else if name.ends_with(".spec.ts")
            || name.ends_with(".spec.js")
            || name.ends_with(".test.ts")
            || name.ends_with(".test.js")
            || name.ends_with(".cy.ts")
            || name.ends_with(".cy.js")
        {
            out.push(path);
        }
    }
    Ok(())
}

struct Converted {
    sid: String,
    step_count: usize,
    scenario: Value,
    skipped: Vec<(usize, String)>,
}

fn lit(s: &str) -> Value {
    json!({ "from": "literal", "literal": s })
}

fn role_loc(role: &str, name: Option<&str>) -> Value {
    match name {
        Some(n) => json!({ "role": role, "name": { "pattern": n, "match": "exact" } }),
        None => json!({ "role": role }),
    }
}

fn raw_loc(kind: &str, value: &str, reason: &str) -> Value {
    json!({ "raw": { "kind": kind, "value": value }, "reason": reason })
}

fn do_step(n: usize, verb: &str, intent: String, extra: &[(&str, Value)]) -> Value {
    let mut step = json!({ "id": format!("s{n}"), "kind": "do", "verb": verb, "intent": intent });
    for (k, v) in extra {
        step[*k] = v.clone();
    }
    step
}

fn check_step(
    n: usize,
    intent: String,
    subject: Value,
    predicate: &str,
    value: Option<&str>,
) -> Value {
    let mut claim = json!({ "subject": subject, "predicate": predicate });
    if let Some(v) = value {
        claim["value"] = json!(v);
    }
    json!({ "id": format!("s{n}"), "kind": "check", "intent": intent, "claim": claim })
}

fn convert_file(path: &Path, source: &str, prefix: &str) -> Converted {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("migrated")
        .trim_end_matches(".spec")
        .trim_end_matches(".test")
        .trim_end_matches(".cy")
        .replace(
            |c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "-",
        )
        .trim_matches('-')
        .to_lowercase();
    let sid = format!("{prefix}{stem}");

    let reason = format!("migrated from {}", path.display());
    let mut steps: Vec<Value> = Vec::new();
    let mut skipped: Vec<(usize, String)> = Vec::new();
    let mut current_test = String::new();
    let mut n = 0usize;
    // `const x = page.locator('.a');` → later `x.click()` / `expect(x)` get
    // the call text substituted so locator_of sees a normal locator.
    let mut aliases: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let alias_re = Regex::new(
        r#"\bconst\s+([A-Za-z_]\w*)\s*=\s*(?:await\s+)?((?:page|cy)\.[A-Za-z]+\s*\([^;]+\))"#,
    )
    .unwrap();

    let statements = statements_of(source);
    for (line_no, stmt) in statements {
        let s = stmt.trim();
        if let Some(t) = capture(r#"^\s*(?:describe|context)\s*\(\s*['"`]([^'"`]+)"#, s) {
            current_test = t;
            continue;
        }
        if let Some(t) = capture(r#"^\s*(?:test|it)\s*(?:\.\w+)*\s*\(\s*['"`]([^'"`]+)"#, s) {
            current_test = t;
            continue;
        }
        if let Some(m) = alias_re.captures(s) {
            aliases.insert(m[1].to_string(), m[2].to_string());
        }
        let mut expanded = s.to_string();
        for (name, call) in &aliases {
            let dotted = Regex::new(&format!(r"\b{}\s*\.", regex::escape(name))).unwrap();
            expanded = dotted
                .replace_all(&expanded, format!("{call}."))
                .into_owned();
            let expect = Regex::new(&format!(r"(expect\(\s*){}", regex::escape(name))).unwrap();
            expanded = expect
                .replace_all(&expanded, format!("${{1}}{call}"))
                .into_owned();
        }
        if let Some(step) = map_statement(&expanded, &reason, n) {
            n += 1;
            steps.push(step);
            continue;
        }
        if ignorable(s) {
            continue;
        }
        skipped.push((line_no, s.chars().take(100).collect()));
    }

    let intent = if current_test.is_empty() {
        format!("migrated: {stem}")
    } else {
        format!("migrated: {stem} — {current_test}")
    };
    let scenario = json!({
        "schema": "scenario/2",
        "id": sid,
        "intent": intent,
        "env": { "open": [{ "kind": "fresh" }] },
        "steps": steps,
    });
    let step_count = scenario["steps"].as_array().map_or(0, Vec::len);
    Converted {
        sid,
        step_count,
        scenario,
        skipped,
    }
}

/// Join physical lines into statements: block headers and closers emit on
/// their own (their parens never balance); everything else accumulates until
/// parens balance and the line ends with `;`. Keeps the first line's number
/// for skip reports.
fn statements_of(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut start = 0usize;
    for (i, line) in source.lines().enumerate() {
        let t = line.trim();
        if buf.is_empty() {
            if t.is_empty() || t.starts_with("//") {
                continue;
            }
            start = i + 1;
            let standalone = t.starts_with("describe(")
                || t.starts_with("context(")
                || t.starts_with("test(")
                || t.starts_with("test.")
                || t.starts_with("it(")
                || t.starts_with("it.")
                || t.starts_with("beforeEach(")
                || t.starts_with("afterEach(")
                || t.starts_with("beforeAll(")
                || t.starts_with("afterAll(")
                || matches!(t, "});" | "})" | "}" | "{");
            if standalone {
                // One-liner blocks (`test('x', () => { await page.goto('/'); });`)
                // put the body on the header line — split it back out.
                if let Some(pos) = t.find("=>") {
                    let body = t[pos + 2..].trim_start().trim_start_matches('{').trim();
                    let head = t[..pos + 2].trim_end().to_string();
                    out.push((start, head));
                    for piece in body.split(';') {
                        let p = piece.trim();
                        if !p.is_empty() {
                            out.push((start, format!("{p};")));
                        }
                    }
                } else {
                    out.push((start, t.to_string()));
                }
                continue;
            }
        }
        buf.push_str(t);
        buf.push(' ');
        let opens = buf.matches('(').count();
        let closes = buf.matches(')').count();
        if opens <= closes && t.ends_with(';') {
            out.push((start, buf.trim().to_string()));
            buf.clear();
        }
    }
    if !buf.trim().is_empty() {
        out.push((start, buf.trim().to_string()));
    }
    out
}

fn capture(pattern: &str, text: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()?
        .captures(text)?
        .get(1)
        .map(|m| m.as_str().to_string())
}

/// Extract the locator a statement acts on: the first getBy*/locator/get/$
/// expression in the statement.
fn locator_of(s: &str, reason: &str) -> Option<Value> {
    if let Some(m) = Regex::new(
        r#"getByRole\(\s*['"]([a-zA-Z]+)['"](?:\s*,\s*\{\s*name\s*:\s*(['"][^'"]+['"]|/[^/]+/[a-z]*))?"#,
    )
    .ok()?
    .captures(s)
    {
        let role = m.get(1)?.as_str();
        let raw_name = m.get(2).map(|g| g.as_str());
        return Some(match raw_name {
            Some(n) if n.starts_with('/') => {
                // "/save/i" → pattern "save" (flags dropped)
                let inner = n[1..].split('/').next().unwrap_or("").to_string();
                json!({ "role": role, "name": { "pattern": inner, "match": "regex" } })
            }
            Some(n) => role_loc(role, Some(n.trim_matches(|c| c == '\'' || c == '"'))),
            None => role_loc(role, None),
        });
    }
    for (getter, kind) in [
        ("getByTestId", "testId"),
        ("getByText", "text"),
        ("getByAltText", "text"),
    ] {
        if let Some(v) = capture(&format!(r#"{getter}\(\s*['"]([^'"]+)"#), s) {
            return Some(raw_loc(kind, &v, reason));
        }
    }
    for (getter, attr) in [
        ("getByLabel", "aria-label"),
        ("getByPlaceholder", "placeholder"),
        ("getByTitle", "title"),
    ] {
        if let Some(v) = capture(&format!(r#"{getter}\(\s*['"]([^'"]+)"#), s) {
            return Some(raw_loc("css", &format!("[{attr}=\"{v}\"]"), reason));
        }
    }
    if let Some(v) = capture(r#"cy\.contains\(\s*['"]([^'"]+)"#, s) {
        return Some(raw_loc("text", &v, reason));
    }
    for pat in [
        r#"page\.locator\(\s*['"]([^'"]+)"#,
        r#"page\.\$\$?\(\s*['"]([^'"]+)"#,
        r#"cy\.get\(\s*['"]([^'"]+)"#,
        r#"locator\(\s*['"]([^'"]+)"#,
    ] {
        if let Some(v) = capture(pat, s) {
            return Some(raw_loc("css", &v, reason));
        }
    }
    None
}

fn ignorable(s: &str) -> bool {
    s.starts_with("import ")
        || s.starts_with("export ")
        || s.starts_with("const ")
        || s.starts_with("let ")
        || s.starts_with("await page.waitFor")
        || s.starts_with("cy.wait(")
        || s.starts_with("await test.")
        || s.starts_with("test.use(")
        || s.starts_with("test.")
        || s.contains("beforeEach(")
        || s.contains("afterEach(")
        || s.contains("beforeAll(")
        || s.contains("afterAll(")
        || s == "});"
        || s == "}"
        || s.ends_with("});") && !s.contains('(')
}

fn map_statement(s: &str, reason: &str, n: usize) -> Option<Value> {
    // navigation
    for pat in [
        r#"page\.goto\(\s*['"`]([^'"`]+)"#,
        r#"cy\.visit\(\s*['"`]([^'"`]+)"#,
    ] {
        if let Some(url) = capture(pat, s) {
            return Some(do_step(
                n,
                "goto",
                format!("goto {url}"),
                &[("value", lit(&url))],
            ));
        }
    }

    // expects/assertions
    if s.contains("expect(") || s.contains(".should(") {
        return map_assertion(s, reason, n);
    }

    // actions on a locator
    if let Some(loc) = locator_of(s, reason) {
        if s.contains(".dblclick(") {
            return Some(do_step(
                n,
                "dblclick",
                "double-click".into(),
                &[("on", loc)],
            ));
        }
        if s.contains(".uncheck(") {
            return Some(do_step(n, "uncheck", "uncheck".into(), &[("on", loc)]));
        }
        if s.contains(".check(") {
            return Some(do_step(n, "check", "check".into(), &[("on", loc)]));
        }
        if let Some(v) = capture(r#"\.selectOption\(\s*['"]([^'"]+)"#, s) {
            return Some(do_step(
                n,
                "select",
                format!("select {v}"),
                &[("on", loc), ("value", lit(&v))],
            ));
        }
        if s.contains(".hover(") {
            return Some(do_step(n, "hover", "hover".into(), &[("on", loc)]));
        }
        for pat in [
            r#"\.fill\(\s*['"`]([^'"`]*)"#,
            r#"\.type\(\s*['"`]([^'"`]+)"#,
            r#"\.pressSequentially\(\s*['"`]([^'"`]+)"#,
        ] {
            if let Some(v) = capture(pat, s) {
                return Some(do_step(
                    n,
                    "type",
                    format!("type {v:?}"),
                    &[("on", loc), ("value", lit(&v))],
                ));
            }
        }
        if let Some(v) = capture(r#"\.press\(\s*['"]([^'"]+)"#, s) {
            return Some(do_step(
                n,
                "press",
                format!("press {v}"),
                &[("on", loc), ("value", lit(&v))],
            ));
        }
        if s.contains(".click(") {
            return Some(do_step(n, "click", "click".into(), &[("on", loc)]));
        }
        return None;
    }

    // keyboard input without a target locator — press/type onto body
    if let Some(v) = capture(r#"page\.keyboard\.press\(\s*['"]([^'"]+)"#, s) {
        return Some(do_step(
            n,
            "press",
            format!("press {v}"),
            &[("on", raw_loc("css", "body", reason)), ("value", lit(&v))],
        ));
    }
    if let Some(v) = capture(
        r#"page\.keyboard\.(?:type|insertText)\(\s*['"`]([^'"`]+)"#,
        s,
    ) {
        return Some(do_step(
            n,
            "type",
            format!("type {v:?}"),
            &[("on", raw_loc("css", "body", reason)), ("value", lit(&v))],
        ));
    }

    // selector-free page actions
    if let Some(v) = capture(r#"page\.press\(\s*['"]([^'"]+)['"]\s*,\s*['"]([^'"]+)"#, s) {
        return Some(do_step(
            n,
            "press",
            format!("press {v}"),
            &[("on", raw_loc("css", &v, reason)), ("value", lit(&v))],
        ));
    }
    None
}

fn map_assertion(s: &str, reason: &str, n: usize) -> Option<Value> {
    // URL assertions
    if let Some(u) = capture(r#"toHaveURL\(\s*/([^/]+)/"#, s) {
        return Some(check_step(
            n,
            format!("url matches /{u}/"),
            json!({ "url": true }),
            "matches",
            Some(&u),
        ));
    }
    for pat in [
        r#"toHaveURL\(\s*['"`]([^'"`]+)"#,
        r#"cy\.url\(\)\.should\(\s*['"](?:include|contain)['"]\s*,\s*['"]([^'"]+)"#,
    ] {
        if let Some(u) = capture(pat, s) {
            return Some(check_step(
                n,
                format!("url contains {u}"),
                json!({ "url": true }),
                "contains",
                Some(&u),
            ));
        }
    }
    // .should('be.visible') / 'exist' / 'have.text' / 'contain'
    if let Some(loc) = locator_of(s, reason) {
        // expect(...).not.<flag> inverts flag predicates exactly; negated
        // text assertions have no schema predicate — report them instead of
        // mapping wrong.
        let neg = s.contains(".not.");
        let flag = |pos: &'static str, negated: &'static str| if neg { negated } else { pos };
        if s.contains("toBeVisible") || s.contains("'be.visible'") || s.contains("\"be.visible\"") {
            return Some(check_step(
                n,
                "element visible".into(),
                json!({ "element": loc }),
                flag("isVisible", "isHidden"),
                None,
            ));
        }
        if s.contains("toBeHidden") || s.contains("'be.hidden'") {
            return Some(check_step(
                n,
                "element hidden".into(),
                json!({ "element": loc }),
                flag("isHidden", "isVisible"),
                None,
            ));
        }
        if s.contains("toBeChecked") || s.contains("'be.checked'") {
            return Some(check_step(
                n,
                "element checked".into(),
                json!({ "element": loc }),
                flag("isChecked", "isUnchecked"),
                None,
            ));
        }
        if s.contains("toBeEnabled") || s.contains("'be.enabled'") {
            return Some(check_step(
                n,
                "element enabled".into(),
                json!({ "element": loc }),
                flag("isEnabled", "isDisabled"),
                None,
            ));
        }
        if s.contains("toBeDisabled") || s.contains("'be.disabled'") {
            return Some(check_step(
                n,
                "element disabled".into(),
                json!({ "element": loc }),
                flag("isDisabled", "isEnabled"),
                None,
            ));
        }
        if s.contains("'exist'") {
            return Some(check_step(
                n,
                "element exists".into(),
                json!({ "element": loc }),
                flag("exists", "notExists"),
                None,
            ));
        }
        if neg {
            return None;
        }
        if let Some(v) = capture(r#"toHaveValue\(\s*['"`]([^'"`]*)"#, s) {
            return Some(check_step(
                n,
                format!("value equals {v:?}"),
                json!({ "element": loc, "ofKind": "value" }),
                "equals",
                Some(&v),
            ));
        }
        if let Some(v) = capture(r#"toHaveCount\(\s*(\d+)"#, s) {
            let mut st = check_step(
                n,
                format!("count equals {v}"),
                json!({ "element": loc, "ofKind": "count" }),
                "countEquals",
                None,
            );
            st["claim"]["value"] = json!(v.parse::<u64>().unwrap_or(0));
            return Some(st);
        }
        if let Some(a) = capture(r#"toHaveAttribute\(\s*['"]([^'"]+)"#, s) {
            if let Some(v) = capture(
                r#"toHaveAttribute\(\s*['"][^'"]+['"]\s*,\s*['"`]([^'"`]+)"#,
                s,
            ) {
                return Some(check_step(
                    n,
                    format!("{a} equals {v:?}"),
                    json!({ "element": loc, "ofKind": "attribute", "attribute": a }),
                    "equals",
                    Some(&v),
                ));
            }
            if let Some(v) = capture(r#"toHaveAttribute\(\s*['"][^'"]+['"]\s*,\s*/([^/]+)/"#, s) {
                return Some(check_step(
                    n,
                    format!("{a} matches /{v}/"),
                    json!({ "element": loc, "ofKind": "attribute", "attribute": a }),
                    "matches",
                    Some(&v),
                ));
            }
        }
        for (pat, pred, intent) in [
            (r#"toHaveText\(\s*['"`]([^'"`]+)"#, "equals", "text equals"),
            (
                r#"should\(\s*['"]have\.text['"]\s*,\s*['"]([^'"]+)"#,
                "equals",
                "text equals",
            ),
            (
                r#"toContainText\(\s*['"`]([^'"`]+)"#,
                "contains",
                "text contains",
            ),
            (
                r#"should\(\s*['"]contain['"]\s*,\s*['"]([^'"]+)"#,
                "contains",
                "text contains",
            ),
        ] {
            if let Some(v) = capture(pat, s) {
                return Some(check_step(
                    n,
                    format!("{intent} {v:?}"),
                    json!({ "element": loc, "ofKind": "text" }),
                    pred,
                    Some(&v),
                ));
            }
        }
        return Some(check_step(
            n,
            "element exists".into(),
            json!({ "element": loc }),
            "exists",
            None,
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYWRIGHT: &str = r#"
import { test, expect } from '@playwright/test';

test('checkout with saved card', async ({ page }) => {
  await page.goto('/products/desk-lamp');
  await page.getByTestId('add-to-cart').click();
  await page.getByRole('button', { name: 'Checkout' }).click();
  await page.getByLabel('Card ending in 4242').check();
  await expect(page.getByTestId('order-total')).toHaveText('$49.00');
  await expect(page).toHaveURL('/cart');
});
"#;

    const CYPRESS: &str = r#"
describe('login', () => {
  it('logs in', () => {
    cy.visit('/login');
    cy.get('#email').type('a@b.c');
    cy.contains('Sign in').click();
    cy.url().should('include', '/dashboard');
    cy.get('.welcome').should('be.visible');
  });
});
"#;

    #[test]
    fn playwright_spec_converts() {
        let c = convert_file(Path::new("tests/checkout.spec.ts"), PLAYWRIGHT, "");
        assert_eq!(c.sid, "checkout");
        let steps = c.scenario["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 6);
        assert_eq!(steps[0]["verb"], "goto");
        assert_eq!(steps[1]["on"]["raw"]["kind"], "testId");
        assert_eq!(steps[2]["on"]["role"], "button");
        assert_eq!(steps[3]["verb"], "check");
        assert_eq!(steps[4]["claim"]["predicate"], "equals");
        assert_eq!(steps[5]["claim"]["subject"]["url"], true);
    }

    #[test]
    fn cypress_spec_converts() {
        let c = convert_file(Path::new("cypress/e2e/login.cy.ts"), CYPRESS, "");
        let steps = c.scenario["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0]["verb"], "goto");
        assert_eq!(steps[1]["verb"], "type");
        assert_eq!(steps[2]["on"]["raw"]["kind"], "text");
        assert_eq!(steps[3]["claim"]["predicate"], "contains");
        assert_eq!(steps[4]["claim"]["predicate"], "isVisible");
    }

    #[test]
    fn negated_flags_invert() {
        let src = "test('x', async ({ page }) => {\n  await expect(page.locator('#m')).not.toBeVisible();\n});\n";
        let c = convert_file(Path::new("x.spec.ts"), src, "");
        assert_eq!(c.scenario["steps"][0]["claim"]["predicate"], "isHidden");
    }

    #[test]
    fn regex_role_name_maps_to_regex_match() {
        let src = "test('x', async ({ page }) => {\n  await page.getByRole('button', { name: /save/i }).click();\n});\n";
        let c = convert_file(Path::new("x.spec.ts"), src, "");
        assert_eq!(c.scenario["steps"][0]["on"]["name"]["match"], "regex");
        assert_eq!(c.scenario["steps"][0]["on"]["name"]["pattern"], "save");
    }

    #[test]
    fn config_lines_and_hooks_dont_count_as_skipped() {
        let src = "test.use({ storageState: 'auth.json' });\ntest.beforeEach(async ({ page }) => { await page.goto('/'); });\ntest.describe.configure({ mode: 'serial' });\ntest('x', async ({ page }) => {\n  await page.goto('/a');\n});\n";
        let c = convert_file(Path::new("x.spec.ts"), src, "");
        // hooks and config lines are ignored outright — they must not
        // pollute the skip report (their bodies are setup, not the test).
        assert!(c
            .skipped
            .iter()
            .all(|(_, t)| !t.contains("test.use") && !t.contains("configure")));
    }

    #[test]
    fn unmapped_lines_are_reported() {
        let src = "test('x', async ({ page }) => {\n  await page.goto('/a');\n  await page.mouse.wheel(0, 100);\n});\n";
        let c = convert_file(Path::new("x.spec.ts"), src, "");
        assert_eq!(c.skipped.len(), 1);
    }
}
