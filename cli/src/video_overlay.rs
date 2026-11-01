//! Two headed-replay surfaces.
//!
//! The page overlay (`annotate`) is a fake cursor, a click ripple, and a
//! dashed ring on the step's target. It is painted into the page only
//! while `--record-video` is on, and it does not list steps — a list on
//! the page covers the UI under test.
//!
//! The step log (`sync_step_log`) is a separate window tiled to the left
//! of the app, the way a command log sits beside the page. Finished steps
//! stay quiet, the running step is highlighted, and later steps stay
//! faded. Navigation cannot wipe it, and it is not part of the page, so
//! locators and screenshots stay on the app. Headless replays skip it;
//! the workbench already lists the steps.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};
use serde_json::{json, Value as Json};

use crate::browser;
use crate::scenario::{ClaimSubject, Locator, NameMatch, RawLocatorKind, Step, Verb};
use crate::value::{substitute_scenario_vars, ValueScope};

/// Hide/show the overlay around a screenshot capture — `shot` claims
/// pixel-compare the saved sidecar, and a painted cursor would read as
/// page drift. Element ids live under `__aq_`; the next `annotate`
/// re-creates them, so hiding is a pure `display:none`.
pub fn set_visible(session: &str, visible: bool) {
    let vis = if visible { "" } else { "none" };
    let expr = format!(
        "try{{document.querySelectorAll('#__aq_rail,#__aq_step,#__aq_ring,#__aq_cursor,#__aq_pulse').forEach(function(e){{e.style.display='{vis}'}})}}catch(e){{}}"
        , vis = vis
    );
    let _ = browser::eval_expression(session, &expr);
}

/// Paint the cursor and target ring for step `cur` (1-based).
/// `outcomes[i]` is `Some("pass"|"fail"|"skip")` once a step settles,
/// `None` while pending/running. Errors are swallowed — a dead page or
/// unresolvable locator must not affect replay.
pub fn annotate(
    session: &str,
    cur: u32,
    outcomes: &[Option<&str>],
    step: &Step,
    scope: &mut ValueScope,
) {
    let (target, kind, click) = match step {
        Step::Do { on, verb, .. } => (
            on.as_ref(),
            "do",
            matches!(
                verb,
                Verb::Click
                    | Verb::DblClick
                    | Verb::RightClick
                    | Verb::Check
                    | Verb::Uncheck
                    | Verb::Select
                    | Verb::Download
                    | Verb::Dismiss
            ),
        ),
        Step::Check { claim, .. } => {
            let loc = match &claim.subject {
                ClaimSubject::Element { element, .. } => Some(element),
                ClaimSubject::Shot { clip, .. } => clip.as_ref(),
                _ => None,
            };
            (loc, "check", false)
        }
    };
    let target = target.and_then(|loc| locator_query(loc, scope));
    // Fresh outcome for this step (set post-dispatch; None while running).
    let st = outcomes
        .get(cur.saturating_sub(1) as usize)
        .copied()
        .flatten();
    // Drop a rail left by an older build. The step list lives in the
    // workbench; painting it on the page covers the UI under test.
    let _ = browser::eval_expression(
        session,
        "try{var r=document.getElementById('__aq_rail');if(r)r.remove()}catch(e){}",
    );
    let painter = format!(
        "(function(q,kind,click,st){{{RESOLVE}\n{POINTER_BODY}}})({},{},{},{})",
        serde_json::to_string(&target).unwrap_or_else(|_| "null".into()),
        serde_json::to_string(kind).unwrap_or_default(),
        click,
        serde_json::to_string(&st).unwrap_or_else(|_| "null".into()),
    );
    let _ = browser::eval_expression(session, &painter);
}

/// One line for the step log: the author's intent, or the verb when the
/// step has no intent.
pub(crate) fn step_caption(step: &Step) -> String {
    let intent = step.intent().trim();
    if !intent.is_empty() {
        return intent.to_string();
    }
    match step {
        Step::Do { verb, .. } => format!("{verb:?}").to_ascii_lowercase(),
        Step::Check { .. } => "check".to_string(),
    }
}

/// Lower a step locator to the JSON descriptor the page script resolves.
/// Best-effort: returns None for anything it can't express, which just
/// means no highlight ring this step.
fn locator_query(loc: &Locator, scope: &mut ValueScope) -> Option<Json> {
    match loc {
        Locator::Raw(raw) => {
            let v = substitute_scenario_vars(&raw.raw.value, scope);
            Some(match raw.raw.kind {
                RawLocatorKind::Css => json!({"css": v}),
                RawLocatorKind::TestId => json!({"testId": v}),
                RawLocatorKind::Xpath => json!({"xpath": v}),
                RawLocatorKind::Text => json!({"text": v}),
            })
        }
        Locator::Role(r) => {
            let name = r.name.as_ref().map(|n| match n {
                NameMatch::Plain(s) => substitute_scenario_vars(s, scope),
                NameMatch::Pattern { pattern, .. } => substitute_scenario_vars(pattern, scope),
                NameMatch::I18n { i18n_key } => i18n_key.clone(),
            });
            Some(json!({"role": r.role, "name": name}))
        }
    }
}

/// `function resolve(doc,q)` — lowers the JSON descriptor to a DOM element.
/// Embedded inside the ring painter's IIFE. Returns null on anything it
/// can't express; lookups never throw outward.
const RESOLVE: &str = r#"function resolve(doc,q){
if(!q)return null;
function first(sel){try{return doc.querySelector(sel)}catch(e){return null}}
if(q.css)return first(q.css);
if(q.testId)return first('[data-testid="'+q.testId+'"],[data-test-id="'+q.testId+'"],[data-test="'+q.testId+'"]');
if(q.xpath){try{return doc.evaluate(q.xpath,doc,null,XPathResult.FIRST_ORDERED_NODE_TYPE,null).singleNodeValue}catch(e){return null}}
if(q.text){try{
 var w=doc.createTreeWalker(doc.body||doc.documentElement,NodeFilter.SHOW_ELEMENT,null),n;
 while(n=w.nextNode()){if(n.childElementCount===0&&(n.textContent||'').trim()===q.text)return n}
}catch(e){}return null}
if(q.role){try{
 var tags={button:'button,input[type=button],input[type=submit],input[type=reset],[role=button]',link:'a[href],[role=link]',textbox:'input:not([type]),input[type=text],input[type=email],input[type=password],input[type=search],input[type=tel],input[type=url],textarea,[role=textbox]',checkbox:'input[type=checkbox],[role=checkbox]',radio:'input[type=radio],[role=radio]',heading:'h1,h2,h3,h4,h5,h6,[role=heading]',img:'img,[role=img]',listbox:'select,[role=listbox]',option:'option,[role=option]',combobox:'select,[role=combobox]',slider:'input[type=range],[role=slider]',tab:'[role=tab]',menuitem:'[role=menuitem]',navigation:'nav,[role=navigation]',main:'main,[role=main]',form:'form,[role=form]',dialog:'dialog,[role=dialog]',alert:'[role=alert]',list:'ul,ol,[role=list]',listitem:'li,[role=listitem]',cell:'td,[role=cell]',columnheader:'th,[role=columnheader]',row:'tr,[role=row]',table:'table,[role=table]'};
 var cand=doc.querySelectorAll('[role="'+q.role+'"]'+(tags[q.role]?','+tags[q.role]:''));
 for(var i=0;i<cand.length;i++){var c=cand[i];
  if(!q.name)return c;
  var nm=(c.getAttribute('aria-label')||'')||((c.textContent||'').trim())||((c.value||''));
  if(nm===q.name||nm.indexOf(q.name)>=0)return c;
 }
}catch(e){}return null}
return null;
}"#;

/// Pointer painter body — runs inside `function(q,kind,click,st){...}`
/// after `RESOLVE`. Drops a step pill or rail left by an older build,
/// then boxes every resolved target (dashed sky; a check flips the box
/// green/red once the outcome lands), glides the fake cursor to the
/// target's center, and ripples on click verbs. With no resolvable
/// target the ring comes down but the cursor stays parked — jumping it
/// off-screen between steps reads as flicker in the video.
const POINTER_BODY: &str = r##"var doc=document;try{
var stepEl=doc.getElementById('__aq_step');if(stepEl)stepEl.remove();
var rail=doc.getElementById('__aq_rail');if(rail)rail.remove();
var old=doc.getElementById('__aq_ring');if(old)old.remove();
var el=resolve(doc,q);
if(!el||!el.getBoundingClientRect)return;
var r=el.getBoundingClientRect();
if(!(r.width>0||r.height>0)||r.bottom<=0||r.right<=0||r.top>=window.innerHeight||r.left>=window.innerWidth)return;
var cx=r.left+r.width/2,cy=r.top+r.height/2;
// The box shows where the step acted. Checks recolor it with the
// outcome; clicks keep the sky dash and add the cursor ripple below.
var col='#38bdf8',dash=true;
if(kind==='check'){
if(st==='pass'){col='#22c55e';dash=false}
else if(st==='fail'||st==='skip'){col='#ef4444';dash=false}
}
var ring=doc.createElement('div');ring.id='__aq_ring';
ring.setAttribute('style','position:fixed;z-index:2147483646;left:'+(r.left-4)+'px;top:'+(r.top-4)+'px;width:'+(r.width+8)+'px;height:'+(r.height+8)+'px;border:'+(dash?'3px dashed':'3px solid')+' '+col+';border-radius:5px;box-shadow:0 0 0 4px '+col+'40,0 0 18px '+col+'8c;pointer-events:none;');
(doc.documentElement||doc.body||doc).appendChild(ring);
var cursor=doc.getElementById('__aq_cursor');
if(!cursor){
 cursor=doc.createElement('div');cursor.id='__aq_cursor';
 cursor.setAttribute('style','position:fixed;z-index:2147483647;width:32px;height:32px;pointer-events:none;transition:left .28s ease,top .28s ease;left:'+cx+'px;top:'+cy+'px;filter:drop-shadow(0 2px 3px rgba(0,0,0,.65));');
 cursor.innerHTML='<svg width="32" height="32" viewBox="0 0 24 24"><path d="M4 2 L20 12 L13 13 L17 20 L14 21.5 L10 14.5 L4 19 Z" fill="#fff" stroke="#0f172a" stroke-width="2.2"/></svg>';
 (doc.documentElement||doc.body||doc).appendChild(cursor);
 // Let it mount at the target before any transition kicks in.
 cursor.getBoundingClientRect();
}
cursor.style.left=(cx-5)+'px';cursor.style.top=(cy-3)+'px';
if(click){
 var old2=doc.getElementById('__aq_pulse');if(old2)old2.remove();
 var p=doc.createElement('div');p.id='__aq_pulse';
 p.setAttribute('style','position:fixed;z-index:2147483646;left:'+(cx-48)+'px;top:'+(cy-48)+'px;width:96px;height:96px;border:4px solid rgba(244,63,94,.95);border-radius:50%;background:radial-gradient(circle,rgba(244,63,94,.85) 0%,rgba(244,63,94,.4) 38%,rgba(244,63,94,0) 68%);pointer-events:none;animation:__aq_glow .8s ease-out forwards;');
 if(!doc.getElementById('__aq_style')){
  var s=doc.createElement('style');s.id='__aq_style';
  s.textContent='@keyframes __aq_glow{0%{transform:scale(.3);opacity:0}12%{opacity:1}60%{opacity:.9}100%{transform:scale(1.45);opacity:0}}';
  (doc.documentElement||doc.body||doc).appendChild(s);
 }
 (doc.documentElement||doc.body||doc).appendChild(p);
}
}catch(e){}"##;

/// How one row in the step log reads. Finished steps are quiet, the
/// running step is the one at full strength, and steps not reached yet
/// stay faded.
pub(crate) fn row_state(index: usize, cur: u32, outcome: Option<&str>) -> &'static str {
    match outcome {
        Some("fail") => "fail",
        Some("skip") => "skip",
        Some("pass") => "done",
        _ => {
            let n = u32::try_from(index).unwrap_or(u32::MAX).saturating_add(1);
            if n == cur {
                "now"
            } else if n > cur {
                "wait"
            } else {
                "done"
            }
        }
    }
}

/// Paint the in-page cursor when a recording is on, and refresh the
/// beside-the-app step log when the browser window is visible.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    session: &str,
    show_log: bool,
    record_on: bool,
    cur: u32,
    captions: &[String],
    outcomes: &[Option<&str>],
    step: &Step,
    scope: &mut ValueScope,
) {
    if record_on {
        annotate(session, cur, outcomes, step, scope);
    }
    if show_log {
        sync_step_log(session, cur, captions, outcomes);
    }
}

const LOG_HTML: &str = r#"<!doctype html>
<!-- aq-step-log -->
<html><head><meta charset="utf-8"><title>Steps</title>
<style>
html,body{margin:0;height:100%;background:#f4f4f5;color:#18181b;font:13px/1.4 ui-sans-serif,system-ui,sans-serif}
header{position:sticky;top:0;padding:14px 16px 10px;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:#71717a;background:#f4f4f5;border-bottom:1px solid #e4e4e7}
ol{list-style:none;margin:0;padding:8px}
li{display:flex;gap:8px;align-items:flex-start;padding:8px 10px;border-radius:8px;border-left:3px solid transparent}
li .m{width:1em;flex:none;text-align:center;color:#71717a}
li .n{width:1.6em;flex:none;font-variant-numeric:tabular-nums;color:#71717a}
li .t{min-width:0}
li.done{opacity:.55}
li.skip{opacity:.45}
li.wait{opacity:.38}
li.now{opacity:1;background:#fff;font-weight:650;border-left-color:#4f46e5;box-shadow:0 1px 2px rgba(0,0,0,.06)}
li.fail{opacity:1;background:#fef2f2;border-left-color:#dc2626;color:#991b1b}
</style></head><body>
<header>Steps</header>
<ol id="rows"></ol>
<script>
window.__aqRender=function(rows){
 var ol=document.getElementById('rows');
 ol.textContent='';
 var focus=null;
 rows.forEach(function(s,i){
  var li=document.createElement('li');
  li.className=s.state;
  var m=document.createElement('span');m.className='m';
  m.textContent=s.state==='fail'?'!':(s.state==='now'?'\u203a':(s.state==='wait'?'\u00b7':'\u2713'));
  var n=document.createElement('span');n.className='n';n.textContent=String(i+1);
  var t=document.createElement('span');t.className='t';t.textContent=s.label||'';
  li.appendChild(m);li.appendChild(n);li.appendChild(t);
  ol.appendChild(li);
  if(s.state==='now'||s.state==='fail')focus=li;
 });
 if(focus)focus.scrollIntoView({block:'center'});
};
</script></body></html>"#;

struct LogSlot {
    target_id: String,
    session_id: String,
}

fn log_slots() -> &'static Mutex<HashMap<String, LogSlot>> {
    static SLOTS: OnceLock<Mutex<HashMap<String, LogSlot>>> = OnceLock::new();
    SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Open (once) a step-log window beside the app and repaint its rows.
/// Best-effort: a missing browser socket must not fail the step.
pub fn sync_step_log(session: &str, cur: u32, captions: &[String], outcomes: &[Option<&str>]) {
    let rows: Vec<Json> = captions
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let outcome = outcomes.get(i).copied().flatten();
            json!({ "label": label, "state": row_state(i, cur, outcome) })
        })
        .collect();
    let payload = serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into());
    if let Err(e) = crate::cdp::with_connection(session, |conn| push_log(conn, session, &payload)) {
        if !crate::cdp::is_unavailable(&e) {
            eprintln!("[v2-replay] step log: {e:#}");
        }
    }
}

fn push_log(conn: &mut crate::cdp::CdpConnection, session: &str, payload: &str) -> Result<()> {
    let targets = conn.call("Target.getTargets", json!({}))?;
    let infos = targets
        .get("targetInfos")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    let app_id = infos.iter().rev().find_map(|t| {
        if t.get("type").and_then(|v| v.as_str()) != Some("page") {
            return None;
        }
        let url = t.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if !crate::cdp::is_app_page(url) {
            return None;
        }
        t.get("targetId")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    });
    let Some(app_id) = app_id else {
        return Ok(());
    };
    let mut slots = log_slots()
        .lock()
        .map_err(|_| anyhow!("step log lock poisoned"))?;
    let known = slots.get(session).and_then(|slot| {
        infos
            .iter()
            .any(|t| t.get("targetId").and_then(|v| v.as_str()) == Some(slot.target_id.as_str()))
            .then(|| slot.session_id.clone())
            .map(|session_id| (slot.target_id.clone(), session_id))
    });
    let (_target_id, session_id) = if let Some(pair) = known {
        pair
    } else {
        // Drop leftover log tabs. A tab in the app's window is what the
        // next click would hit, and it is not a panel beside the page.
        for id in log_target_ids(&infos) {
            let _ = conn.call("Target.closeTarget", json!({ "targetId": id }));
        }
        let created = conn.call(
            "Target.createTarget",
            json!({
                "url": log_data_url(),
                "newWindow": true,
                "background": true,
                "width": 380,
                "height": 800,
            }),
        )?;
        let target_id = created
            .get("targetId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Target.createTarget: no targetId"))?
            .to_string();
        let session_id = attach(conn, &target_id)?;
        wait_log_ready(conn, &session_id)?;
        if let Err(e) = tile(conn, &app_id, &target_id) {
            eprintln!("[v2-replay] step log place: {e:#}");
        }
        let _ = conn.call("Target.activateTarget", json!({ "targetId": &app_id }));
        slots.insert(
            session.to_string(),
            LogSlot {
                target_id: target_id.clone(),
                session_id: session_id.clone(),
            },
        );
        (target_id, session_id)
    };
    drop(slots);
    conn.call_on(
        "Runtime.evaluate",
        json!({
            "expression": format!("window.__aqRender&&window.__aqRender({payload})"),
            "returnByValue": true,
        }),
        &session_id,
    )?;
    // Creating the log window makes it the daemon's current tab. The next
    // step must keep driving the app, not the list.
    refocus_app(session, &app_id);
    Ok(())
}

fn log_target_ids(infos: &[Json]) -> Vec<String> {
    infos
        .iter()
        .filter_map(|t| {
            let url = t.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if url.contains("aq-step-log") {
                t.get("targetId")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            } else {
                None
            }
        })
        .collect()
}

/// The document is the data URL, so the load itself is the log. The marker
/// stays in the URL so this window is never treated as the app.
fn log_data_url() -> String {
    let mut out = String::from("data:text/html;charset=utf-8,");
    for b in LOG_HTML.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn wait_log_ready(conn: &mut crate::cdp::CdpConnection, session_id: &str) -> Result<()> {
    for _ in 0..25 {
        let ready = conn.call_on(
            "Runtime.evaluate",
            json!({
                "expression": "typeof window.__aqRender",
                "returnByValue": true,
            }),
            session_id,
        )?;
        let kind = ready
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if kind == "function" {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    anyhow::bail!("step log did not load");
}

fn refocus_app(session: &str, app_id: &str) {
    let _ = crate::browser::run(
        session,
        ["tab", app_id],
        crate::browser::RunOpts::new().capture().lenient(),
    );
}

fn attach(conn: &mut crate::cdp::CdpConnection, target_id: &str) -> Result<String> {
    let attached = conn.call(
        "Target.attachToTarget",
        json!({ "targetId": target_id, "flatten": true }),
    )?;
    attached
        .get("sessionId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Target.attachToTarget: no sessionId"))
}

/// Put the log on the left and the app in the remaining screen, flush,
/// like a command log beside the page.
fn tile(conn: &mut crate::cdp::CdpConnection, app_id: &str, log_id: &str) -> Result<()> {
    let (x, y, w, h) = screen_box(conn, app_id)?;
    if w < 900 || h < 500 {
        return Ok(());
    }
    let log_w = 380;
    // Leave maximized state before a move, or the bounds are ignored.
    set_bounds(conn, log_id, x, y, log_w, h)?;
    set_bounds(conn, app_id, x + log_w, y, w - log_w, h)?;
    Ok(())
}

fn screen_box(conn: &mut crate::cdp::CdpConnection, app_id: &str) -> Result<(i64, i64, i64, i64)> {
    let app_session = attach(conn, app_id)?;
    let screen = conn.call_on(
        "Runtime.evaluate",
        json!({
            "expression": "JSON.stringify({x:screen.availLeft||0,y:screen.availTop||0,w:screen.availWidth||0,h:screen.availHeight||0})",
            "returnByValue": true,
        }),
        &app_session,
    )?;
    let raw = screen
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let box_: Json = serde_json::from_str(raw).unwrap_or(Json::Null);
    let x = box_.get("x").and_then(|v| v.as_i64()).unwrap_or(0);
    let y = box_.get("y").and_then(|v| v.as_i64()).unwrap_or(0);
    let w = box_.get("w").and_then(|v| v.as_i64()).unwrap_or(0);
    let h = box_.get("h").and_then(|v| v.as_i64()).unwrap_or(0);
    if w >= 900 && h >= 500 {
        return Ok((x, y, w, h));
    }
    let win = conn.call("Browser.getWindowForTarget", json!({ "targetId": app_id }))?;
    let bounds = win.get("bounds").cloned().unwrap_or(Json::Null);
    Ok((
        bounds.get("left").and_then(|v| v.as_i64()).unwrap_or(0),
        bounds.get("top").and_then(|v| v.as_i64()).unwrap_or(0),
        bounds.get("width").and_then(|v| v.as_i64()).unwrap_or(w),
        bounds.get("height").and_then(|v| v.as_i64()).unwrap_or(h),
    ))
}

fn set_bounds(
    conn: &mut crate::cdp::CdpConnection,
    target_id: &str,
    left: i64,
    top: i64,
    width: i64,
    height: i64,
) -> Result<()> {
    let win = conn.call(
        "Browser.getWindowForTarget",
        json!({ "targetId": target_id }),
    )?;
    let window_id = win
        .get("windowId")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| anyhow!("Browser.getWindowForTarget: no windowId"))?;
    conn.call(
        "Browser.setWindowBounds",
        json!({ "windowId": window_id, "bounds": { "windowState": "normal" } }),
    )?;
    conn.call(
        "Browser.setWindowBounds",
        json!({
            "windowId": window_id,
            "bounds": {
                "left": left,
                "top": top,
                "width": width,
                "height": height,
                "windowState": "normal",
            },
        }),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{row_state, LOG_HTML, POINTER_BODY};

    #[test]
    fn overlay_paints_the_action_and_not_a_step_list() {
        assert!(POINTER_BODY.contains("__aq_cursor"));
        assert!(POINTER_BODY.contains("__aq_ring"));
        assert!(POINTER_BODY.contains("__aq_pulse"));
        assert!(POINTER_BODY.contains("if(rail)rail.remove()"));
        assert!(POINTER_BODY.contains("if(stepEl)stepEl.remove()"));
        assert!(!POINTER_BODY.contains("pinRight"));
        assert!(!POINTER_BODY.contains("id='__aq_rail'"));
    }

    #[test]
    fn step_log_fades_finished_and_upcoming_rows() {
        assert_eq!(row_state(0, 2, Some("pass")), "done");
        assert_eq!(row_state(1, 2, None), "now");
        assert_eq!(row_state(2, 2, None), "wait");
        assert_eq!(row_state(1, 2, Some("fail")), "fail");
        assert_eq!(row_state(0, 1, None), "now");
        assert!(LOG_HTML.contains("li.done{opacity:.55}"));
        assert!(LOG_HTML.contains("li.wait{opacity:.38}"));
        assert!(LOG_HTML.contains("li.now{opacity:1"));
        assert!(crate::cdp::is_app_page("http://127.0.0.1:8765/"));
        assert!(!crate::cdp::is_app_page("data:text/html,aq-step-log"));
    }
}
