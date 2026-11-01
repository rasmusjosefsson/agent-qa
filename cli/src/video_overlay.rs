//! In-page overlay painted during `--record-video` replays: a fake cursor
//! that glides to each step's target and ripples on click verbs, plus a
//! dashed sky ring around that target (checks flip the ring green/red
//! with the outcome). The workbench already lists the steps, so this does
//! not paint a second list over the page. Injected per-step via `eval` —
//! navigation wipes the DOM, so each call re-adds the elements;
//! best-effort only, never fails a step.

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
        "try{{document.querySelectorAll('#__aq_rail,#__aq_ring,#__aq_cursor,#__aq_pulse').forEach(function(e){{e.style.display='{vis}'}})}}catch(e){{}}"
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
/// after `RESOLVE`. Boxes every resolved target (dashed sky; a check
/// flips the box green/red once the outcome lands), glides the fake
/// cursor to the target's center, and ripples on click verbs. With no
/// resolvable target the ring comes down but the cursor stays parked —
/// jumping it off-screen between steps reads as flicker in the video.
const POINTER_BODY: &str = r##"var doc=document;try{
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
var cur=doc.getElementById('__aq_cursor');
if(!cur){
 cur=doc.createElement('div');cur.id='__aq_cursor';
 cur.setAttribute('style','position:fixed;z-index:2147483647;width:32px;height:32px;pointer-events:none;transition:left .28s ease,top .28s ease;left:'+cx+'px;top:'+cy+'px;filter:drop-shadow(0 2px 3px rgba(0,0,0,.65));');
 cur.innerHTML='<svg width="32" height="32" viewBox="0 0 24 24"><path d="M4 2 L20 12 L13 13 L17 20 L14 21.5 L10 14.5 L4 19 Z" fill="#fff" stroke="#0f172a" stroke-width="2.2"/></svg>';
 (doc.documentElement||doc.body||doc).appendChild(cur);
 // Let it mount at the target before any transition kicks in.
 cur.getBoundingClientRect();
}
cur.style.left=(cx-5)+'px';cur.style.top=(cy-3)+'px';
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

#[cfg(test)]
mod tests {
    use super::POINTER_BODY;

    #[test]
    fn overlay_paints_cursor_and_ring_not_a_step_list() {
        assert!(POINTER_BODY.contains("__aq_cursor"));
        assert!(POINTER_BODY.contains("__aq_ring"));
        assert!(POINTER_BODY.contains("__aq_pulse"));
        assert!(
            !POINTER_BODY.contains("__aq_rail"),
            "the in-page step list covers the page under test"
        );
    }
}
