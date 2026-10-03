//! In-page overlay painted during `--record-video` replays: a Cypress-style
//! left rail listing every step — pending dimmed, current highlighted,
//! pass/fail colored — plus a red ring around the current step's target
//! element, so the recorded video shows what the replay is doing instead of
//! a bare page. Injected per-step via `eval` — navigation wipes the DOM, so
//! each call re-adds the elements; best-effort only, never fails a step.

use serde_json::{json, Value as Json};

use crate::browser;
use crate::scenario::{Locator, NameMatch, RawLocatorKind, Step};
use crate::value::{substitute_scenario_vars, ValueScope};

/// Hide/show the overlay around a screenshot capture — `shot` claims
/// pixel-compare the saved sidecar, and a painted rail would read as
/// page drift. Element ids live under `__aq_`; both are re-created by the
/// next `annotate` anyway, so hiding is a pure `display:none`.
pub fn set_visible(session: &str, visible: bool) {
    let vis = if visible { "" } else { "none" };
    let expr = format!(
        "try{{document.querySelectorAll('#__aq_rail,#__aq_ring').forEach(function(e){{e.style.display='{vis}'}})}}catch(e){{}}"
    );
    let _ = browser::eval_expression(session, &expr);
}

/// Paint the overlay for step `cur` (1-based). `labels` is the full
/// post-window step list; `outcomes[i]` is `Some("pass"|"fail"|"skip")`
/// once a step settles, `None` while pending/running. Errors are swallowed
/// — a dead page or unresolvable locator must not affect replay.
pub fn annotate(
    session: &str,
    labels: &[String],
    cur: u32,
    outcomes: &[Option<&str>],
    step: &Step,
    scope: &mut ValueScope,
) {
    let target = match step {
        Step::Do { on, .. } => on.as_ref(),
        Step::Check { .. } => None,
    }
    .and_then(|loc| locator_query(loc, scope));
    let expr = format!(
        "({OVERLAY_FN})({},{},{})",
        serde_json::to_string(labels).unwrap_or_else(|_| "[]".into()),
        cur,
        serde_json::to_string(&outcomes).unwrap_or_else(|_| "[]".into()),
    );
    let _ = browser::eval_expression(session, &expr);
    let ring = match target {
        Some(t) => format!(
            "(function(q){{{RESOLVE}\n{RING_BODY}}})({})",
            serde_json::to_string(&t).unwrap_or_default()
        ),
        None => {
            "(function(){var o=document.getElementById('__aq_ring');if(o)o.remove()})()".to_string()
        }
    };
    let _ = browser::eval_expression(session, &ring);
}

/// Concise label for one rail row — verb + accessible name for targeted
/// verbs, else the authored intent.
pub fn rail_label(step: &Step) -> String {
    match step {
        Step::Do {
            verb, intent, on, ..
        } => {
            let v = format!("{verb:?}").to_ascii_lowercase();
            let name = match on {
                // Raw selector text is technical noise — the authored
                // intent reads better; role names are human-meaningful.
                Some(Locator::Raw(_)) if !intent.trim().is_empty() => {
                    Some(intent.trim().to_string())
                }
                other => locator_name(other.as_ref())
                    .or_else(|| (!intent.trim().is_empty()).then(|| intent.trim().to_string())),
            };
            match name {
                Some(name) => format!("{v} \"{name}\""),
                None => v,
            }
        }
        Step::Check { intent, .. } => {
            if intent.trim().is_empty() {
                "check".to_string()
            } else {
                format!("check — {}", intent.trim())
            }
        }
    }
}

/// Accessible-name for a row: role name first, then the raw selector text.
fn locator_name(on: Option<&Locator>) -> Option<String> {
    match on? {
        Locator::Role(r) => r.name.as_ref().map(|n| match n {
            NameMatch::Plain(s) => s.clone(),
            NameMatch::Pattern { pattern, .. } => pattern.clone(),
            NameMatch::I18n { i18n_key } => i18n_key.clone(),
        }),
        Locator::Raw(r) => Some(r.raw.value.clone()),
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

/// Ring painter body — runs inside `function(q){...}` after `RESOLVE`.
const RING_BODY: &str = r#"var doc=document;try{
var old=doc.getElementById('__aq_ring');if(old)old.remove();
var el=resolve(doc,q);
if(el&&el.getBoundingClientRect){
 var r=el.getBoundingClientRect();
 if((r.width>0||r.height>0)&&r.bottom>0&&r.right>0&&r.top<window.innerHeight&&r.left<window.innerWidth){
  var ring=doc.createElement('div');ring.id='__aq_ring';
  ring.setAttribute('style','position:fixed;z-index:2147483646;left:'+(r.left-4)+'px;top:'+(r.top-4)+'px;width:'+(r.width+8)+'px;height:'+(r.height+8)+'px;border:3px solid #f43f5e;border-radius:5px;box-shadow:0 0 0 4px rgba(244,63,94,.25),0 0 18px rgba(244,63,94,.55);pointer-events:none;');
  (doc.documentElement||doc.body||doc).appendChild(ring);
 }
}
}catch(e){}"#;

/// (labels, cur, outcomes) → paints the step rail. cur is 1-based.
const OVERLAY_FN: &str = r#"function(labels,cur,outcomes){
var doc=document;try{
var rail=doc.getElementById('__aq_rail');
if(!rail){
 rail=doc.createElement('div');rail.id='__aq_rail';
 rail.setAttribute('style','position:fixed;left:0;top:0;bottom:0;z-index:2147483647;width:230px;background:rgba(15,23,42,.88);color:#cbd5e1;font:500 11px/1.6 ui-monospace,monospace;padding:8px 6px;pointer-events:none;overflow:hidden;border-right:1px solid rgba(148,163,184,.3);');
 (doc.documentElement||doc.body||doc).appendChild(rail);
}
var html='';
for(var i=0;i<labels.length;i++){
 var st=outcomes[i]||'',glyph='·',col='#64748b',op=.45,bg='transparent';
 var n=i+1;
 if(st==='pass'){glyph='✓';col='#4ade80';op=.75}
 else if(st==='fail'){glyph='✗';col='#f87171';op=1;bg='rgba(248,113,113,.12)'}
 else if(st==='skip'){glyph='–';col='#94a3b8';op=.5}
 else if(n===cur){glyph='▸';col='#f8fafc';op=1;bg='rgba(244,63,94,.25)'}
 html+='<div style="opacity:'+op+';background:'+bg+';border-radius:4px;padding:1px 5px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;color:'+col+'"><span style="display:inline-block;width:24px;color:#64748b">'+(n<10?' '+n:n)+'</span> '+glyph+' <span style="color:'+(n===cur||st?col:'#94a3b8')+'">'+labels[i]+'</span></div>';
}
rail.innerHTML=html;
var rows=rail.children;var at=cur-1;
if(rows[at]&&rail.scrollHeight>rail.clientHeight)rail.scrollTop=Math.max(0,at*20-rail.clientHeight/3);
}catch(e){}}"#;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rail_label_prefers_intent_for_raw_locators() {
        let step: Step = serde_json::from_value(json!({
            "kind": "do", "id": "s", "intent": "add the todo",
            "verb": "click", "on": {"raw": {"kind": "css", "value": "#add"}, "reason": "r"}
        }))
        .unwrap();
        assert_eq!(rail_label(&step), "click \"add the todo\"");
    }

    #[test]
    fn rail_label_uses_role_name() {
        let step: Step = serde_json::from_value(json!({
            "kind": "do", "id": "s", "intent": "x",
            "verb": "click", "on": {"role": "button", "name": "Submit"}
        }))
        .unwrap();
        assert_eq!(rail_label(&step), "click \"Submit\"");
    }

    #[test]
    fn rail_label_for_check_uses_intent() {
        let step: Step = serde_json::from_value(json!({
            "kind": "check", "id": "c", "intent": "page rendered",
            "claim": {"subject": {"url": true}, "predicate": "exists"}
        }))
        .unwrap();
        assert_eq!(rail_label(&step), "check — page rendered");
    }
}
