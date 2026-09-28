#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc04", "Edge — redirect chain followed and asserted",
  "/redirector", "#redirect",
  async (b) => {
    await b.openPage();
    await b.clickSelector("#redirect", "follow the redirect link");
    await b.waitSelector('a[href="status_codes/200"]', "status-code listing rendered");
    await b.assertUrlContains("status_codes", "redirected to the status-code listing");
    await b.clickSelector('a[href="status_codes/200"]', "open the 200 page");
    await b.assertUrlContains("status_codes/200", "landed on the 200 page");
    await b.assertElementText("h3", "Status Codes", "status code page heading");
  },
);
