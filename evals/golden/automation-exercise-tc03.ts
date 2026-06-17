#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc03", "Automation Exercise TC03 invalid login shows error", async (golden) => {
  await golden.openHome();
  await golden.openUrl("https://www.automationexercise.com/login", "open login page");
  await golden.waitSelector('input[data-qa="login-email"]', "login email field is visible");
  await golden.fillSelector('input[data-qa="login-email"]', "invalid-user@example.com", "enter invalid email");
  await golden.fillSelector('input[data-qa="login-password"]', "WrongPass!234", "enter invalid password");
  await golden.domClickSelector('button[data-qa="login-button"]', "submit invalid login");
  await golden.assertLiveCondition(
    `(() => { if (!document.body.innerText.includes('Your email or password is incorrect!')) throw new Error('invalid login error missing'); return true; })()`,
    "invalid login error is visible",
  );
  await golden.waitText("Your email or password is incorrect!", "invalid login error is replay-visible");
});
