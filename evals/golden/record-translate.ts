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
        case "scrollToBySelector":
          return doStep(intent, { verb: "scrollTo", on: css(args[0]) });
        case "reloadPage":
          return doStep(intent, { verb: "reload" });
        case "tabCommand":
          // args[0] is the full `tab` subcommand tail: "new <url>", "list",
          // "close <ref>", or "<ref>" to switch.
          return doStep(intent, { verb: "tab", value: literal(args[0]) });
        case "clickNthOption":
          // args[0] = scoped listbox css, args[1] = 1-based option index
          return doStep(intent, {
            verb: "click",
            on: css(`${args[0]} [role="option"]:nth-child(${args[1]})`),
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
        default:
          throw new Error(`record-step translate: unknown wait condition ${String(c.kind)}`);
      }
    }
    case "assert": {
      const args = Array.isArray(p.args) ? p.args : [];
      switch (p.kind) {
        case "url":
          return checkStep(intent, { url: true }, "contains", args[0]);
        case "present":
          return checkStep(intent, { element: roleLoc(args[0], args[1]) }, "isVisible");
        case "absent":
          return checkStep(intent, { element: roleLoc(args[0], args[1]) }, "notExists");
        case "elementAbsent":
          return checkStep(intent, { element: css(args[0]) }, "notExists");
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
        case "fileName":
          return checkStep(intent, { file: args[0] }, "equals", args[1]);
        case "dialogOpen":
          return checkStep(intent, { dialog: true }, "exists");
        case "dialogClosed":
          return checkStep(intent, { dialog: true }, "notExists");
        case "dialogText":
          return checkStep(intent, { dialog: true }, "contains", args[0]);
        default:
          throw new Error(`record-step translate: unknown assert kind ${String(p.kind)}`);
      }
    }
    default:
      throw new Error(`record-step translate: unknown record kind ${kind}`);
  }
}
