#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc06", "Automation Exercise TC06 contact us form", async (golden) => {
  await golden.openHome();
  await golden.domClickSelector('a[href="/contact_us"]', "open contact us page");
  await golden.waitUrl("/contact_us", "contact us URL is reached");
  await golden.waitText("GET IN TOUCH", "get in touch heading is visible");
  await golden.fillSelector('input[data-qa="name"]', "Agent QA", "enter contact name");
  await golden.fillSelector('input[data-qa="email"]', `agent-qa-contact-${Date.now()}@example.com`, "enter contact email");
  await golden.fillSelector('input[data-qa="subject"]', "Automation Exercise TC06", "enter contact subject");
  await golden.fillSelector('textarea[data-qa="message"]', "Contact form message", "enter contact message");
  await golden.uploadSelector('input[name="upload_file"]', "upload-valid.txt", "upload contact fixture");
  await golden.waitSelector('input[data-qa="submit-button"]', "contact submit button is available");
  await golden.assertLiveCondition(
    `(() => { const input = document.querySelector('input[name="upload_file"]'); if (!input || !Array.from(input.files || []).some((file) => file.name === 'upload-valid.txt')) throw new Error('contact upload fixture not selected'); return true; })()`,
    "contact upload fixture is selected",
  );
});
