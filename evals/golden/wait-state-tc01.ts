import { runAuthoredGolden } from "./edge-pages-lib";

// wait {locator,state} — element-condition waiting on the-internet's delayed
// DOM: /dynamic_loading/1 keeps #finish in the DOM but display:none (proves
// `hidden` resolves and `visible` blocks until the transition); then
// /dynamic_controls detaches #checkbox entirely (proves `detached`).
const css = (value: string) => ({
  raw: { kind: "css", value },
  reason: "authored wait-state golden",
});
const lit = (literal: unknown) => ({ from: "literal", literal });
const step = (id: string, intent: string, extra: Record<string, unknown>) => ({
  id,
  intent,
  kind: "do",
  ...extra,
});

await runAuthoredGolden(
  "wait-state-tc01",
  "wait locator states — hidden→visible transition + detached",
  {
    steps: [
      step("s0", "open dynamic_loading/1", {
        verb: "goto",
        value: lit("https://the-internet.herokuapp.com/dynamic_loading/1"),
      }),
      step("s1", "#finish already hidden (in DOM, display:none)", {
        verb: "wait",
        params: { locator: css("#finish"), state: "hidden", timeoutMs: 15000 },
      }),
      step("s2", "start the load", { verb: "click", on: css("#start button") }),
      step("s3", "wait for #finish to turn visible", {
        verb: "wait",
        params: { locator: css("#finish"), state: "visible", timeoutMs: 30000 },
      }),
      {
        id: "s4",
        intent: "loaded text rendered",
        kind: "check",
        claim: {
          subject: { element: css("#finish"), attribute: "text" },
          predicate: "contains",
          value: "Hello World",
        },
      },
      step("s5", "open dynamic_controls", {
        verb: "goto",
        value: lit("https://the-internet.herokuapp.com/dynamic_controls"),
      }),
      step("s6", "remove the checkbox", { verb: "click", on: css("#checkbox-example button") }),
      step("s7", "wait for #checkbox to detach", {
        verb: "wait",
        params: { locator: css("#checkbox"), state: "detached", timeoutMs: 30000 },
      }),
      {
        id: "s8",
        intent: "removal banner",
        kind: "check",
        claim: {
          subject: { element: css("#message"), attribute: "text" },
          predicate: "contains",
          value: "It's gone",
        },
      },
    ],
  },
  { label: "wait-state" },
);
