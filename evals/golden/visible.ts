// Shared live-drive snippets for golden libs.
//
// Replay's css path resolves the first *visible* match; the helpers below
// make the same pick in-page so a hidden duplicate twin can't win the
// live drive while replay picks a different node (storefronts render a
// hidden `btn-cart` mirror, practice apps ship zero-size anchors).

/** First visible match for a css selector, evaluated in-page. */
export function pickVisible(selector: string): string {
  return `[...document.querySelectorAll(${JSON.stringify(selector)})].find((el) => { const r = el.getBoundingClientRect(); const s = getComputedStyle(el); return r.width > 0 && r.height > 0 && s.display !== "none" && s.visibility !== "hidden"; })`;
}

/**
 * Full eval expression: click the first visible match (falling back to the
 * first match when nothing is visible — a hidden-but-wired twin still
 * carries the delegated handler). Dispatches mousedown/mouseup first so
 * listeners bound to the real sequence see it.
 */
export function clickVisibleEval(selector: string): string {
  return `(() => { const el = ${pickVisible(selector)} ?? document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error("selector not found: " + ${JSON.stringify(selector)}); el.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true, view: window })); el.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true, view: window })); el.click(); return true; })()`;
}

/**
 * Sticky fill via the native value setter + input/change events.
 *
 * `agent-browser fill` reports success on React-controlled and constrained
 * inputs (`type=date`, `type=number`, …) while leaving `el.value` empty —
 * same gap replay already repairs in `verbs.rs::fill_or_act_via_selector`.
 * Assigning `el.value = …` directly is also a no-op for controlled React
 * inputs; the prototype setter + InputEvent is what sticks.
 */
export function stickyFillEval(selector: string, value: string): string {
  return `(() => new Promise((resolve, reject) => {
  const els = Array.from(document.querySelectorAll(${JSON.stringify(selector)}));
  const el = els.find((n) => n.getClientRects().length > 0) || els[0];
  if (!el) return reject(new Error("selector not found: " + ${JSON.stringify(selector)}));
  const want = ${JSON.stringify(value)};
  const apply = () => {
    el.focus();
    const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(proto, "value")?.set;
    if (setter) setter.call(el, want); else el.value = want;
    el.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: want }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  };
  apply();
  let n = 0;
  const tick = () => {
    if (el.value === want) return resolve(true);
    if (++n > 20) return reject(new Error("fill did not stick on " + ${JSON.stringify(selector)} + ": got " + JSON.stringify(el.value)));
    apply();
    setTimeout(tick, 100);
  };
  setTimeout(tick, 50);
}))()`;
}

/** @deprecated alias — prefer stickyFillEval (native setter). */
export function fillVisibleEval(selector: string, value: string): string {
  return stickyFillEval(selector, value);
}

/** eval: select option(s) on the first visible match (value or label). */
export function selectVisibleEval(selector: string, values: string[]): string {
  return `(() => { const el = ${pickVisible(selector)} ?? document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error("selector not found: " + ${JSON.stringify(selector)}); const want = ${JSON.stringify(values)}; for (const opt of el.options) opt.selected = want.includes(opt.value) || want.includes(opt.text); el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true })); return true; })()`;
}

/** eval: check a checkbox/radio on the first visible match. */
export function checkVisibleEval(selector: string): string {
  return `(() => { const el = ${pickVisible(selector)} ?? document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error("selector not found: " + ${JSON.stringify(selector)}); if (!el.checked) { el.checked = true; el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true })); } return true; })()`;
}

/** eval: uncheck a checkbox/radio on the first visible match. */
export function uncheckVisibleEval(selector: string): string {
  return `(() => { const el = ${pickVisible(selector)} ?? document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error("selector not found: " + ${JSON.stringify(selector)}); if (el.checked) { el.checked = false; el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true })); } return true; })()`;
}

/** eval: hover the first visible match (scroll + pointer chain). */
export function hoverVisibleEval(selector: string): string {
  return `(() => { const el = ${pickVisible(selector)} ?? document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error("selector not found: " + ${JSON.stringify(selector)}); try { el.scrollIntoView({ block: "center", inline: "nearest" }); } catch (e) {} const o = { bubbles: true, cancelable: true, view: window }; el.dispatchEvent(new PointerEvent("pointerover", o)); el.dispatchEvent(new MouseEvent("mouseover", o)); el.dispatchEvent(new MouseEvent("mousemove", o)); return true; })()`;
}

const UNHITTABLE = /covered|unhittable|not clickable|not visible|zero.size|no element|not found|timeout/i;

/**
 * Trusted `agent-browser <verb>` first; when the daemon refuses because
 * the first match is unhittable (covered / zero-size / below fold), fall
 * back to the visibility-first eval instead of failing the golden.
 */
export async function trustedOrVisible(
  ctx: { agentBrowser: string; session: string },
  run: (name: string, command: string[]) => Promise<string>,
  name: string,
  command: string[],
  fallbackEval: string,
): Promise<void> {
  try {
    await run(name, command);
  } catch (e) {
    const msg = String(e);
    if (!UNHITTABLE.test(msg)) {
      throw e;
    }
    await run(`${name} (visible fallback)`, [
      ctx.agentBrowser,
      "--session",
      ctx.session,
      "eval",
      fallbackEval,
    ]);
  }
}

/**
 * Live-drive a fill that actually sticks on React / constrained inputs.
 * Prefer the sticky native-setter eval; `agent-browser fill` alone is a
 * silent no-op on `type=date` and many controlled fields.
 */
export async function fillSticky(
  ctx: { agentBrowser: string; session: string },
  run: (name: string, command: string[]) => Promise<string>,
  selector: string,
  value: string,
): Promise<void> {
  await run(`fill ${selector}`, [
    ctx.agentBrowser,
    "--session",
    ctx.session,
    "eval",
    stickyFillEval(selector, value),
  ]);
}

/**
 * Trusted `agent-browser click` first; when the daemon refuses because the
 * first match is unhittable (covered / zero-size / below fold), fall back
 * to the visibility-first eval click instead of failing the golden.
 */
export async function clickTrustedOrVisible(
  ctx: { agentBrowser: string; session: string },
  run: (name: string, command: string[]) => Promise<string>,
  selector: string,
): Promise<void> {
  await trustedOrVisible(
    ctx,
    run,
    `click ${selector}`,
    [ctx.agentBrowser, "--session", ctx.session, "click", selector],
    clickVisibleEval(selector),
  );
}
