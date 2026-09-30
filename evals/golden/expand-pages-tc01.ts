import { runAuthoredGolden } from "./edge-pages-lib";

// expandtesting misc pages — dynamic table renders named process rows, and
// infinite-scroll grows after pressing End.
const css = (value: string) => ({
  raw: { kind: "css", value },
  reason: "authored expandtesting golden",
});
const lit = (literal: unknown) => ({ from: "literal", literal });
const step = (id: string, intent: string, extra: Record<string, unknown>) => ({
  id,
  intent,
  kind: "do",
  ...extra,
});

await runAuthoredGolden(
  "expand-pages-tc01",
  "expandtesting: dynamic table + infinite scroll",
  {
    steps: [
      step("s0", "dynamic table page", {
        verb: "goto",
        value: lit("https://practice.expandtesting.com/dynamic-table"),
      }),
      {
        id: "s1",
        intent: "table rendered with the process rows",
        kind: "check",
        claim: {
          subject: { element: css("table.table tbody"), attribute: "text" },
          predicate: "contains",
          value: "Chrome",
        },
      },
      {
        id: "s2",
        intent: "CPU column present",
        kind: "check",
        claim: {
          subject: { element: css("table.table thead"), attribute: "text" },
          predicate: "contains",
          value: "CPU",
        },
      },
      step("s3", "infinite scroll page", {
        verb: "goto",
        value: lit("https://practice.expandtesting.com/infinite-scroll"),
      }),
      {
        id: "s4",
        intent: "only the initial content batch before scrolling",
        kind: "check",
        claim: { subject: { element: css("div.jscroll-added:nth-of-type(2)") }, predicate: "notExists" },
      },
      step("s5", "scrollIntoView the footer (bottom of page)", {
        verb: "scrollTo",
        on: css("footer"),
      }),
      {
        id: "s6",
        intent: "a second batch was appended by the scroller",
        kind: "check",
        claim: { subject: { element: css("div.jscroll-added:nth-of-type(2)") }, predicate: "exists" },
      },
    ],
  },
  {},
);
