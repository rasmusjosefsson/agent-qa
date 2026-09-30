import { runAuthoredGolden } from "./edge-pages-lib";

// demoqa serves several ad/analytics embeds — navigations can take well
// past the 10s default per-verb cap.
process.env.AGENT_QA_AGENT_BROWSER_TIMEOUT_MS ||= "60000";

// demoqa widgets III — the non-drag widgets: react-select typeahead,
// a native range-input slider, tabs, react-bootstrap accordion, and a
// tooltip via the hover verb.
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
  "demoqa-widgets-tc02",
  "demoqa: auto-complete + slider + tabs + accordion + tooltip",
  {
    steps: [
      step("s0", "auto complete page", {
        verb: "goto",
        value: lit("https://demoqa.com/auto-complete"),
      }),
      step("s1", "type 'whi' into the single-value picker", {
        verb: "type",
        on: css("#autoCompleteSingleInput"),
        value: lit("whi"),
      }),
      // "whi" matches only "White" — the lone suggestion resolves css-unique.
      step("s2", "pick the suggestion", { verb: "click", on: css(".auto-complete__option") }),
      check(
        "s3",
        "selection landed in the input",
        ".auto-complete__single-value",
        "text",
        "equals",
        "White",
      ),
      step("s4", "slider page", {
        verb: "goto",
        value: lit("https://demoqa.com/slider"),
      }),
      step("s5", "focus the slider handle", {
        verb: "focus",
        on: css("#slider"),
      }),
      step("s6", "ArrowRight once", { verb: "press", value: lit("ArrowRight") }),
      step("s7", "ArrowRight twice", { verb: "press", value: lit("ArrowRight") }),
      check(
        "s8",
        "slider moved 25 -> 27",
        "#sliderValue",
        "value",
        "equals",
        "27",
      ),
      step("s9", "tabs page", {
        verb: "goto",
        value: lit("https://demoqa.com/tabs"),
      }),
      step("s10", "open the Origin tab", { verb: "click", on: css("#demo-tab-origin") }),
      check(
        "s11",
        "origin pane is showing its text",
        "#demo-tabpane-origin",
        "text",
        "contains",
        "Contrary",
      ),
      step("s12", "accordion page", {
        verb: "goto",
        value: lit("https://demoqa.com/accordian"),
      }),
      step("s13", "expand section 2", {
        verb: "click",
        on: css(".accordion-item:nth-child(2) .accordion-button"),
      }),
      check(
        "s14",
        "section 2 content is displayed",
        ".accordion-item:nth-child(2) .accordion-collapse",
        "class",
        "contains",
        "show",
      ),
      step("s15", "tool tips page", {
        verb: "goto",
        value: lit("https://demoqa.com/tool-tips"),
      }),
      step("s16", "hover the button", { verb: "hover", on: css("#toolTipButton") }),
      check(
        "s17",
        "bootstrap tooltip rendered",
        ".tooltip-inner",
        "text",
        "contains",
        "hovered over the Button",
      ),
    ],
  },
);
