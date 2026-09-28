/**
 * Translate the golden libs' record() payloads into the current
 * `record-step <do|check> <draft-json>` contract.
 *
 * The libs record semantic events (navigation/action/wait/assert); the
 * recorder only accepts scenario/2 step drafts. Each mapping preserves the
 * recorded intent so `flush` produces a replayable scenario.
 */

export type RecordDraft = [kind: "do" | "check", draft: Record<string, unknown>];

const literal = (v: unknown) => ({ from: "literal", literal: v });
const css = (sel: unknown) => ({
  raw: { kind: "css", value: sel },
  reason: "css selector recorded by golden runner",
});
const textLoc = (text: unknown) => ({
  raw: { kind: "text", value: text },
  reason: "visible text recorded by golden runner",
});
const roleLoc = (role: unknown, name: unknown) => ({ role, name });
const doStep = (intent: string, body: Record<string, unknown>): RecordDraft => [
  "do",
  { intent, ...body },
];
const checkStep = (
  intent: string,
  subject: Record<string, unknown>,
  predicate: string,
  value?: unknown,
): RecordDraft => [
  "check",
  {
    intent,
    claim: {
      subject,
      predicate,
      ...(value === undefined ? {} : { value }),
    },
  },
];

export function toRecordDraft(kind: string, payload: unknown): RecordDraft {
  const p = (payload ?? {}) as Record<string, unknown>;
  const intent = typeof p.intent === "string" ? p.intent : kind;
  switch (kind) {
    case "navigation":
      return doStep(intent, { verb: "goto", value: literal(p.route) });
    case "action": {
      const args = Array.isArray(p.args) ? p.args : [];
      switch (p.method) {
        case "clickSelector":
          return doStep(intent, { verb: "click", on: css(args[0]) });
        case "clickByText":
        case "clickText":
          return doStep(intent, { verb: "click", on: textLoc(args[0]) });
        case "clickRole":
          return doStep(intent, { verb: "click", on: roleLoc(args[0], args[1]) });
        case "clickScopedRole":
          // Scoped locator: the role+name search runs strictly inside the
          // container the scope chain resolves to. `name` may be a string or
          // { i18nKey: "..." } — the key resolves through i18n.json beside
          // the scenario at replay.
          return doStep(intent, {
            verb: "click",
            on: {
              role: args[1],
              name: args[2],
              scope: [css(args[0])],
            },
          });
        case "fillBySelector":
          return doStep(intent, {
            verb: "type",
            on: css(args[0]),
            value: literal(args[1]),
          });
        case "selectBySelector":
          return doStep(intent, {
            verb: "select",
            on: css(args[0]),
            value: literal(Array.isArray(args[1]) ? args[1].join(",") : args[1]),
          });
        case "uploadBySelector":
          return doStep(intent, {
            verb: "upload",
            on: css(args[0]),
            value: literal(args[1]),
          });
        case "pressSelector":
          // `press` accepts `on`: the runner focuses the locator first, then
          // sends the key — the faithful replay of "press KEY on SELECTOR".
          return doStep(intent, {
            verb: "press",
            on: css(args[0]),
            value: literal(args[1]),
          });
        case "pressKey":
          return doStep(intent, { verb: "press", value: literal(args[0]) });
        case "clearBySelector":
          return doStep(intent, { verb: "clear", on: css(args[0]) });
        case "focusBySelector":
          return doStep(intent, { verb: "focus", on: css(args[0]) });
        case "blurBySelector":
          return doStep(intent, { verb: "blur", on: css(args[0]) });
        case "hoverBySelector":
          return doStep(intent, { verb: "hover", on: css(args[0]) });
        case "navigate":
          return doStep(intent, { verb: "goto", value: literal(args[0]) });
        case "dialogAccept":
          return doStep(intent, {
            verb: "dialog",
            params: {
              action: "accept",
              ...(args[0] === undefined ? {} : { text: args[0] }),
            },
          });
        case "dialogDismiss":
          return doStep(intent, { verb: "dialog", params: { action: "dismiss" } });
        case "downloadBySelector":
          return doStep(intent, {
            verb: "download",
            on: css(args[0]),
            value: literal(args[1]),
          });
        case "dblclickBySelector":
          return doStep(intent, { verb: "dblclick", on: css(args[0]) });
        case "holdBySelector":
          // args[0] = css, args[1] = optional hold ms (default 500).
          return doStep(intent, {
            verb: "hold",
            on: css(args[0]),
            ...(args[1] != null ? { params: { ms: args[1] } } : {}),
          });
        case "swipeBySelector":
          // args[0] = css, args[1] = direction, args[2] = optional distance px.
          return doStep(intent, {
            verb: "swipe",
            on: css(args[0]),
            params: {
              direction: args[1],
              ...(args[2] != null ? { distance: args[2] } : {}),
            },
          });
        case "swipePage":
          // args[0] = direction, args[1] = optional distance px — no `on`.
          return doStep(intent, {
            verb: "swipe",
            params: {
              direction: args[0],
              ...(args[1] != null ? { distance: args[1] } : {}),
            },
          });
        case "pinchBySelector":
          // args[0] = css, args[1] = direction ("in"|"out"), args[2] =
          // optional per-finger distance px.
          return doStep(intent, {
            verb: "pinch",
            on: css(args[0]),
            params: {
              direction: args[1],
              ...(args[2] != null ? { distance: args[2] } : {}),
            },
          });
        case "pinchPage":
          // args[0] = direction, args[1] = optional distance px — no `on`.
          return doStep(intent, {
            verb: "pinch",
            params: {
              direction: args[0],
              ...(args[1] != null ? { distance: args[1] } : {}),
            },
          });
        case "rotateBySelector":
          // args[0] = css, args[1] = signed degrees (+ cw), args[2] =
          // optional orbit radius px.
          return doStep(intent, {
            verb: "rotate",
            on: css(args[0]),
            params: {
              degrees: args[1],
              ...(args[2] != null ? { radius: args[2] } : {}),
            },
          });
        case "rotatePage":
          // args[0] = signed degrees, args[1] = optional radius px — no `on`.
          return doStep(intent, {
            verb: "rotate",
            params: {
              degrees: args[0],
              ...(args[1] != null ? { radius: args[1] } : {}),
            },
          });
        case "scrollToBySelector":
          return doStep(intent, { verb: "scrollTo", on: css(args[0]) });
        case "scrollTop":
          // scrollTo with no locator scrolls the page to the top.
          return doStep(intent, { verb: "scrollTo" });
        case "dragBySelector":
          // args[0] = source css, args[1] = target css — drives do/drag.
          return doStep(intent, {
            verb: "drag",
            on: css(args[0]),
            params: { to: css(args[1]) },
          });
        case "checkBySelector":
          return doStep(intent, { verb: "check", on: css(args[0]) });
        case "uncheckBySelector":
          return doStep(intent, { verb: "uncheck", on: css(args[0]) });
        case "setViewport":
          // args[0] = width px, args[1] = height px — do/viewport resizes the
          // live browser so breakpoint-gated content can be asserted.
          return doStep(intent, {
            verb: "viewport",
            params: { width: args[0], height: args[1] },
          });
        case "fileChooserFiles":
          // Arms native file-chooser interception: the NEXT click that would
          // open the OS picker resolves with these files. args[0] may be a
          // single path or an array of paths.
          return doStep(intent, {
            verb: "fileChooser",
            params: { files: Array.isArray(args[0]) ? args[0] : [args[0]] },
          });
        case "readBySelector":
          // args[1] (optional) names the saveAs binding the read text lands in.
          return doStep(intent, {
            verb: "read",
            on: css(args[0]),
            ...(args[1] === undefined ? {} : { saveAs: args[1] }),
          });
        case "goBack":
          return doStep(intent, { verb: "back" });
        case "goForward":
          return doStep(intent, { verb: "forward" });
        case "waitLoadState":
          // args[0] = load state: "load" | "domcontentloaded" | "networkidle"
          return doStep(intent, { verb: "wait", params: { until: args[0] } });
        case "callGqlApi":
          // args = [url, query, variables?, saveAs?] — the response binds to
          // saveAs when given, for later {{steps.<id>}} claims.
          return doStep(intent, {
            verb: "callGql",
            params: {
              url: args[0],
              query: args[1],
              ...(args[2] === undefined ? {} : { variables: args[2] }),
            },
            ...(args[3] === undefined ? {} : { saveAs: args[3] }),
          });
        case "reloadPage":
          return doStep(intent, { verb: "reload" });
        case "tabCommand":
          // args[0] is the full `tab` subcommand tail: "new <url>", "list",
          // "close <ref>", or "<ref>" to switch.
          return doStep(intent, { verb: "tab", value: literal(args[0]) });
        case "seedState":
          // do/state seeding: args[0] = params ({cookies:[...], localStorage:{...}, ...})
          return doStep(intent, { verb: "state", params: args[0] });
        case "clickNthOption":
          // args[0] = scoped listbox css, args[1] = 1-based option index
          return doStep(intent, {
            verb: "click",
            on: css(`${args[0]} [role="option"]:nth-child(${args[1]})`),
          });
        case "setViewport":
          // args[0] = width px, args[1] = height px — do/viewport resizes the
          // live browser so breakpoint-gated content can be asserted.
          return doStep(intent, {
            verb: "viewport",
            params: { width: args[0], height: args[1] },
          });
        default:
          throw new Error(`record-step translate: unknown action method ${String(p.method)}`);
      }
    }
    case "wait": {
      const c = (p.condition ?? {}) as Record<string, unknown>;
      switch (c.kind) {
        case "duration":
          return doStep(intent, { verb: "wait", params: { ms: c.ms } });
        case "selector":
          return checkStep(intent, { element: css(c.selector) }, "isVisible");
        case "selectorAbsent":
          return checkStep(intent, { element: css(c.selector) }, "notExists");
        case "scopedRole":
          return checkStep(
            intent,
            {
              element: {
                role: c.role,
                name: c.name,
                scope: [css(c.selector)],
              },
            },
            "isVisible",
          );
        case "selectorText":
          return checkStep(
            intent,
            { element: css(c.selector), attribute: "text" },
            "contains",
            c.text,
          );
        case "text":
          return checkStep(intent, { element: textLoc(c.text) }, "isVisible");
        case "url":
          return checkStep(intent, { url: true }, "contains", c.pattern);
        case "loadState":
          // c.state = "load" | "domcontentloaded" | "networkidle"
          return doStep(intent, { verb: "wait", params: { until: c.state } });
        default:
          throw new Error(`record-step translate: unknown wait condition ${String(c.kind)}`);
      }
    }
    case "assert": {
      const args = Array.isArray(p.args) ? p.args : [];
      switch (p.kind) {
        case "url":
          // args[1] optionally overrides the predicate (equals/matches/
          // startsWith/endsWith); default stays contains.
          return checkStep(intent, { url: true }, args[1] ?? "contains", args[0]);
        case "present":
          return checkStep(intent, { element: roleLoc(args[0], args[1]) }, "isVisible");
        case "absent":
          return checkStep(intent, { element: roleLoc(args[0], args[1]) }, "notExists");
        case "elementAbsent":
          return checkStep(intent, { element: css(args[0]) }, "notExists");
        case "elementPresent":
          return checkStep(intent, { element: css(args[0]) }, "isVisible");
        case "elementText":
          return checkStep(
            intent,
            { element: css(args[0]), attribute: "text" },
            "equals",
            args[1],
          );
        case "elementAttribute":
          return checkStep(
            intent,
            { element: css(args[0]), attribute: args[1] },
            args[2] ?? "equals",
            args[3],
          );
        case "fileExists":
          return checkStep(intent, { file: args[0] }, "exists");
        case "fileAbsent":
          return checkStep(intent, { file: args[0] }, "notExists");
        case "fileSizeGt":
          return checkStep(intent, { file: args[0] }, "gt", args[1]);
        case "fileSizeLt":
          return checkStep(intent, { file: args[0] }, "lt", args[1]);
        case "fileName":
          return checkStep(intent, { file: args[0] }, "equals", args[1]);
        case "fileString":
          // args[0] = scenario-relative path — any string predicate against
          // the file name (equals/startsWith/endsWith/matches/...).
          return checkStep(intent, { file: args[0] }, args[1], args[2]);
        case "fileContent":
          // args[0] = scenario-relative path, args[1] = needle — string
          // predicates run against the file's UTF-8 text.
          return checkStep(
            intent,
            { file: args[0], attribute: "content" },
            "contains",
            args[1],
          );
        case "dialogOpen":
          return checkStep(intent, { dialog: true }, "exists");
        case "dialogClosed":
          return checkStep(intent, { dialog: true }, "notExists");
        case "dialogText":
          return checkStep(intent, { dialog: true }, "contains", args[0]);
        case "stepTiming":
          // args: [stepId, predicate, ms]
          return checkStep(
            intent,
            { timing: args[0] },
            args[1] ?? "lt",
            args[2],
          );
        case "networkFired":
          // args[0] = matcher {urlMatches?, operationName?, method?};
          // args[1] === false flips to "must not have fired".
          return checkStep(
            intent,
            { network: args[0], ofKind: "fired" },
            args[1] === false ? "notExists" : "exists",
          );
        case "networkStatus":
          // args: [matcher, predicate, value] — predicate on the latest
          // matching request's HTTP status.
          return checkStep(
            intent,
            { network: args[0], ofKind: "status" },
            args[1] ?? "equals",
            args[2],
          );
        case "networkJson":
          // args: [matcher, path, predicate, value?] — JSON path into the
          // latest matching request's response body.
          return checkStep(
            intent,
            { network: args[0], ofKind: "responseJsonPath", path: args[1] },
            args[2],
            args[3],
          );
        case "elementChecked":
          // `checked` reads the live IDL property, not the attribute — so this
          // sees the post-interaction state. args[1] flips to expect unchecked.
          return checkStep(
            intent,
            { element: css(args[0]), attribute: "checked" },
            "equals",
            args[1] === false ? "false" : "true",
          );
        case "elementFocused":
          return checkStep(
            intent,
            { element: css(args[0]), attribute: "focused" },
            "equals",
            "true",
          );
        case "consoleMessage":
          // args[0] = matcher: true | {type?, text?} — defaults to all
          // messages; args[1] = predicate ("exists" default, "notExists",
          // numeric/text predicates); args[2] = optional value.
          return checkStep(
            intent,
            { console: args[0] ?? true },
            args[1] ?? "exists",
            args[2],
          );
        case "pageError":
          // args[0] = matcher: true | {text?, url?} — uncaught exceptions
          // (the `errors` channel), NOT console.* calls. args[1] predicate
          // ("exists" default / "notExists" / numeric / text), args[2] value.
          return checkStep(
            intent,
            { pageError: args[0] ?? true },
            args[1] ?? "exists",
            args[2],
          );
        case "a11yViolations":
          // args[0] = matcher: true | {impact?, rule?, within?, incomplete?};
          // args[1] = predicate — "notExists" (no violations) is the usual
          // default; numeric predicates compare the finding count.
          return checkStep(
            intent,
            { a11y: args[0] ?? true },
            args[1] ?? "notExists",
            args[2],
          );
        case "cookiePresent":
          // args[0] = cookie name; args[1] === false flips to expecting it
          // absent (e.g. after logout).
          return checkStep(
            intent,
            { cookie: args[0] },
            args[1] === false ? "notExists" : "exists",
          );
        case "storagePresent":
          // args[0] = key or {key, scope:"local"|"session"}; args[1] ===
          // false flips to expecting the key absent.
          return checkStep(
            intent,
            { storage: args[0] },
            args[1] === false ? "notExists" : "exists",
          );
        case "consoleMessage":
          // args[0] = matcher: true | {type?, text?} — defaults to all
          // messages; args[1] = predicate ("exists" default, "notExists",
          // numeric/text predicates); args[2] = optional value.
          return checkStep(
            intent,
            { console: args[0] ?? true },
            args[1] ?? "exists",
            args[2],
          );
        case "pageError":
          // args[0] = matcher: true | {text?, url?} — uncaught exceptions
          // (the `errors` channel), NOT console.* calls. args[1] predicate
          // ("exists" default / "notExists" / numeric / text), args[2] value.
          return checkStep(
            intent,
            { pageError: args[0] ?? true },
            args[1] ?? "exists",
            args[2],
          );
        case "a11yViolations":
          // args[0] = matcher: true | {impact?, rule?, within?, incomplete?};
          // args[1] = predicate — "notExists" (no violations) is the usual
          // default; numeric predicates compare the finding count.
          return checkStep(
            intent,
            { a11y: args[0] ?? true },
            args[1] ?? "notExists",
            args[2],
          );
        default:
          throw new Error(`record-step translate: unknown assert kind ${String(p.kind)}`);
      }
    }
    default:
      throw new Error(`record-step translate: unknown record kind ${kind}`);
  }
}
