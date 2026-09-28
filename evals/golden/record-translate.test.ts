/**
 * Record ↔ replay parity gate.
 *
 * The golden libs record a semantic vocabulary (navigation/action/wait/assert
 * payloads); `toRecordDraft` translates each into a scenario/2 step draft for
 * `record-step`. These tests pin two directions:
 *
 *   1. Every verb a draft carries must exist in the shipped schema enum —
 *      a draft naming a verb replay doesn't know fails `scenario check`.
 *   2. Every shipped verb must be either reachable through a translate
 *      mapping or deliberately listed in NOT_RECORDABLE — adding a verb
 *      without a record path fails here until it's triaged.
 *
 * Run: `bun test evals/golden/record-translate.test.ts`
 */

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { toRecordDraft } from "./record-translate";

const schema = JSON.parse(
  readFileSync(new URL("../../schema/scenario-schema.json", import.meta.url), "utf8"),
);

// The StepDo verb enum is the single source of truth for shipped verbs.
function shippedVerbs(): string[] {
  let found: string[] | null = null;
  const walk = (node: unknown): void => {
    if (found || node === null || typeof node !== "object") return;
    if (Array.isArray(node)) {
      for (const v of node) walk(v);
      return;
    }
    const obj = node as Record<string, unknown>;
    if (Array.isArray(obj.enum) && obj.enum.includes("click")) {
      found = obj.enum as string[];
      return;
    }
    for (const v of Object.values(obj)) walk(v);
  };
  walk(schema);
  if (!found) throw new Error("verb enum not found in scenario schema");
  return found;
}

// Verbs that exist for hand-written scenarios / tooling but are not reachable
// through the golden record vocabulary — with the reason. A verb missing from
// BOTH the emitted set and this list fails the gate.
const NOT_RECORDABLE: Record<string, string> = {
  loop: "structural — scenarios iterate via the loop step, not a recorded action",
  group: "structural — groupings are authored, not recorded",
  useTemplate: "structural — template application is authored",
  drag: "recorded natively by the workbench live pane (endpoint pick + emit), not via agent-browser actions",
  mock: "authored — stubbing decisions belong in the scenario, not the recording",
  unmock: "authored — removes authored stubs; never emitted by capture",
  frame: "authored — frame context is a scenario decision; capture records flat actions",
  state: "authored — seeds cookies/storage before navigation; never emitted by capture",
  emulate: "authored — emulation config is a scenario decision, not a recorded action",
};

const action = (method: string, args: unknown[] = [], intent = "t") =>
  toRecordDraft("action", { method, args, intent });
const wait = (condition: Record<string, unknown>) =>
  toRecordDraft("wait", { condition });
const assertK = (kind: string, args: unknown[] = []) =>
  toRecordDraft("assert", { kind, args });

describe("toRecordDraft — emitted verbs are shipped", () => {
  const payloads: Array<[string, unknown]> = [
    ["navigation", { route: "https://example.com" }],
    ["action", { method: "clickSelector", args: ["#a"] }],
    ["action", { method: "clickText", args: ["Save"] }],
    ["action", { method: "clickRole", args: ["button", "Save"] }],
    ["action", { method: "clickScopedRole", args: ["#c", "checkbox", "All"] }],
    ["action", { method: "clickNthOption", args: ["#list", 2] }],
    ["action", { method: "fillBySelector", args: ["#i", "x"] }],
    ["action", { method: "selectBySelector", args: ["#s", "v"] }],
    ["action", { method: "uploadBySelector", args: ["#u", "f.txt"] }],
    ["action", { method: "pressSelector", args: ["#i", "Enter"] }],
    ["action", { method: "pressKey", args: ["Escape"] }],
    ["action", { method: "clearBySelector", args: ["#i"] }],
    ["action", { method: "focusBySelector", args: ["#i"] }],
    ["action", { method: "blurBySelector", args: ["#i"] }],
    ["action", { method: "hoverBySelector", args: ["#h"] }],
    ["action", { method: "navigate", args: ["https://example.com/x"] }],
    ["action", { method: "dialogAccept", args: ["ok"] }],
    ["action", { method: "dialogDismiss", args: [] }],
    ["action", { method: "downloadBySelector", args: ["#d", "out/x"] }],
    ["action", { method: "dblclickBySelector", args: ["#d"] }],
    ["action", { method: "scrollToBySelector", args: ["#s"] }],
    ["action", { method: "scrollTop", args: [] }],
    ["action", { method: "checkBySelector", args: ["#c"] }],
    ["action", { method: "uncheckBySelector", args: ["#c"] }],
    ["action", { method: "setViewport", args: [375, 812] }],
    ["action", { method: "fileChooserFiles", args: [["a.txt", "b.txt"]] }],
    ["action", { method: "readBySelector", args: ["#t", "saved"] }],
    ["action", { method: "goBack", args: [] }],
    ["action", { method: "goForward", args: [] }],
    ["action", { method: "waitLoadState", args: ["load"] }],
    ["action", { method: "callGqlApi", args: ["/gql", "{ q }", {}, "resp"] }],
    ["action", { method: "reloadPage", args: [] }],
    ["action", { method: "tabCommand", args: ["new https://example.com"] }],
    ["wait", { condition: { kind: "duration", ms: 100 } }],
    ["wait", { condition: { kind: "selector", selector: "#x" } }],
    ["wait", { condition: { kind: "selectorAbsent", selector: "#x" } }],
    [
      "wait",
      { condition: { kind: "scopedRole", selector: "#c", role: "option", name: "A" } },
    ],
    ["wait", { condition: { kind: "selectorText", selector: "#x", text: "hi" } }],
    ["wait", { condition: { kind: "text", text: "hi" } }],
    ["wait", { condition: { kind: "url", pattern: "/home" } }],
    ["wait", { condition: { kind: "loadState", state: "networkidle" } }],
    ["assert", { kind: "url", args: ["/home"] }],
    ["assert", { kind: "url", args: ["https://example.com/", "equals"] }],
    ["assert", { kind: "present", args: ["button", "Save"] }],
    ["assert", { kind: "absent", args: ["button", "Gone"] }],
    ["assert", { kind: "elementAbsent", args: ["#x"] }],
    ["assert", { kind: "elementText", args: ["#x", "hi"] }],
    ["assert", { kind: "elementAttribute", args: ["#x", "href", "equals", "/a"] }],
    ["assert", { kind: "elementChecked", args: ["#c"] }],
    ["assert", { kind: "elementChecked", args: ["#c", false] }],
    ["assert", { kind: "elementFocused", args: ["#i"] }],
    ["assert", { kind: "fileExists", args: ["downloads/x"] }],
    ["assert", { kind: "fileAbsent", args: ["downloads/x"] }],
    ["assert", { kind: "fileSizeGt", args: ["downloads/x", 10] }],
    ["assert", { kind: "fileSizeLt", args: ["downloads/x", 1000] }],
    ["assert", { kind: "fileName", args: ["downloads/x", "x.pdf"] }],
    ["assert", { kind: "dialogOpen", args: [] }],
    ["assert", { kind: "dialogClosed", args: [] }],
    ["assert", { kind: "dialogText", args: ["Sure?"] }],
    ["assert", { kind: "networkFired", args: [{ urlMatches: "/api/" }] }],
    ["assert", { kind: "networkFired", args: [{ urlMatches: "/api/" }, false] }],
    ["assert", { kind: "networkStatus", args: [{ method: "GET" }, "equals", "200"] }],
    ["assert", { kind: "networkJson", args: [{ operationName: "GetUser" }, "$.data.id", "exists"] }],
  ];

  const verbs = shippedVerbs();
  for (const [kind, payload] of payloads) {
    test(`${kind} ${JSON.stringify(payload).slice(0, 60)}`, () => {
      const [draftKind, draft] = toRecordDraft(kind, payload);
      expect(["do", "check"]).toContain(draftKind);
      if (draftKind === "do") {
        const verb = (draft as { verb?: string }).verb;
        expect(
          verbs,
          `verb '${verb}' emitted but not in scenario schema enum`,
        ).toContain(verb);
      } else {
        expect(draft).toHaveProperty("claim");
      }
    });
  }
});

describe("toRecordDraft — every shipped verb is reachable or triaged", () => {
  // One probe payload per shipped do-verb: emits the verb through the mapping
  // and asserts it lands with the right shape.
  const probes: Array<[verb: string, draft: [string, unknown], check?: (d: Record<string, unknown>) => void]> = [
    ["goto", ["navigation", { route: "https://example.com" }]],
    ["reload", ["action", { method: "reloadPage" }]],
    ["back", ["action", { method: "goBack" }]],
    ["forward", ["action", { method: "goForward" }]],
    ["click", ["action", { method: "clickSelector", args: ["#a"] }]],
    ["type", ["action", { method: "fillBySelector", args: ["#i", "v"] }]],
    ["clear", ["action", { method: "clearBySelector", args: ["#i"] }]],
    ["press", ["action", { method: "pressKey", args: ["Enter"] }]],
    ["hover", ["action", { method: "hoverBySelector", args: ["#h"] }]],
    ["select", ["action", { method: "selectBySelector", args: ["#s", "v"] }]],
    ["check", ["action", { method: "checkBySelector", args: ["#c"] }]],
    ["uncheck", ["action", { method: "uncheckBySelector", args: ["#c"] }]],
    ["upload", ["action", { method: "uploadBySelector", args: ["#u", "f"] }]],
    [
      "scrollTo",
      ["action", { method: "scrollToBySelector", args: ["#s"] }],
    ],
    ["focus", ["action", { method: "focusBySelector", args: ["#i"] }]],
    ["blur", ["action", { method: "blurBySelector", args: ["#i"] }]],
    ["read", ["action", { method: "readBySelector", args: ["#t"] }]],
    [
      "callGql",
      ["action", { method: "callGqlApi", args: ["/gql", "{ q }"] }],
    ],
    ["wait", ["wait", { condition: { kind: "duration", ms: 50 } }]],
    ["dialog", ["action", { method: "dialogAccept", args: [] }]],
    [
      "download",
      ["action", { method: "downloadBySelector", args: ["#d", "out"] }],
    ],
    ["dblclick", ["action", { method: "dblclickBySelector", args: ["#d"] }]],
    ["tab", ["action", { method: "tabCommand", args: ["list"] }]],
    ["viewport", ["action", { method: "setViewport", args: [375, 812] }]],
    [
      "fileChooser",
      ["action", { method: "fileChooserFiles", args: [["a.txt"]] }],
    ],
  ];

  const emitted = new Set<string>();
  for (const [verb, [kind, payload]] of probes) {
    test(`verb '${verb}' has a record path`, () => {
      const [draftKind, draft] = toRecordDraft(kind, payload);
      expect(draftKind).toBe("do");
      expect((draft as { verb: string }).verb).toBe(verb);
      emitted.add(verb);
    });
  }

  test("no shipped verb is unreachable AND untriaged", () => {
    const emittedVerbs = new Set(probes.map(([v]) => v));
    const untriaged = shippedVerbs().filter(
      (v) => !emittedVerbs.has(v) && !(v in NOT_RECORDABLE),
    );
    expect(
      untriaged,
      "new verbs must get a translate mapping or a NOT_RECORDABLE entry",
    ).toEqual([]);
  });
});

describe("toRecordDraft — new mappings land the right fields", () => {
  test("setViewport carries params {width,height}", () => {
    const [, d] = action("setViewport", [375, 812]);
    expect(d).toMatchObject({ verb: "viewport", params: { width: 375, height: 812 } });
  });
  test("fileChooserFiles normalises a single path to an array", () => {
    const [, d] = action("fileChooserFiles", ["one.txt"]);
    expect(d).toMatchObject({ verb: "fileChooser", params: { files: ["one.txt"] } });
  });
  test("readBySelector threads saveAs", () => {
    const [, d] = action("readBySelector", ["#t", "title"]);
    expect(d).toMatchObject({ verb: "read", saveAs: "title" });
  });
  test("callGqlApi threads params and saveAs", () => {
    const [, d] = action("callGqlApi", ["/gql", "{ q }", { a: 1 }, "resp"]);
    expect(d).toMatchObject({
      verb: "callGql",
      params: { url: "/gql", query: "{ q }", variables: { a: 1 } },
      saveAs: "resp",
    });
  });
  test("elementChecked expects 'true', flips with args[1]=false", () => {
    const [, yes] = assertK("elementChecked", ["#c"]);
    expect(yes).toMatchObject({
      claim: { subject: { attribute: "checked" }, predicate: "equals", value: "true" },
    });
    const [, no] = assertK("elementChecked", ["#c", false]);
    expect(no).toMatchObject({ claim: { value: "false" } });
  });
  test("url assert keeps contains default, honours predicate override", () => {
    const [, d1] = assertK("url", ["/home"]);
    expect(d1).toMatchObject({ claim: { predicate: "contains", value: "/home" } });
    const [, d2] = assertK("url", ["example.com", "equals"]);
    expect(d2).toMatchObject({ claim: { predicate: "equals" } });
  });
  test("loadState wait maps to wait params.until", () => {
    const [, d] = wait({ kind: "loadState", state: "networkidle" });
    expect(d).toMatchObject({ verb: "wait", params: { until: "networkidle" } });
  });
  test("unknown methods still throw", () => {
    expect(() => action("nopeNever")).toThrow(/unknown action method/);
    expect(() => wait({ kind: "mystery" })).toThrow(/unknown wait condition/);
    expect(() => assertK("mystery")).toThrow(/unknown assert kind/);
  });
});
