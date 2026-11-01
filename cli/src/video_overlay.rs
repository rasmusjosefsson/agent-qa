//! Two headed-replay surfaces, both in the app's own window.
//!
//! The page overlay (`annotate`) is a fake cursor, a click ripple, and a
//! dashed ring on the step's target. It is painted into the page only
//! while `--record-video` is on.
//!
//! The step log (`sync_step_log`) is a dock on the left of that same
//! window. The page shifts right by the dock's width, including
//! `position:fixed` UI (a transform on `body` makes it the containing
//! block), so the list does not cover the app. The dock lives on
//! `documentElement`, outside that transform, and the cursor is measured
//! after the shift so the ring stays on the target. Finished steps stay
//! quiet, the running step is highlighted, and later steps stay faded.
//! The dock is `aria-hidden` and its rows sit in an open shadow root, so
//! a locator cannot match a step label. Screenshots hide the dock and
//! unshift the page. Headless replays skip it; the workbench already
//! lists the steps.

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value as Json};

use crate::browser;
use crate::scenario::{ClaimSubject, Locator, NameMatch, RawLocatorKind, Step, Verb};
use crate::value::{substitute_scenario_vars, ValueScope};

/// Hide/show the overlay around a screenshot capture — `shot` claims
/// pixel-compare the saved sidecar, and a painted cursor or the step
/// dock would read as page drift. Hiding also unshifts the page; showing
/// shifts it back when the dock is present.
pub fn set_visible(session: &str, visible: bool) {
    let _ = browser::eval_expression(session, &visibility_expr(visible));
}

/// Save a screenshot with the cursor and the step dock hidden, then put
/// them back on the same CDP turn. A separate hide command leaves the
/// page unshifted for the whole screenshot subprocess, and the recording
/// holds that frame until the dock returns.
pub fn capture_clean_screenshot(session: &str, path: &std::path::Path) -> bool {
    let captured = crate::cdp::with_connection(session, |conn| {
        let sid = attach_app_page(conn)?;
        let hidden = conn
            .call_on(
                "Runtime.evaluate",
                json!({
                    "expression": visibility_expr(false),
                    "returnByValue": true,
                }),
                &sid,
            )
            .map(|v| v.get("exceptionDetails").is_none())
            .unwrap_or(false);
        let shot = if hidden {
            let params = screenshot_params(conn, &sid);
            conn.call_on("Page.captureScreenshot", params, &sid)
        } else {
            Err(anyhow::anyhow!("step dock did not hide for the screenshot"))
        };
        let _ = conn.call_on(
            "Runtime.evaluate",
            json!({
                "expression": visibility_expr(true),
                "returnByValue": true,
            }),
            &sid,
        );
        shot
    });
    match captured {
        Ok(result) => write_png(
            path,
            result.get("data").and_then(|v| v.as_str()).unwrap_or(""),
        ),
        Err(e) => {
            if !crate::cdp::is_unavailable(&e) {
                eprintln!("[v2-replay] clean screenshot: {e:#}");
            }
            false
        }
    }
}

fn attach_app_page(conn: &mut crate::cdp::CdpConnection) -> anyhow::Result<String> {
    let targets = conn.call("Target.getTargets", json!({}))?;
    let app_id = targets
        .get("targetInfos")
        .and_then(|t| t.as_array())
        .and_then(|infos| {
            infos.iter().rev().find_map(|t| {
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
            })
        })
        .ok_or_else(|| anyhow::anyhow!("no app page for screenshot"))?;
    let attached = conn.call(
        "Target.attachToTarget",
        json!({ "targetId": app_id, "flatten": true }),
    )?;
    attached
        .get("sessionId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Target.attachToTarget: no sessionId"))
}

/// Viewport shot, or the full document when it is taller or wider than
/// the window — the same case `screenshot --full` covers.
fn screenshot_params(conn: &mut crate::cdp::CdpConnection, sid: &str) -> Json {
    let metrics = conn
        .call_on("Page.getLayoutMetrics", json!({}), sid)
        .unwrap_or(Json::Null);
    let content_w = css_px(&metrics, "/cssContentSize/width");
    let content_h = css_px(&metrics, "/cssContentSize/height");
    let view_w = css_px(&metrics, "/cssLayoutViewport/clientWidth");
    let view_h = css_px(&metrics, "/cssLayoutViewport/clientHeight");
    if content_w > view_w + 1.0 || content_h > view_h + 1.0 {
        json!({
            "format": "png",
            "captureBeyondViewport": true,
            "clip": {
                "x": 0,
                "y": 0,
                "width": content_w,
                "height": content_h,
                "scale": 1,
            },
        })
    } else {
        json!({ "format": "png" })
    }
}

fn css_px(v: &Json, path: &str) -> f64 {
    let Some(n) = v.pointer(path) else {
        return 0.0;
    };
    n.as_f64()
        .or_else(|| n.as_i64().map(|i| i as f64))
        .unwrap_or(0.0)
}

fn write_png(path: &std::path::Path, data: &str) -> bool {
    use base64::Engine;
    let bytes = match base64::engine::general_purpose::STANDARD.decode(data.trim()) {
        Ok(bytes) if !bytes.is_empty() => bytes,
        _ => return false,
    };
    if let Err(e) = std::fs::write(path, bytes) {
        eprintln!("[v2-replay] clean screenshot write: {e}");
        return false;
    }
    true
}

fn visibility_expr(visible: bool) -> String {
    let vis = if visible { "" } else { "none" };
    let show = if visible { "true" } else { "false" };
    format!(
        "(function(show){{{shift}\n\
         try{{document.querySelectorAll('#__aq_rail,#__aq_step,#__aq_ring,#__aq_cursor,#__aq_pulse,#__aq_log').forEach(function(e){{e.style.display='{vis}'}})}}catch(e){{}}\n\
         aqShift(show&&!!document.getElementById('__aq_log'));}})({show})",
        shift = SHIFT_FN,
        vis = vis,
        show = show,
    )
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

/// Paint the step dock before the cursor. The dock shifts the page, so
/// the ring has to be measured after that move. A recording paints the
/// cursor; a visible window also refreshes the dock.
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
    if show_log {
        sync_step_log(session, cur, captions, outcomes);
    }
    if record_on {
        annotate(session, cur, outcomes, step, scope);
    }
}

/// Reserve a strip on the left and move the page — including its
/// `position:fixed` UI — into the rest of the window. Inline styles, not
/// a `<style>` tag, so a page content-security policy cannot block it.
/// The saved inline values go back when the strip is released for a
/// screenshot.
const SHIFT_FN: &str = r#"function aqShift(on){
 var b=document.body;if(!b)return;
 var props=['margin-left','width','max-width','transform','min-height','box-sizing'];
 if(on){
  if(!b.getAttribute('data-aq-shift')){
   var saved={};
   props.forEach(function(p){saved[p]={v:b.style.getPropertyValue(p),i:b.style.getPropertyPriority(p)}});
   b.setAttribute('data-aq-shift',JSON.stringify(saved));
  }
  b.classList.add('__aq_shifted');
  b.style.setProperty('margin-left','320px','important');
  b.style.setProperty('width','calc(100vw - 320px)','important');
  b.style.setProperty('max-width','calc(100vw - 320px)','important');
  b.style.setProperty('transform','translateZ(0)','important');
  b.style.setProperty('min-height','100vh','important');
  b.style.setProperty('box-sizing','border-box','important');
 }else{
  b.classList.remove('__aq_shifted');
  var saved={};
  try{saved=JSON.parse(b.getAttribute('data-aq-shift')||'{}')}catch(e){}
  props.forEach(function(p){
   var prev=saved[p];
   if(prev&&prev.v)b.style.setProperty(p,prev.v,prev.i||'');
   else b.style.removeProperty(p);
  });
  b.removeAttribute('data-aq-shift');
 }
}"#;

/// Rows live in a shadow root on `documentElement` (not inside the shifted
/// body) so the cursor's viewport coordinates still match the target.
const DOCK_BODY: &str = r#"var doc=document;
var oldRail=doc.getElementById('__aq_rail');if(oldRail)oldRail.remove();
var oldStep=doc.getElementById('__aq_step');if(oldStep)oldStep.remove();
var host=doc.getElementById('__aq_log');
if(!host){
 host=doc.createElement('div');
 host.id='__aq_log';
 host.setAttribute('aria-hidden','true');
 host.style.cssText='all:initial;position:fixed;z-index:2147483647;left:0;top:0;bottom:0;width:320px;overflow:auto;background:#f4f4f5;border-right:1px solid #e4e4e7;box-sizing:border-box;pointer-events:auto;';
 (doc.documentElement||doc.body).appendChild(host);
 host.attachShadow({mode:'open'});
}
var root=host.shadowRoot;if(!root)return;
var ol=root.getElementById('rows');
if(!ol){
 var header=doc.createElement('div');
 header.textContent='Steps';
 header.style.cssText='position:sticky;top:0;padding:14px 16px 10px;font:11px/1.3 ui-sans-serif,system-ui,sans-serif;letter-spacing:.08em;text-transform:uppercase;color:#71717a;background:#f4f4f5;border-bottom:1px solid #e4e4e7;';
 ol=doc.createElement('ol');
 ol.id='rows';
 ol.style.cssText='list-style:none;margin:0;padding:8px;';
 root.appendChild(header);
 root.appendChild(ol);
}
ol.textContent='';
var focus=null;
var base='display:flex;gap:8px;align-items:flex-start;padding:8px 10px;border-radius:8px;border-left:3px solid transparent;font:13px/1.4 ui-sans-serif,system-ui,sans-serif;color:#18181b;';
var look={done:'opacity:.55',skip:'opacity:.45',wait:'opacity:.38',now:'opacity:1;background:#fff;font-weight:650;border-left-color:#4f46e5;box-shadow:0 1px 2px rgba(0,0,0,.06)',fail:'opacity:1;background:#fef2f2;border-left-color:#dc2626;color:#991b1b'};
for(var i=0;i<rows.length;i++){
 var s=rows[i];
 var li=doc.createElement('li');
 li.style.cssText=base+(look[s.state]||'');
 var m=doc.createElement('span');
 m.style.cssText='width:1em;flex:none;text-align:center;color:#71717a;';
 m.textContent=s.state==='fail'?'!':(s.state==='now'?'\u203a':(s.state==='wait'?'\u00b7':'\u2713'));
 var n=doc.createElement('span');
 n.style.cssText='width:1.6em;flex:none;font-variant-numeric:tabular-nums;color:#71717a;';
 n.textContent=String(i+1);
 var t=doc.createElement('span');
 t.style.cssText='min-width:0;';
 t.textContent=s.label||'';
 li.appendChild(m);li.appendChild(n);li.appendChild(t);
 ol.appendChild(li);
 if(s.state==='now'||s.state==='fail')focus=li;
}
if(focus){
 var top=focus.offsetTop-(host.clientHeight/2)+(focus.offsetHeight/2);
 host.scrollTop=top<0?0:top;
}
host.style.display='';
aqShift(true);"#;

fn dock_expr(payload: &str) -> String {
    format!(
        "(function(rows){{{shift}\n{body}}})({payload})",
        shift = SHIFT_FN,
        body = DOCK_BODY,
        payload = payload,
    )
}

/// Paint the step dock into the app page. Best-effort: a missing page
/// must not fail the step. One process-wide pass closes a step-log
/// window left by an older build so it cannot steal the next click.
pub fn sync_step_log(session: &str, cur: u32, captions: &[String], outcomes: &[Option<&str>]) {
    close_leftover_log_windows(session);
    let rows: Vec<Json> = captions
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let outcome = outcomes.get(i).copied().flatten();
            json!({ "label": label, "state": row_state(i, cur, outcome) })
        })
        .collect();
    let payload = serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into());
    let _ = browser::eval_expression(session, &dock_expr(&payload));
}

fn close_leftover_log_windows(session: &str) {
    static CLOSED: AtomicBool = AtomicBool::new(false);
    if CLOSED.load(Ordering::Relaxed) {
        return;
    }
    let closed = crate::cdp::with_connection(session, |conn| {
        let targets = conn.call("Target.getTargets", json!({}))?;
        let infos = targets
            .get("targetInfos")
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();
        let mut app_id = None;
        let mut dropped = false;
        for t in &infos {
            if t.get("type").and_then(|v| v.as_str()) != Some("page") {
                continue;
            }
            let url = t.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let id = t.get("targetId").and_then(|v| v.as_str());
            if url.contains("aq-step-log") {
                if let Some(id) = id {
                    let _ = conn.call("Target.closeTarget", json!({ "targetId": id }));
                    dropped = true;
                }
            } else if crate::cdp::is_app_page(url) {
                if let Some(id) = id {
                    app_id = Some(id.to_string());
                }
            }
        }
        Ok((dropped, app_id))
    });
    match closed {
        Ok((dropped, app_id)) => {
            if dropped {
                if let Some(app_id) = app_id {
                    let _ = browser::run(
                        session,
                        ["tab", &app_id],
                        browser::RunOpts::new().capture().lenient(),
                    );
                }
            }
            CLOSED.store(true, Ordering::Relaxed);
        }
        Err(e) if crate::cdp::is_unavailable(&e) => {}
        Err(e) => eprintln!("[v2-replay] step log: {e:#}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{dock_expr, row_state, visibility_expr, DOCK_BODY, POINTER_BODY};

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
    fn step_log_docks_in_the_same_window() {
        assert_eq!(row_state(0, 2, Some("pass")), "done");
        assert_eq!(row_state(1, 2, None), "now");
        assert_eq!(row_state(2, 2, None), "wait");
        assert_eq!(row_state(1, 2, Some("fail")), "fail");
        assert_eq!(row_state(0, 1, None), "now");
        assert!(DOCK_BODY.contains("id='__aq_log'"));
        assert!(DOCK_BODY.contains("aria-hidden"));
        assert!(DOCK_BODY.contains("attachShadow"));
        assert!(DOCK_BODY.contains("done:'opacity:.55'"));
        assert!(DOCK_BODY.contains("wait:'opacity:.38'"));
        assert!(DOCK_BODY.contains("now:'opacity:1"));
        assert!(DOCK_BODY.contains("aqShift(true)"));
        let expr = dock_expr("[]");
        assert!(expr.contains("translateZ(0)"));
        assert!(expr.contains("margin-left"));
        assert!(!expr.contains("createTarget"));
        assert!(!expr.contains("newWindow"));
        let hidden = visibility_expr(false);
        assert!(hidden.contains("#__aq_log"));
        assert!(hidden.contains("__aq_shifted"));
        assert!(hidden.contains("})(false)"));
        assert!(visibility_expr(true).contains("})(true)"));
        assert!(crate::cdp::is_app_page("http://127.0.0.1:8765/"));
        assert!(!crate::cdp::is_app_page("data:text/html,aq-step-log"));
    }
}
