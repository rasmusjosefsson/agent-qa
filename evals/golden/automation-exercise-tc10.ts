#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc10", "Automation Exercise TC10 subscription on home page", async (golden) => {
  const email = `agent-qa-subscribe-${Date.now()}@example.com`;
  await golden.openHome();
  await golden.waitSelectorText("#slider", "AutomationExercise", "home page is visible");
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#footer'); if (!el) throw new Error('footer missing'); el.scrollIntoView({ block: 'center' }); return true; })()`,
    "footer is reachable",
  );
  await golden.waitSelectorText("#footer .single-widget h2", "Subscription", "subscription heading is visible");
  await golden.fillSelector("#susbscribe_email", email, "enter disposable subscription email");
  await golden.domClickSelector("#subscribe", "submit subscription form");
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#success-subscribe:not(.hide)'); if (!el) throw new Error('success alert hidden'); if (!el.textContent.includes('You have been successfully subscribed!')) throw new Error('success text missing'); return true; })()`,
    "subscription success message is visible",
  );
  await golden.waitSelector("#success-subscribe", "subscription success container exists");
});
