#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc18", "Automation Exercise TC18 view category products", async (golden) => {
  await golden.openHome();
  await golden.waitSelector("#accordian.category-products", "categories are visible in left sidebar");
  await golden.waitSelectorText(".left-sidebar", "Category", "category sidebar heading is visible");
  await golden.domClickSelector('#accordian a[href="#Women"]', "expand Women category");
  await golden.waitSelector('#Women.in a[href="/category_products/2"]', "Women Tops subcategory is visible");
  await golden.domClickSelector('#Women a[href="/category_products/2"]', "open Women Tops category");
  await golden.waitUrl("/category_products/2", "Women Tops category URL is reached");
  await golden.waitSelectorText(".features_items h2.title", "Women - Tops Products", "Women Tops category heading is visible");
  await golden.domClickSelector('#accordian a[href="#Men"]', "expand Men category");
  await golden.waitSelector('#Men.in a[href="/category_products/3"]', "Men Tshirts subcategory is visible");
  await golden.domClickSelector('#Men a[href="/category_products/3"]', "open Men Tshirts category");
  await golden.waitUrl("/category_products/3", "Men Tshirts category URL is reached");
  await golden.waitSelectorText(".features_items h2.title", "Men - Tshirts Products", "Men Tshirts category heading is visible");
});
