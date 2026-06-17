#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc25", "Automation Exercise TC25 scroll up with arrow", async (golden) => {
  await golden.openHome();
  await golden.assertLiveCondition(`(() => { document.querySelector('#footer').scrollIntoView({ block: 'center' }); return true; })()`, "scroll to footer");
  await golden.waitSelectorText("#footer .single-widget h2", "Subscription", "subscription footer is visible");
  await golden.domClickSelector("#scrollUp", "click scroll up arrow");
  await golden.waitText("Full-Fledged practice website for Automation Engineers", "top hero text is visible after arrow scroll");
});
