#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc07", "Automation Exercise TC07 verify test cases page", async (golden) => {
  await golden.openHome();
  await golden.assertLiveCondition(
    `(() => { if (!document.body.innerText.includes('AutomationExercise')) throw new Error('home page marker missing'); return true; })()`,
    "home page is visible",
  );
  await golden.domClickSelector('a[href="/test_cases"]', "open test cases page");
  await golden.assertLiveCondition(
    `(() => { if (!location.pathname.includes('/test_cases')) throw new Error('not on test cases page: ' + location.href); if (!document.body.innerText.includes('Test Cases')) throw new Error('test cases marker missing'); return true; })()`,
    "test cases page is visible",
  );
  await golden.waitUrl("/test_cases", "test cases URL is reached");
  await golden.waitText("Test Cases", "test cases page text is visible");
});
