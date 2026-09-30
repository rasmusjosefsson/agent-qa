// bonigarcia-tc05 — web storage: seed localStorage/sessionStorage via
// do/state, then have the page render each store and claim the seeded
// value — the first golden exercising do/state's storage params.

import { runEdgeGolden } from "./edge-pages-lib.ts";

await runEdgeGolden(
  "tc05",
  "Bonigarcia — seed storage, render it, assert the values",
  "https://bonigarcia.dev/selenium-webdriver-java/web-storage.html",
  "#display-local",
  async (b) => {
    await b.openPage();
    await b.seedStorage("local", "aqKey", "aq-val-42", "seed a localStorage entry");
    await b.clickSelector("#display-local", "render localStorage");
    await b.waitSelectorText("#local-storage", "aq-val-42", "seeded value shown");

    await b.seedStorage("session", "aqSess", "sess-7", "seed a sessionStorage entry");
    await b.clickSelector("#display-session", "render sessionStorage");
    await b.waitSelectorText("#session-storage", "sess-7", "seeded session value shown");
    await b.assertStorage({ key: "aqKey" }, true, "the localStorage claim sees the seed");
  },
);
