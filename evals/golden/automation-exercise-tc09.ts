#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc09", "Automation Exercise TC09 search product", async (golden) => {
  await golden.openHome();
  await golden.openUrl("https://www.automationexercise.com/products", "open products page");
  await golden.waitUrl("/products", "products URL is reached");
  await golden.waitText("ALL PRODUCTS", "all products heading is visible");
  await golden.waitSelector("#search_product", "search input is visible");
  await golden.fillSelector("#search_product", "jeans", "enter product search keyword");
  await golden.domClickSelector("#submit_search", "submit product search");
  await golden.assertLiveCondition(
    `(() => { const text = document.body.innerText; if (!text.includes('SEARCHED PRODUCTS')) throw new Error('searched products heading missing'); if (!text.toLowerCase().includes('jeans')) throw new Error('jeans result missing'); return true; })()`,
    "searched products are visible",
  );
  await golden.waitText("SEARCHED PRODUCTS", "searched products heading is visible");
  await golden.waitSelectorText(".features_items", "Soft Stretch Jeans", "matching jeans result is visible");
});
