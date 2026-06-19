#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc21", "Automation Exercise TC21 add review on product", async (golden) => {
  await golden.openUrl("https://www.automationexercise.com/products", "open products page");
  await golden.waitText("ALL PRODUCTS", "all products heading is visible");
  await golden.domClickSelector('a[href="/product_details/1"]', "open first product detail");
  await golden.waitText("Write Your Review", "write review section is visible");
  await golden.fillSelector("#name", "Agent QA", "enter review name");
  await golden.fillSelector("#email", `agent-qa-review-${Date.now()}@example.com`, "enter review email");
  await golden.fillSelector("#review", "Great practice product review", "enter review text");
  await golden.domClickSelector("#button-review", "submit product review");
  await golden.waitText("Thank you for your review.", "review success message is visible");
});
