import { runAuthoredGolden } from "./edge-pages-lib";

// demoqa serves several ad/analytics embeds — navigations can take well
// past the 10s default per-verb cap.
process.env.AGENT_QA_AGENT_BROWSER_TIMEOUT_MS ||= "30000";

// demoqa widgets — React date-picker dropdown, bootstrap modal dialog,
// and the progress-bar's fill→reset cycle on a fifth live site.
const css = (value: string) => ({
  raw: { kind: "css", value },
  reason: "authored demoqa golden",
});
const lit = (literal: unknown) => ({ from: "literal", literal });
const step = (id: string, intent: string, extra: Record<string, unknown>) => ({
  id,
  intent,
  kind: "do",
  ...extra,
});

await runAuthoredGolden(
  "demoqa-tc01",
  "demoqa: date-picker + modal-dialogs + progress-bar",
  {
    steps: [
      step("s0", "date picker page", {
        verb: "goto",
        value: lit("https://demoqa.com/date-picker"),
      }),
      step("s1", "open the date popover", { verb: "click", on: css("#datePickerMonthYearInput") }),
      {
        id: "s2",
        intent: "calendar grid rendered",
        kind: "check",
        claim: { subject: { element: css(".react-datepicker__day--015") }, predicate: "exists" },
      },
      step("s3", "pick the 15th", { verb: "click", on: css(".react-datepicker__day--015") }),
      {
        id: "s4",
        intent: "input shows a *-15-* date",
        kind: "check",
        claim: {
          subject: { element: css("#datePickerMonthYearInput"), attribute: "value" },
          predicate: "contains",
          value: "15",
        },
      },
      step("s5", "modal dialogs page", {
        verb: "goto",
        value: lit("https://demoqa.com/modal-dialogs"),
      }),
      step("s6", "open the small modal", { verb: "click", on: css("#showSmallModal") }),
      {
        id: "s7",
        intent: "modal body visible with its copy",
        kind: "check",
        claim: {
          subject: { element: css(".modal-body"), attribute: "text" },
          predicate: "contains",
          value: "small modal",
        },
      },
      step("s8", "close it", { verb: "click", on: css("#closeSmallModal") }),
      {
        id: "s9",
        intent: "modal gone from the DOM",
        kind: "check",
        claim: { subject: { element: css(".modal-content") }, predicate: "notExists" },
      },
      step("s10", "progress bar page", {
        verb: "goto",
        value: lit("https://demoqa.com/progress-bar"),
      }),
      step("s11", "start it", { verb: "click", on: css("#startStopButton") }),
      {
        // The fill takes ~12s client-side; context.timeoutMs extends the
        // claim poll so the reset-button check waits out the animation.
        id: "s12",
        intent: "reset button appears at completion",
        kind: "check",
        claim: { subject: { element: css("#resetButton") }, predicate: "isVisible" },
        context: { timeoutMs: 20000 },
      },
      {
        id: "s13",
        intent: "bar reached 100%",
        kind: "check",
        claim: {
          subject: { element: css(".progress-bar"), attribute: "text" },
          predicate: "contains",
          value: "100%",
        },
      },
    ],
  },
  {},
);
