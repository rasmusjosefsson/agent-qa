import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: accessibility gating — runs the axe-core audit via the a11y
// claim. /checkboxes has a known critical `label` violation (the checkboxes
// have no labels), so the scenario pins it: the debt is documented, counted,
// and the claim fails if MORE critical violations appear.
await runEdgeGolden(
  "tc27",
  "a11y audit pins the known critical label violation",
  "/checkboxes",
  "form#checkboxes",
  async (b) => {
    await b.openPage();
    await b.a11yAudit(
      { rule: "label", impact: "critical" },
      "exists",
      undefined,
      "the known unlabeled-checkbox debt is present",
    );
    await b.a11yAudit(
      { impact: "critical" },
      "countEquals",
      1,
      "exactly one critical violation — growth fails the gate",
    );
    await b.a11yAudit(
      { rule: "marquee" },
      "notExists",
      undefined,
      "no marquee-rule findings",
    );
  },
);
