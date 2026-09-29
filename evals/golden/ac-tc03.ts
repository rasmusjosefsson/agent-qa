import { runEdgeGolden } from "./edge-pages-lib";

// ac-tc03 — expected_conditions.html: triggers fire their target action
// after a random sleep between #min_wait and #max_wait seconds; pin both
// low for determinism, then drive the visibility toggle end-to-end.
await runEdgeGolden(
  "ac-tc03",
  "expected conditions — trigger reveals the hidden button, then click it",
  "https://play1.automationcamp.ir/expected_conditions.html",
  "#visibility_trigger",
  async (g) => {
    await g.openPage();
    // The trigger sleeps a random min..max seconds — pin the range tight.
    await g.fillSelector("#min_wait", "0", "min wait 0s");
    await g.fillSelector("#max_wait", "1", "max wait 1s");
    await g.clickSelector("#visibility_trigger", "trigger reveal");
    // `wait` on the selector resolves once the button is displayed.
    await g.waitSelector("#visibility_target", "hidden button appears");
    await g.clickSelector("#visibility_target", "Click Me once visible");
    await g.waitSelectorText(
      "body",
      "invisibility cloak",
      "popover confirms the click",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
