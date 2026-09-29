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
 * Trusted `agent-browser click` first; when the daemon refuses because the
 * first match is unhittable (covered / zero-size / below fold), fall back
 * to the visibility-first eval click instead of failing the golden.
 */
export async function clickTrustedOrVisible(
  ctx: { agentBrowser: string; session: string },
  run: (name: string, command: string[]) => Promise<string>,
  selector: string,
): Promise<void> {
  try {
    await run(`click ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "click", selector]);
  } catch (e) {
    const msg = String(e);
    if (!/covered|unhittable|not clickable|not visible|zero.size|no element|not found|timeout/i.test(msg)) {
      throw e;
    }
    await run(`click ${selector} (visible fallback)`, [
      ctx.agentBrowser,
      "--session",
      ctx.session,
      "eval",
      clickVisibleEval(selector),
    ]);
  }
}
