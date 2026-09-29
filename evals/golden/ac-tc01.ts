import { runEdgeGolden } from "./edge-pages-lib";

// ac-tc01 — play1.automationcamp.ir login: admin/admin (the page prints the
// hint) → lands on the pizza order form (order_submit.html).
await runEdgeGolden(
  "ac-tc01",
  "login — admin/admin lands on the pizza order form",
  "https://play1.automationcamp.ir/login.html",
  "input#user",
  async (g) => {
    await g.openPage();
    await g.fillSelector("input#user", "admin", "username");
    await g.fillSelector("input#password", "admin", "password");
    await g.clickSelector("button#login", "log in");
    await g.waitSelectorText("body", "Dinesh's Pizza House", "pizza form renders");
    await g.assertUrlContains("order_submit", "order page url");
    await g.assertElementPresent("input[type='radio']", "pizza options render");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
