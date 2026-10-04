//! `agent-qa compile <file.flow>` / `agent-qa describe <scenario>` — the
//! line-DSL surface (scan-3 item 1): a diffable, hand-writable text form
//! that compiles to a scenario/2 document, and back.
//!
//! Bounded grammar = deterministic parse; no model anywhere near it. One
//! statement per line; `#`/`//` comments and blank lines are ignored.
//! `! <text>` sets the next step's intent; steps get auto-ids `s1`, `s2`,…
//! `group`/`when`/`end` nest.
//!
//!   base file:///page.html        env.open nav (call once, at the top)
//!   title Login flow              scenario intent
//!   input title = "Hello"         input declaration (default optional)
//!   open /login                   do:goto (literal)
//!   click "Sign in"               quoted = text locator
//!   click #id | css:.x | xpath:… | testid:…
//!   click role:button "Save"
//!   fill "Name" with "Rasmus"     quoted fill target = role textbox name
//!   type "hi" into css:#note
//!   press Enter | select css:#cc = "Visa"
//!   tick <loc> | untick <loc>     checkbox check/uncheck (do-verbs)
//!   clear <loc> | focus <loc> | blur <loc>
//!   scroll css:#deep | scroll bottom | scroll 500
//!   wait 500ms | wait css:#ready | wait url /api/done | wait idle
//!   viewport 1280x900
//!   check "Save" exists|absent|visible|hidden|enabled|disabled|checked|unchecked
//!   check css:#title text equals "Hello"  (or contains/matches/startsWith/endsWith)
//!   check url matches /done$ | check url equals https://…
//!   check console quiet
//!   shot home                     shot claim bound to the last open/goto
//!   group <label> … end           do:group
//!   when present <loc> … end / when absent <loc> … end
//!   include shared/login.json     do:include
//!
//! `describe` is the inverse for steps the DSL can express; anything else
//! (env-op tricks, exotic subjects, callGql, mail, …) lands as a
//! `# unsupported: <id> (<verb>)` comment — visible, never silently
//! recompiled-away.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value as Json};

/// `agent-qa compile <file> [--out <path>] [--check]`
pub fn compile_cli(args: &[String]) -> Result<u8> {
    let mut file: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut check_only = false;
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                println!(
                    "agent-qa compile <file.flow> — compile line-DSL to a scenario.json\n\nUsage:\n  agent-qa compile <file> [--out <scenario.json>] [--check]\n\nOne statement per line: base/title/input/open/click/dblclick/hover/fill/\ntype/press/select/tick/untick/clear/focus/blur/scroll/wait/viewport/check/\nshot/group/when/include + `end` to close blocks.\n`! <text>` sets the next step's intent; `#` and `//` are comments.\nLocators: \"text\", css:<sel> (or #/.[ shorthand), xpath:, testid:,\nrole:<role> [\"name\"]. `fill \"Name\"` targets a role textbox by name."
                );
                return Ok(0);
            }
            "--out" => {
                out = Some(PathBuf::from(it.next().context("--out needs a path")?));
            }
            s if s.starts_with("--out=") => {
                out = Some(PathBuf::from(&s["--out=".len()..]));
            }
            "--check" => check_only = true,
            other if other.starts_with("--") => bail!("compile: unknown flag {other:?}"),
            other => file = Some(PathBuf::from(other)),
        }
    }
    let file =
        file.ok_or_else(|| anyhow::anyhow!("usage: agent-qa compile <file> [--out <path>]"))?;
    let body = std::fs::read_to_string(&file)
        .with_context(|| format!("compile: read {}", file.display()))?;
    let default_id = file
        .file_stem()
        .and_then(|s| s.to_str())
        .map(sanitize_id)
        .unwrap_or_else(|| "flow".to_string());
    let doc = compile(&body, &default_id)?;
    if check_only {
        let n = doc["steps"].as_array().map(|a| a.len()).unwrap_or(0);
        println!("{}: ok — {n} step(s)", file.display());
        return Ok(0);
    }
    let out_path = out.unwrap_or_else(|| file.with_extension("json"));
    std::fs::write(&out_path, serde_json::to_string_pretty(&doc)?)
        .with_context(|| format!("write {}", out_path.display()))?;
    println!(
        "wrote {} — {} step(s)",
        out_path.display(),
        doc["steps"].as_array().map(|a| a.len()).unwrap_or(0)
    );
    Ok(0)
}

/// `agent-qa describe <scenario.json|sid> [--out <file>]`
pub fn describe_cli(args: &[String]) -> Result<u8> {
    let mut target: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                println!(
                    "agent-qa describe <scenario.json|sid> — print the line-DSL form\n\nUsage:\n  agent-qa describe <path-or-sid> [--out <file>]\n\nSteps the DSL can't express emit `# unsupported:` comment lines."
                );
                return Ok(0);
            }
            "--out" => {
                out = Some(PathBuf::from(it.next().context("--out needs a path")?));
            }
            s if s.starts_with("--out=") => {
                out = Some(PathBuf::from(&s["--out=".len()..]));
            }
            other if other.starts_with("--") => bail!("describe: unknown flag {other:?}"),
            other => target = Some(other.to_string()),
        }
    }
    let target = target.ok_or_else(|| anyhow::anyhow!("usage: agent-qa describe <path|sid>"))?;
    let path = {
        let p = PathBuf::from(&target);
        if p.exists() {
            p
        } else {
            let cand = crate::paths::scenarios_root()
                .join(&target)
                .join("scenario.json");
            if cand.exists() {
                cand
            } else {
                bail!("describe: {target:?} is neither a file nor a scenario id under the root")
            }
        }
    };
    let body = std::fs::read_to_string(&path)
        .with_context(|| format!("describe: read {}", path.display()))?;
    let doc: Json = serde_json::from_str(&body)
        .with_context(|| format!("describe: {} is not JSON", path.display()))?;
    let text = describe(&doc);
    match out {
        Some(f) => {
            std::fs::write(&f, &text).with_context(|| format!("write {}", f.display()))?;
            println!("wrote {}", f.display());
        }
        None => print!("{text}"),
    }
    Ok(0)
}

// ---------------------------------------------------------------- compile

fn sanitize_id(s: &str) -> String {
    let id: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let id = id.trim_matches('-').to_string();
    if id.is_empty() {
        "flow".into()
    } else {
        id
    }
}

struct Frame {
    kind: &'static str, // "group" | "when"
    steps: Vec<Json>,
    meta: Json, // group: label string; when: {present|absent: locator}
    /// `!` intent pending when the block opened — a child's `!` must not
    /// clobber the block's own intent.
    intent: Option<String>,
}

fn push_step(stack: &mut [Frame], top: &mut Vec<Json>, s: Json) {
    if let Some(f) = stack.last_mut() {
        f.steps.push(s);
    } else {
        top.push(s);
    }
}

pub(crate) fn compile(body: &str, default_id: &str) -> Result<Json> {
    let mut top: Vec<Json> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut env_navs: Vec<Json> = Vec::new();
    let mut inputs: BTreeMap<String, Json> = BTreeMap::new();
    let mut intent: Option<String> = None;
    let mut scenario_intent = String::from("compiled flow");
    let mut last_goto: Option<String> = None;
    let mut counter = 0usize;

    for (n, raw) in body.lines().enumerate() {
        let lineno = n + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("! ") {
            intent = Some(rest.trim().to_string());
            continue;
        }
        macro_rules! err {
            ($($t:tt)*) => { anyhow::anyhow!("line {lineno}: {}", format!($($t)*)) };
        }
        let (head, tail) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let tail = tail.trim();
        match head {
            "title" => {
                if tail.is_empty() {
                    return Err(err!("title needs text"));
                }
                scenario_intent = tail.to_string();
                continue;
            }
            "base" => {
                if tail.is_empty() {
                    return Err(err!("base needs a url"));
                }
                env_navs.push(json!({"kind": "nav", "url": tail}));
                continue;
            }
            "input" => {
                let (name, def) = tail
                    .split_once('=')
                    .map(|(a, b)| (a.trim().to_string(), Some(unquote(b.trim()).to_string())))
                    .unwrap_or_else(|| (tail.to_string(), None));
                if name.is_empty() {
                    return Err(err!("input needs a name"));
                }
                let mut d = json!({"type": "string"});
                if let Some(def) = def {
                    d["default"] = json!(def);
                }
                inputs.insert(name, d);
                continue;
            }
            "end" => {
                let Some(f) = stack.pop() else {
                    return Err(err!("end without an open group/when"));
                };
                let step = match f.kind {
                    "group" => {
                        counter += 1;
                        let label = f.meta.as_str().unwrap_or("group").to_string();
                        let default_intent = if label.is_empty() {
                            "group".to_string()
                        } else {
                            format!("group {label}")
                        };
                        json!({
                            "id": format!("s{counter}"),
                            "intent": f.intent.unwrap_or(default_intent),
                            "kind": "do", "verb": "group",
                            "params": {"steps": f.steps}
                        })
                    }
                    "when" => {
                        counter += 1;
                        let mut params = f.meta;
                        params["steps"] = json!(f.steps);
                        json!({
                            "id": format!("s{counter}"),
                            "intent": f.intent.unwrap_or_else(|| "conditional block".into()),
                            "kind": "do", "verb": "when",
                            "params": params
                        })
                    }
                    _ => unreachable!(),
                };
                push_step(&mut stack, &mut top, step);
                continue;
            }
            "group" => {
                stack.push(Frame {
                    kind: "group",
                    steps: Vec::new(),
                    meta: json!(unquote(tail)),
                    intent: intent.take(),
                });
                continue;
            }
            "when" => {
                let (word, loc) = tail
                    .split_once(char::is_whitespace)
                    .map(|(a, b)| (a, b.trim()))
                    .unwrap_or((tail, ""));
                let key = match word {
                    "present" => "present",
                    "absent" => "absent",
                    _ => return Err(err!("when needs `present <locator>` or `absent <locator>`")),
                };
                let loc = parse_locator(loc).map_err(|e| err!("{e}"))?;
                stack.push(Frame {
                    kind: "when",
                    steps: Vec::new(),
                    meta: json!({key: loc}),
                    intent: intent.take(),
                });
                continue;
            }
            _ => {}
        }

        counter += 1;
        let id = format!("s{counter}");
        let mk_intent =
            |intent: &mut Option<String>| intent.take().unwrap_or_else(|| line.to_string());
        let step: Json = match head {
            "open" | "goto" => {
                if tail.is_empty() {
                    return Err(err!("open needs a url"));
                }
                last_goto = Some(id.clone());
                json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do", "verb": "goto",
                       "value": {"from": "literal", "literal": tail}})
            }
            "click" | "dblclick" | "hover" | "clear" | "focus" | "blur" => {
                let loc = parse_locator(tail).map_err(|e| err!("{e}"))?;
                json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do", "verb": head,
                       "on": loc})
            }
            "tick" | "untick" => {
                let verb = if head == "tick" { "check" } else { "uncheck" };
                let loc = parse_locator(tail).map_err(|e| err!("{e}"))?;
                json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do", "verb": verb,
                       "on": loc})
            }
            "fill" => {
                // `fill <loc> with <text>` — quoted loc = role textbox name.
                let (loc_s, val_s) = tail
                    .split_once(" with ")
                    .ok_or_else(|| err!("fill needs `<loc> with <text>`"))?;
                let loc = parse_locator_fill(loc_s.trim()).map_err(|e| err!("{e}"))?;
                json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do", "verb": "type",
                       "on": loc, "value": {"from": "literal", "literal": unquote(val_s.trim())}})
            }
            "type" => {
                let (val_s, loc_s) = tail
                    .split_once(" into ")
                    .ok_or_else(|| err!("type needs `<text> into <loc>`"))?;
                let loc = parse_locator(loc_s.trim()).map_err(|e| err!("{e}"))?;
                json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do", "verb": "type",
                       "on": loc, "value": {"from": "literal", "literal": unquote(val_s.trim())}})
            }
            "press" => {
                json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do", "verb": "press",
                       "value": {"from": "literal", "literal": tail}})
            }
            "select" => {
                let (loc_s, val_s) = tail
                    .split_once('=')
                    .ok_or_else(|| err!("select needs `<loc> = <value>`"))?;
                let loc = parse_locator(loc_s.trim()).map_err(|e| err!("{e}"))?;
                json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do", "verb": "select",
                       "on": loc, "value": {"from": "literal", "literal": unquote(val_s.trim())}})
            }
            "scroll" => {
                let mut step = json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do", "verb": "scrollto"});
                match tail {
                    "bottom" | "top" => {
                        step["params"] = json!({"to": tail});
                    }
                    t if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) => {
                        step["params"] = json!({"y": t.parse::<u64>().unwrap()});
                    }
                    _ => {
                        step["on"] = parse_locator(tail).map_err(|e| err!("{e}"))?;
                    }
                }
                step
            }
            "wait" => {
                let mut step = json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do", "verb": "wait"});
                if let Some(ms) = tail.strip_suffix("ms") {
                    step["params"] = json!({"ms": ms.trim().parse::<u64>().map_err(|_| err!("wait <N>ms needs digits"))?});
                } else if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
                    step["params"] = json!({"ms": tail.parse::<u64>().unwrap()});
                } else if let Some(u) = tail.strip_prefix("url ") {
                    step["params"] = json!({"url": u.trim()});
                } else if tail == "idle" {
                    step["params"] = json!({"idle": true});
                } else {
                    step["params"] =
                        json!({"locator": parse_locator(tail).map_err(|e| err!("{e}"))?});
                }
                step
            }
            "viewport" => {
                let (w, h) = tail
                    .split_once('x')
                    .and_then(|(a, b)| {
                        a.trim()
                            .parse::<u32>()
                            .ok()
                            .zip(b.trim().parse::<u32>().ok())
                    })
                    .ok_or_else(|| err!("viewport needs WxH (e.g. 1280x900)"))?;
                json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do", "verb": "viewport",
                       "params": {"width": w, "height": h}})
            }
            "check" => {
                let rest = tail;
                if let Some(l) = rest.strip_prefix("url ") {
                    let (pred, val) = l
                        .split_once(char::is_whitespace)
                        .map(|(a, b)| (a, unquote(b.trim())))
                        .ok_or_else(|| err!("check url needs `equals|matches|contains <x>`"))?;
                    match pred {
                        "equals" | "matches" | "contains" | "startsWith" | "endsWith" => {}
                        _ => return Err(err!("check url predicate must be equals|matches|contains|startsWith|endsWith")),
                    }
                    json!({"id": id, "intent": mk_intent(&mut intent), "kind": "check",
                           "claim": {"subject": {"url": true}, "predicate": pred, "value": val}})
                } else if rest == "console quiet" {
                    json!({"id": id, "intent": mk_intent(&mut intent), "kind": "check",
                           "claim": {"subject": {"console": {"type": "error"}}, "predicate": "notExists"}})
                } else {
                    let (loc, rest2) = split_locator_rest(rest)
                        .ok_or_else(|| err!("check needs `<loc> <pred> [value]`"))?;
                    let rest2 = rest2.trim();
                    let (word, val) = rest2
                        .split_once(char::is_whitespace)
                        .map(|(a, b)| (a, Some(b.trim())))
                        .unwrap_or((rest2, None));
                    let mut claim = json!({});
                    let mut subject = json!({"element": loc});
                    match word {
                        "exists" => claim["predicate"] = json!("exists"),
                        "absent" | "missing" => claim["predicate"] = json!("notExists"),
                        "visible" => claim["predicate"] = json!("isVisible"),
                        "hidden" => claim["predicate"] = json!("isHidden"),
                        "enabled" => claim["predicate"] = json!("isEnabled"),
                        "disabled" => claim["predicate"] = json!("isDisabled"),
                        "checked" => claim["predicate"] = json!("isChecked"),
                        "unchecked" => claim["predicate"] = json!("isUnchecked"),
                        "text" => {
                            let t = val
                                .ok_or_else(|| err!("check … text needs `equals|contains <x>`"))?;
                            let (p, v) = t
                                .split_once(char::is_whitespace)
                                .ok_or_else(|| err!("check … text needs `equals|contains <x>`"))?;
                            match p {
                                "equals" | "contains" | "matches" | "startsWith" | "endsWith" => {}
                                _ => return Err(err!("check … text predicate must be equals|contains|matches|startsWith|endsWith")),
                            }
                            subject["ofKind"] = json!("text");
                            claim["predicate"] = json!(p);
                            claim["value"] = json!(unquote(v));
                        }
                        p if ["equals", "contains", "matches", "startsWith", "endsWith"]
                            .contains(&p) =>
                        {
                            let v = val.ok_or_else(|| err!("check … {p} needs a value"))?;
                            subject["ofKind"] = json!("text");
                            claim["predicate"] = json!(p);
                            claim["value"] = json!(unquote(v));
                        }
                        "" => {
                            return Err(err!(
                                "check needs a predicate — exists|absent|visible|…|text <pred> <x>"
                            ))
                        }
                        p => return Err(err!("unknown check predicate {p:?}")),
                    }
                    claim["subject"] = subject;
                    json!({"id": id, "intent": mk_intent(&mut intent), "kind": "check", "claim": claim})
                }
            }
            "shot" => {
                let name = if tail.is_empty() { "page" } else { tail };
                let goto = last_goto
                    .clone()
                    .ok_or_else(|| err!("shot needs a preceding open/goto step to bind to"))?;
                json!({"id": id, "intent": intent.take().unwrap_or_else(|| format!("{name} matches golden")),
                       "kind": "check",
                       "claim": {"subject": {"shot": goto}, "predicate": "matches"}})
            }
            "include" => {
                if tail.is_empty() {
                    return Err(err!("include needs a file path"));
                }
                json!({"id": id, "intent": mk_intent(&mut intent), "kind": "do",
                       "verb": "include", "params": {"file": tail}})
            }
            _ => {
                return Err(err!(
                    "unknown statement {head:?} — see `agent-qa compile --help`"
                ));
            }
        };
        push_step(&mut stack, &mut top, step);
    }
    if let Some(f) = stack.last() {
        bail!("unclosed {} block", f.kind);
    }
    let mut doc = json!({
        "schema": "scenario/2",
        "id": default_id,
        "intent": scenario_intent,
        "steps": top,
    });
    if !env_navs.is_empty() {
        doc["env"] = json!({"open": env_navs});
    }
    if !inputs.is_empty() {
        doc["inputs"] = serde_json::to_value(inputs)?;
    }
    Ok(doc)
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// `<locator>` text → locator JSON:
/// `css:X`, `xpath:X`, `testid:X`, `text:X`, `"X"` (text), `role:R ["N"]`
/// (or `role:R:N`), `#…`/`.…`/`[…` css shorthand.
fn parse_locator(s: &str) -> Result<Json> {
    let s = s.trim();
    if s.is_empty() {
        bail!("missing locator");
    }
    if let Some(rest) = s.strip_prefix("css:") {
        return Ok(json!({"raw": {"kind": "css", "value": rest}, "reason": "flow"}));
    }
    if let Some(rest) = s.strip_prefix("xpath:") {
        return Ok(json!({"raw": {"kind": "xpath", "value": rest}, "reason": "flow"}));
    }
    if let Some(rest) = s.strip_prefix("testid:") {
        return Ok(json!({"raw": {"kind": "testId", "value": rest}, "reason": "flow"}));
    }
    if let Some(rest) = s.strip_prefix("text:") {
        return Ok(json!({"raw": {"kind": "text", "value": unquote(rest)}, "reason": "flow"}));
    }
    if let Some(rest) = s.strip_prefix("role:") {
        let (role, name) = rest
            .split_once(char::is_whitespace)
            .map(|(r, n)| (r.trim(), Some(unquote(n.trim()))))
            .or_else(|| {
                rest.split_once(':')
                    .map(|(r, n)| (r.trim(), Some(unquote(n.trim()))))
            })
            .unwrap_or((rest, None));
        let mut loc = json!({"role": {"role": role}});
        if let Some(n) = name {
            loc["role"]["name"] = json!(n);
        }
        return Ok(loc);
    }
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return Ok(json!({"raw": {"kind": "text", "value": unquote(s)}, "reason": "flow"}));
    }
    if s.starts_with('#') || s.starts_with('.') || s.starts_with('[') || s.starts_with('>') {
        return Ok(json!({"raw": {"kind": "css", "value": s}, "reason": "flow"}));
    }
    bail!("locator {s:?} needs a prefix: css:/xpath:/testid:/text:/role: or a # . [ shorthand")
}

/// `fill "Name"` — a quoted fill target means a role-textbox name.
fn parse_locator_fill(s: &str) -> Result<Json> {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return Ok(json!({"role": {"role": "textbox", "name": unquote(s)}}));
    }
    parse_locator(s)
}

/// Split `s` at the end of the leading locator token. Quoted locators
/// (`"a b"`, `role:x "Name"`) may contain spaces; bare forms end at the
/// first whitespace. Returns (locator, rest-of-line).
fn split_locator_rest(s: &str) -> Option<(Json, &str)> {
    let s = s.trim();
    let consumed = if s.starts_with('"') {
        s.strip_prefix('"')?.find('"')? + 2
    } else if let Some(rest) = s.strip_prefix("role:") {
        let role_len = "role:".len();
        let tok_end = rest.find(char::is_whitespace).map(|e| role_len + e);
        match tok_end {
            None => s.len(),
            Some(e) => {
                let after = &s[e..].trim_start();
                if after.starts_with('"') {
                    // role:R "Name"
                    e + (s.len() - e - after.len()) + (after.strip_prefix('"')?.find('"')? + 2)
                } else {
                    e
                }
            }
        }
    } else {
        s.find(char::is_whitespace).unwrap_or(s.len())
    };
    let (loc_s, rest) = s.split_at(consumed);
    Some((parse_locator(loc_s.trim()).ok()?, rest.trim()))
}

// ---------------------------------------------------------------- describe

pub(crate) fn describe(doc: &Json) -> String {
    let mut out = String::new();
    let mut unsupported: Vec<String> = Vec::new();
    if let Some(t) = doc["intent"].as_str() {
        out.push_str(&format!("title {t}\n"));
    }
    if let Some(ops) = doc["env"]["open"].as_array() {
        for op in ops {
            if op["kind"] == "nav" {
                if let Some(u) = op["url"].as_str() {
                    out.push_str(&format!("base {u}\n"));
                }
            }
        }
    }
    if let Some(inputs) = doc["inputs"].as_object() {
        for (name, decl) in inputs {
            match decl["default"].as_str() {
                Some(d) => out.push_str(&format!("input {name} = \"{d}\"\n")),
                None => out.push_str(&format!("input {name}\n")),
            }
        }
    }
    if !out.is_empty() {
        out.push('\n');
    }
    if let Some(steps) = doc["steps"].as_array() {
        describe_steps(steps, 0, &mut out, &mut unsupported);
    }
    for u in &unsupported {
        eprintln!("describe: {u}");
    }
    out
}

fn describe_steps(steps: &[Json], indent: usize, out: &mut String, unsupported: &mut Vec<String>) {
    let pad = "  ".repeat(indent);
    for step in steps {
        let id = step["id"].as_str().unwrap_or("?");
        if let Some(intent) = step["intent"].as_str() {
            out.push_str(&format!("{pad}! {intent}\n"));
        }
        let verb = step["verb"].as_str().unwrap_or("");
        // Structural verbs recurse — emit the opener, children, `end`.
        match verb {
            "group" => {
                out.push_str(&format!("{pad}group\n"));
                if let Some(subs) = step["params"]["steps"].as_array() {
                    describe_steps(subs, indent + 1, out, unsupported);
                }
                out.push_str(&format!("{pad}end\n"));
                continue;
            }
            "when" => {
                let (key, loc) = if let Some(l) = step["params"].get("present") {
                    ("present", l)
                } else if let Some(l) = step["params"].get("absent") {
                    ("absent", l)
                } else {
                    unsupported.push(format!("step {id} (when) has no present/absent locator"));
                    out.push_str(&format!("{pad}# unsupported: {id} (when)\n"));
                    continue;
                };
                match describe_locator(loc) {
                    Some(l) => out.push_str(&format!("{pad}when {key} {l}\n")),
                    None => {
                        unsupported.push(format!("step {id} (when) locator can't be expressed"));
                        out.push_str(&format!("{pad}# unsupported: {id} (when)\n"));
                        continue;
                    }
                }
                if let Some(subs) = step["params"]["steps"].as_array() {
                    describe_steps(subs, indent + 1, out, unsupported);
                }
                out.push_str(&format!("{pad}end\n"));
                continue;
            }
            _ => {}
        }
        match describe_step(step) {
            Some(line) => out.push_str(&format!("{pad}{line}\n")),
            None => {
                let v = if verb.is_empty() { "?" } else { verb };
                unsupported.push(format!("step {id} ({v}) can't be expressed in flow text"));
                out.push_str(&format!("{pad}# unsupported: {id} ({v})\n"));
            }
        }
    }
}

fn describe_locator(loc: &Json) -> Option<String> {
    if let Some(raw) = loc.get("raw") {
        let kind = raw["kind"].as_str()?;
        let val = raw["value"].as_str()?;
        return Some(match kind {
            "css" => format!("css:{val}"),
            "xpath" => format!("xpath:{val}"),
            "testId" => format!("testid:{val}"),
            "text" => format!("\"{val}\""),
            _ => return None,
        });
    }
    if let Some(role) = loc.get("role") {
        let r = role["role"].as_str()?;
        return match role["name"].as_str() {
            Some(n) => Some(format!("role:{r} \"{n}\"")),
            None => Some(format!("role:{r}")),
        };
    }
    None
}

fn literal_value(step: &Json) -> Option<&str> {
    step["value"]["literal"].as_str()
}

fn describe_step(step: &Json) -> Option<String> {
    if step["kind"].as_str()? == "check" {
        let claim = &step["claim"];
        let subj = &claim["subject"];
        let pred = claim["predicate"].as_str()?;
        if subj.get("shot").is_some() {
            return Some(match pred {
                "matches" => "shot page".to_string(),
                _ => return None,
            });
        }
        if subj.get("url").is_some() {
            let v = claim["value"].as_str()?;
            return Some(format!("check url {pred} {v}"));
        }
        if subj.get("console").is_some() {
            return match pred {
                "notExists" => Some("check console quiet".to_string()),
                _ => None,
            };
        }
        let el = subj.get("element")?;
        let loc = describe_locator(el)?;
        let of_kind = subj["ofKind"].as_str();
        return Some(match (of_kind, pred) {
            (None, "exists") => format!("check {loc} exists"),
            (None, "notExists") => format!("check {loc} absent"),
            (None, "isVisible") => format!("check {loc} visible"),
            (None, "isHidden") => format!("check {loc} hidden"),
            (None, "isEnabled") => format!("check {loc} enabled"),
            (None, "isDisabled") => format!("check {loc} disabled"),
            (None, "isChecked") => format!("check {loc} checked"),
            (None, "isUnchecked") => format!("check {loc} unchecked"),
            (Some("text"), p @ ("equals" | "contains" | "matches" | "startsWith" | "endsWith")) => {
                let v = claim["value"].as_str()?;
                format!("check {loc} text {p} \"{v}\"")
            }
            _ => return None,
        });
    }
    let verb = step["verb"].as_str()?;
    let loc = step.get("on").and_then(describe_locator);
    Some(match verb {
        "goto" => format!("open {}", literal_value(step)?),
        "click" => format!("click {}", loc?),
        "dblclick" => format!("dblclick {}", loc?),
        "hover" => format!("hover {}", loc?),
        "check" => format!("tick {}", loc?),
        "uncheck" => format!("untick {}", loc?),
        "clear" => format!("clear {}", loc?),
        "focus" => format!("focus {}", loc?),
        "blur" => format!("blur {}", loc?),
        "type" => {
            let v = literal_value(step)?;
            if step["on"]["role"]["role"].as_str() == Some("textbox")
                && step["on"]["role"]["name"].is_string()
            {
                format!(
                    "fill \"{}\" with \"{v}\"",
                    step["on"]["role"]["name"].as_str()?
                )
            } else {
                format!("type \"{v}\" into {}", loc?)
            }
        }
        "select" => format!("select {} = \"{}\"", loc?, literal_value(step)?),
        "press" => format!("press {}", literal_value(step)?),
        "scrollto" => {
            if let Some(l) = loc {
                format!("scroll {l}")
            } else if let Some(t) = step["params"]["to"].as_str() {
                format!("scroll {t}")
            } else {
                let y = step["params"]["y"].as_u64()?;
                format!("scroll {y}")
            }
        }
        "wait" => {
            let p = &step["params"];
            if let Some(ms) = p["ms"].as_u64() {
                format!("wait {ms}ms")
            } else if let Some(u) = p["url"].as_str() {
                format!("wait url {u}")
            } else if p["idle"].as_bool() == Some(true) {
                "wait idle".to_string()
            } else {
                let l = p.get("locator")?;
                format!("wait {}", describe_locator(l)?)
            }
        }
        "viewport" => {
            let p = &step["params"];
            format!(
                "viewport {}x{}",
                p["width"].as_u64()?,
                p["height"].as_u64()?
            )
        }
        "include" => format!("include {}", step["params"]["file"].as_str()?),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_str(body: &str) -> Json {
        compile(body, "t").unwrap()
    }

    #[test]
    fn compile_basics() {
        let doc = compile_str(
            r#"
# comment
title Login flow
base file:///page.html
input title = "Hello"
open /login
click "Sign in"
fill "Name" with "Rasmus"
type "hi" into css:#note
press Enter
check "Saved" exists
check css:#title text contains "Sav"
check url matches /done$
check console quiet
shot home
"#,
        );
        assert_eq!(doc["intent"], "Login flow");
        assert_eq!(doc["env"]["open"][0]["url"], "file:///page.html");
        assert_eq!(doc["inputs"]["title"]["default"], "Hello");
        let steps = doc["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 10);
        // s1 open, s2 click, s3 fill, s4 type, s5 press,
        // s6 exists-check, s7 text-check, s8 url-check, s9 console, s10 shot
        assert_eq!(steps[0]["verb"], "goto");
        assert_eq!(steps[1]["verb"], "click");
        assert_eq!(steps[1]["on"]["raw"]["kind"], "text");
        assert_eq!(steps[1]["on"]["raw"]["value"], "Sign in");
        assert_eq!(steps[2]["verb"], "type");
        assert_eq!(steps[2]["on"]["role"]["role"], "textbox");
        assert_eq!(steps[2]["on"]["role"]["name"], "Name");
        assert_eq!(steps[2]["value"]["literal"], "Rasmus");
        assert_eq!(steps[4]["verb"], "press");
        assert_eq!(steps[5]["claim"]["predicate"], "exists");
        assert_eq!(steps[6]["claim"]["subject"]["ofKind"], "text");
        assert_eq!(steps[6]["claim"]["value"], "Sav");
        assert_eq!(steps[7]["claim"]["subject"]["url"], true);
        assert_eq!(steps[7]["claim"]["predicate"], "matches");
        assert_eq!(steps[8]["claim"]["predicate"], "notExists");
        assert_eq!(steps[9]["claim"]["subject"]["shot"], "s1"); // bound to `open`
        assert_eq!(steps[9]["intent"], "home matches golden");
    }

    #[test]
    fn compile_blocks_and_include() {
        let doc = compile_str(
            r#"group Login
  include shared/login.json
  when present "Create user"
    click "Create user"
  end
end
"#,
        );
        let steps = doc["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 1);
        let g = &steps[0];
        assert_eq!(g["verb"], "group");
        let inner = g["params"]["steps"].as_array().unwrap();
        assert_eq!(inner[0]["verb"], "include");
        assert_eq!(inner[0]["params"]["file"], "shared/login.json");
        let w = &inner[1];
        assert_eq!(w["verb"], "when");
        assert_eq!(w["params"]["present"]["raw"]["kind"], "text");
        assert_eq!(w["params"]["steps"][0]["verb"], "click");
    }

    #[test]
    fn compile_wait_scroll_select_forms() {
        let doc = compile_str(
            "wait 500ms\nwait url /api/done\nwait css:#ready\nwait idle\nscroll bottom\nscroll 400\nscroll css:#deep\nselect css:#cc = \"Visa\"\n",
        );
        let steps = doc["steps"].as_array().unwrap();
        assert_eq!(steps[0]["params"]["ms"], 500);
        assert_eq!(steps[1]["params"]["url"], "/api/done");
        assert_eq!(steps[2]["params"]["locator"]["raw"]["kind"], "css");
        assert_eq!(steps[3]["params"]["idle"], true);
        assert_eq!(steps[4]["params"]["to"], "bottom");
        assert_eq!(steps[5]["params"]["y"], 400);
        assert_eq!(steps[6]["on"]["raw"]["value"], "#deep");
        assert_eq!(steps[7]["verb"], "select");
        assert_eq!(steps[7]["on"]["raw"]["value"], "#cc");
        assert_eq!(steps[7]["value"]["literal"], "Visa");
    }

    #[test]
    fn compile_intent_lines_and_errors() {
        let doc = compile_str("! press the big button\nclick #go\n");
        assert_eq!(doc["steps"][0]["intent"], "press the big button");

        assert!(compile("end\n", "t").is_err());
        assert!(compile("group x\nclick #a\n", "t").is_err()); // unclosed
        assert!(compile("shot home\n", "t").is_err()); // no goto to bind
        assert!(compile("bogus line\n", "t").is_err());
        assert!(compile("click bogusword\n", "t").is_err());
        assert!(compile("check css:#x bogus\n", "t").is_err());
        assert!(compile("check css:#x text\n", "t").is_err());
        assert!(compile("fill css:#x\n", "t").is_err());
    }

    #[test]
    fn compile_locator_forms() {
        assert_eq!(parse_locator("#a").unwrap()["raw"]["kind"], "css");
        assert_eq!(parse_locator(".cls").unwrap()["raw"]["value"], ".cls");
        assert_eq!(
            parse_locator("role:button \"Save\"").unwrap()["role"]["name"],
            "Save"
        );
        assert_eq!(parse_locator("role:list").unwrap()["role"]["role"], "list");
        assert_eq!(
            parse_locator("role:button:Save").unwrap()["role"]["name"],
            "Save"
        );
        assert_eq!(
            parse_locator("testid:sub").unwrap()["raw"]["kind"],
            "testId"
        );
        assert_eq!(parse_locator("\"a b\"").unwrap()["raw"]["kind"], "text");
    }

    #[test]
    fn describe_roundtrip() {
        let body = r#"title R
base file:///p.html
input title = "x"
open /a
click "Go"
fill "Name" with "x"
check css:#done exists
check url equals file:///p.html
check console quiet
shot page
group
  wait 200ms
end
when absent css:#modal
  click #ok
end
"#;
        let doc = compile_str(body);
        let text = describe(&doc);
        let doc2 = compile_str(&text);
        assert_eq!(doc["steps"], doc2["steps"]);
        assert_eq!(doc["env"], doc2["env"]);
        assert_eq!(doc["inputs"], doc2["inputs"]);
        assert_eq!(doc["intent"], doc2["intent"]);
    }

    #[test]
    fn describe_unsupported_becomes_comment() {
        let doc = serde_json::json!({"steps": [
            {"id": "s1", "intent": "q", "kind": "do", "verb": "callGql",
             "params": {"url": "u", "query": "q"}}
        ]});
        let text = describe(&doc);
        assert!(text.contains("# unsupported: s1 (callGql)"), "{text}");
    }
}
