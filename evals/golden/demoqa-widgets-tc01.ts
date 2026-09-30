import { runAuthoredGolden } from "./edge-pages-lib";

// demoqa serves several ad/analytics embeds — navigations can take well
// past the 10s default per-verb cap, and the built-in retry ladder can
// eat 15s of that budget before the winning attempt starts.
process.env.AGENT_QA_AGENT_BROWSER_TIMEOUT_MS ||= "60000";

// demoqa widgets II — drag widgets driven by three different libraries:
// sortable is react-dnd (HTML5 native dnd), droppable tracks raw pointer
// events, resizable is react-resizable. Exercises the drag verb's
// trusted-mouse gesture path — synthetic DOM events move none of these.
const css = (value: string) => ({
  raw: { kind: "css", value },
  reason: "authored demoqa-widgets golden",
});
const lit = (literal: unknown) => ({ from: "literal", literal });
const step = (id: string, intent: string, extra: Record<string, unknown>) => ({
  id,
  intent,
  kind: "do",
  ...extra,
});
const check = (
  id: string,
  intent: string,
  selector: string,
  attribute: string | null,
  predicate: string,
  value?: unknown,
) => ({
  id,
  intent,
  kind: "check",
  claim: {
    subject: {
      element: css(selector),
      ...(attribute ? { attribute } : {}),
    },
    predicate,
    ...(value === undefined ? {} : { value }),
  },
});

await runAuthoredGolden(
  "demoqa-widgets-tc01",
  "demoqa: sortable + droppable + resizable via mouse-drag gestures",
  {
    steps: [
      step("s0", "sortable list page", {
        verb: "goto",
        value: lit("https://demoqa.com/sortable"),
      }),
      check(
        "s1",
        "precondition: item 4 is Four",
        ".vertical-list-container .list-group-item:nth-child(4)",
        "text",
        "equals",
        "Four",
      ),
      step("s2", "drag Two onto Four", {
        verb: "drag",
        on: css(".vertical-list-container .list-group-item:nth-child(2)"),
        params: {
          to: css(".vertical-list-container .list-group-item:nth-child(4)"),
        },
      }),
      check(
        "s3",
        "order changed: Two moved to position 3",
        ".vertical-list-container .list-group-item:nth-child(3)",
        "text",
        "equals",
        "Two",
      ),
      step("s4", "droppable page", {
        verb: "goto",
        value: lit("https://demoqa.com/droppable"),
      }),
      step("s5", "drag the box into the drop target", {
        verb: "drag",
        on: css("#draggable"),
        params: { to: css("#droppable") },
      }),
      check(
        "s6",
        "drop target reports the drop",
        "#droppable p",
        "text",
        "equals",
        "Dropped!",
      ),
      step("s7", "resizable page", {
        verb: "goto",
        value: lit("https://demoqa.com/resizable"),
      }),
      step("s8", "drag the resize handle to the container center", {
        verb: "drag",
        on: css("#resizableBoxWithRestriction .react-resizable-handle-se"),
        params: { to: css(".constraint-area") },
      }),
      // The box starts at 200x200; dragging its SE handle to the 500x300
      // container's center lands it at ~250x150 — a height drop below
      // 200px proves the mouse-drag resized it.
      check(
        "s9",
        "box resized: computed height dropped under 200px",
        "#resizableBoxWithRestriction",
        "style:height",
        "matches",
        "^1\\d\\d(\\.\\d+)?px$",
      ),
    ],
  },
);
