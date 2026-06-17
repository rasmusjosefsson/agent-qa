#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc08", "Automation Exercise TC08 products and detail page", async (golden) => {
  await golden.openHome();
  await golden.openUrl("https://www.automationexercise.com/products", "open products page");
  await golden.waitUrl("/products", "products URL is reached");
  await golden.waitText("ALL PRODUCTS", "all products heading is visible");
  await golden.waitSelector('a[href^="/product_details/"]', "product detail links are visible");
  await golden.domClickSelector('a[href^="/product_details/"]', "open first product detail");
  await golden.assertLiveCondition(
    `(() => { const text = document.body.innerText; if (!location.pathname.includes('/product_details/')) throw new Error('not on detail page: ' + location.href); for (const marker of ['Category:', 'Availability:', 'Condition:', 'Brand:']) if (!text.includes(marker)) throw new Error('missing marker ' + marker); return true; })()`,
    "product detail markers are visible",
  );
  await golden.waitUrl("/product_details/", "product detail URL is reached");
  await golden.waitText("Availability:", "availability is visible");
  await golden.waitText("Condition:", "condition is visible");
  await golden.waitText("Brand:", "brand is visible");
});
