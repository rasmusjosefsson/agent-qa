//! In-page overlay painted during `--record-video` replays: a caption bar
//! naming the step about to run plus a highlight ring around its target
//! element, so the recorded video shows what the replay is doing instead of
//! a bare page. Injected per-step via `eval` — navigation wipes the DOM, so
//! each call re-adds the elements; best-effort only, never fails a step.

use serde_json::{json, Value as Json};

use crate::browser;
use crate::scenario::{Locator, NameMatch, RawLocatorKind, Step};
use crate::value::{substitute_scenario_vars, ValueScope};

/// Paint the overlay for one step. `label` is the same progress label the
/// terminal shows. Errors are swallowed — a dead page or unresolvable
/// locator must not affect replay.
pub fn annotate(session: &str, idx: u32, total: u32, step: &Step, scope: &mut ValueScope) {
    let label = step_label(idx, total, step);
    let target = match step {
        Step::Do { on, .. } => on.as_ref(),
        Step::Check { .. } => None,
    }
    .and_then(|loc| locator_query(loc, scope));
    let expr = build_expr(&label, target.as_ref());
    let _ = browser::eval_expression(session, &expr);
}

fn step_label(idx: u32, total: u32, step: &Step) -> String {
    let body = match step {
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
    };
    format!("[{idx}/{total}] {body}")
}

/// Accessible-name for the caption: role name first, then the raw selector
/// text so `css:`/`text:` targets still read meaningfully.
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

fn build_expr(label: &str, target: Option<&Json>) -> String {
    let label_js = serde_json::to_string(label).unwrap_or_else(|_| "\"\"".into());
    let target_js = serde_json::to_string(&target.cloned().unwrap_or(Json::Null))
        .unwrap_or_else(|_| "null".into());
    format!("({OVERLAY_FN})({label_js},{target_js})")
}

/// Self-contained painter: (label, query) → void. Creates or reuses a
/// fixed caption bar; replaces the previous highlight ring. All lookups
/// are wrapped — a bad selector must not throw into `eval`.
const OVERLAY_FN: &str = r#"function(label,q){
var doc=document;try{
var hud=doc.getElementById('__aq_hud');
if(!hud){
 hud=doc.createElement('div');hud.id='__aq_hud';
 hud.setAttribute('style','position:fixed;left:10px;bottom:10px;z-index:2147483647;background:rgba(15,23,42,.92);color:#fff;font:600 13px/1.35 ui-monospace,monospace;padding:6px 10px;border-radius:6px;pointer-events:none;max-width:82vw;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;border:1px solid rgba(148,163,184,.4);');
 (doc.documentElement||doc.body||doc).appendChild(hud);
}
hud.textContent=label;
var old=doc.getElementById('__aq_ring');if(old)old.remove();
var el=null;
function first(sel){try{return doc.querySelector(sel)}catch(e){return null}}
if(q){
 if(q.css)el=first(q.css);
 else if(q.testId)el=first('[data-testid="'+q.testId+'"],[data-test-id="'+q.testId+'"],[data-test="'+q.testId+'"]');
 else if(q.xpath){try{el=doc.evaluate(q.xpath,doc,null,XPathResult.FIRST_ORDERED_NODE_TYPE,null).singleNodeValue}catch(e){}}
 else if(q.text){try{
   var w=doc.createTreeWalker(doc.body||doc.documentElement,NodeFilter.SHOW_ELEMENT,null),n;
   while(n=w.nextNode()){if(n.childElementCount===0&&(n.textContent||'').trim()===q.text){el=n;break}}
 }catch(e){}}
 else if(q.role){try{
   var tags={button:'button,input[type=button],input[type=submit],input[type=reset],[role=button]',link:'a[href],[role=link]',textbox:'input:not([type]),input[type=text],input[type=email],input[type=password],input[type=search],input[type=tel],input[type=url],textarea,[role=textbox]',checkbox:'input[type=checkbox],[role=checkbox]',radio:'input[type=radio],[role=radio]',heading:'h1,h2,h3,h4,h5,h6,[role=heading]',img:'img,[role=img]',listbox:'select,[role=listbox]',option:'option,[role=option]',combobox:'select,[role=combobox]',slider:'input[type=range],[role=slider]',tab:'[role=tab]',menuitem:'[role=menuitem]',navigation:'nav,[role=navigation]',main:'main,[role=main]',form:'form,[role=form]',dialog:'dialog,[role=dialog]',alert:'[role=alert]',list:'ul,ol,[role=list]',listitem:'li,[role=listitem]',cell:'td,[role=cell]',columnheader:'th,[role=columnheader]',row:'tr,[role=row]',table:'table,[role=table]'};
   var cand=doc.querySelectorAll('[role="'+q.role+'"]'+(tags[q.role]?','+tags[q.role]:''));
   for(var i=0;i<cand.length;i++){var c=cand[i];
     if(!q.name){el=c;break}
     var nm=(c.getAttribute('aria-label')||'')||((c.textContent||'').trim())||((c.value||''));
     if(nm===q.name||nm.indexOf(q.name)>=0){el=c;break}
   }
 }catch(e){}}
}
if(el&&el.getBoundingClientRect){
 var r=el.getBoundingClientRect();
 if((r.width>0||r.height>0)&&r.bottom>0&&r.right>0&&r.top<window.innerHeight&&r.left<window.innerWidth){
  var ring=doc.createElement('div');ring.id='__aq_ring';
  ring.setAttribute('style','position:fixed;z-index:2147483646;left:'+(r.left-4)+'px;top:'+(r.top-4)+'px;width:'+(r.width+8)+'px;height:'+(r.height+8)+'px;border:3px solid #f43f5e;border-radius:5px;box-shadow:0 0 0 4px rgba(244,63,94,.25),0 0 18px rgba(244,63,94,.55);pointer-events:none;');
  (doc.documentElement||doc.body||doc).appendChild(ring);
 }
}
}catch(e){}}"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expr_embeds_label_and_target_safely() {
        let e = build_expr("click \"a b\"", Some(&json!({"css": "#x\"y"})));
        assert!(e.contains("click \\\"a b\\\""));
        assert!(e.contains("#x\\\"y"));
    }

    #[test]
    fn expr_without_target_passes_null() {
        let e = build_expr("goto", None);
        assert!(e.ends_with(",null)"));
    }
}
