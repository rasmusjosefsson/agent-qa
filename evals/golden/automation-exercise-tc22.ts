#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc22", "Automation Exercise TC22 add recommended item to cart", async (golden) => {
  await golden.openHome();
  await golden.assertLiveCondition(`(() => { const el = document.querySelector('.recommended_items'); if (!el) throw new Error('recommended items missing'); el.scrollIntoView({ block: 'center' }); return true; })()`, "recommended items are reachable");
  await golden.waitText("RECOMMENDED ITEMS", "recommended items heading is visible");
  await golden.domClickSelector('.recommended_items a.add-to-cart[data-product-id]', "add recommended product to cart");
  await golden.waitSelector("#cartModal.show", "added-to-cart modal is visible");
  await golden.domClickSelector('#cartModal.show a[href="/view_cart"]', "open cart from modal");
  await golden.waitUrl("/view_cart", "cart URL is reached");
  await golden.waitSelector("#cart_info tbody tr[id^='product-']", "recommended product is displayed in cart");
});
