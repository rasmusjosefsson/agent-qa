#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc26", "Automation Exercise TC26 scroll up without arrow", async (golden) => {
  await golden.openHome();
  await golden.assertLiveCondition(`(() => { document.querySelector('#footer').scrollIntoView({ block: 'center' }); return true; })()`, "scroll to footer");
  await golden.waitSelectorText("#footer .single-widget h2", "Subscription", "subscription footer is visible");
  await golden.assertLiveCondition(`(() => { window.scrollTo(0, 0); return true; })()`, "scroll to top without arrow");
  await golden.waitText("Full-Fledged practice website for Automation Engineers", "top hero text is visible after manual scroll");
});
