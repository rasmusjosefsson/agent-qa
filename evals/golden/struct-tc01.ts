import { runAuthoredGolden } from "./edge-pages-lib";

// Authored constructs — group/useTemplate/loop in one replay on saucedemo:
// template "login" expands the credential steps; a group wraps a loop over
// the four sort orders asserting the first inventory item each pass.
const css = (value: string) => ({
  raw: { kind: "css", value },
  reason: "authored structural golden",
});
const lit = (literal: unknown) => ({ from: "literal", literal });
const step = (id: string, intent: string, extra: Record<string, unknown>) => ({
  id,
  intent,
  kind: "do",
  ...extra,
});

await runAuthoredGolden(
  "struct-tc01",
  "saucedemo template login + grouped sort loop",
  {
    templates: {
      login: {
        steps: [
          step("l1", "username", { verb: "type", on: css("#user-name"), value: lit("standard_user") }),
          step("l2", "password", { verb: "type", on: css("#password"), value: lit("secret_sauce") }),
          step("l3", "log in", { verb: "click", on: css("#login-button") }),
          {
            id: "l4",
            intent: "inventory rendered",
            kind: "check",
            claim: { subject: { element: css(".inventory_list") }, predicate: "isVisible" },
          },
        ],
      },
    },
    steps: [
      step("s0", "navigation", { verb: "goto", value: lit("https://www.saucedemo.com/") }),
      {
        id: "s1",
        intent: "login form rendered",
        kind: "check",
        claim: { subject: { element: css("input#login-button") }, predicate: "isVisible" },
      },
      step("s2", "log in via template", { verb: "useTemplate", params: { template: "login" } }),
      step("s3", "sort sweep", {
        verb: "group",
        params: {
          steps: [
            step("g0", "each sort order", {
              verb: "loop",
              params: {
                over: lit(["lohi", "hilo", "az", "za"]),
                as: "opt",
                do: [
                  step("d0", "sort by {{vars.opt}}", {
                    verb: "select",
                    on: css("select.product_sort_container"),
                    value: lit("{{vars.opt}}"),
                  }),
                  {
                    id: "d1",
                    intent: "inventory still rendered for {{vars.opt}}",
                    kind: "check",
                    claim: {
                      subject: { element: css(".inventory_item") },
                      predicate: "isVisible",
                    },
                  },
                ],
              },
            }),
          ],
        },
      }),
      {
        id: "s4",
        intent: "loop ended on za → first item is the Z-side product",
        kind: "check",
        claim: {
          subject: {
            element: css(".inventory_item:first-child .inventory_item_name"),
            attribute: "text",
          },
          predicate: "equals",
          value: "Test.allTheThings() T-Shirt (Red)",
        },
      },
    ],
  },
  { label: "struct" },
);
