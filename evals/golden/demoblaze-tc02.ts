#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// demoblaze.com — category filter: the Phones/Laptops/Monitors rail all
// share id="itemc" (duplicate ids), so the selector keys off the onclick
// argument. Filtering is an XHR + re-render, exercised via text waits.
await runEdgeGolden(
  "tc02", "demoblaze — category filter",
  "https://www.demoblaze.com/", ".card-block",
  async (b) => {
    await b.openPage();
    await b.assertElementAttribute(".card-title a", "text", "contains", "Samsung galaxy s6", "all products view starts with phones+laptops+monitors");
    await b.clickSelector('a.list-group-item[onclick*="monitor"]', "filter to Monitors");
    await b.waitSelectorText("#tbodyid", "Apple monitor 24", "monitor items rendered");
    await b.assertElementAttribute("#tbodyid", "text", "contains", "ASUS Full HD", "second monitor item rendered");
    await b.clickSelector('a.list-group-item[onclick*="phone"]', "filter back to Phones");
    await b.waitSelectorText("#tbodyid", "Samsung galaxy s6", "phone items rendered again");
    await b.assertElementAttribute("#tbodyid", "text", "contains", "Nexus 6", "second phone item rendered");
  },
  { label: "demoblaze" },
);
