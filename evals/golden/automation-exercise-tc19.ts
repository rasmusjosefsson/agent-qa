#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc19", "Automation Exercise TC19 view brand products", async (golden) => {
  await golden.openHome();
  await golden.openUrl("https://www.automationexercise.com/products", "open products page");
  await golden.waitUrl("/products", "products URL is reached");
  await golden.waitSelectorText(".brands_products", "Brands", "brands sidebar is visible");
  await golden.domClickSelector('a[href="/brand_products/Polo"]', "open Polo brand products");
  await golden.waitUrl("/brand_products/Polo", "Polo brand URL is reached");
  await golden.waitSelectorText(".features_items h2.title", "Brand - Polo Products", "Polo brand products heading is visible");
  await golden.waitSelector('.features_items a[href^="/product_details/"]', "Polo brand products are visible");
  await golden.domClickSelector('a[href="/brand_products/H&M"]', "open H&M brand products");
  await golden.waitUrl("/brand_products/H&M", "H&M brand URL is reached");
  await golden.waitSelectorText(".features_items h2.title", "Brand - H&M Products", "H&M brand products heading is visible");
  await golden.waitSelector('.features_items a[href^="/product_details/"]', "H&M brand products are visible");
});
