import { runEdgeGolden } from "./edge-pages-lib";

// pta-tc01 — positive login: student/Password123 → success page with
// "Logged In Successfully" + the congratulations copy + Log out button.
await runEdgeGolden(
  "pta-tc01",
  "login success — student lands on the logged-in page",
  "https://practicetestautomation.com/practice-test-login/",
  "#username",
  async (g) => {
    await g.openPage();
    await g.fillSelector("#username", "student", "username");
    await g.fillSelector("#password", "Password123", "password");
    await g.clickSelector("#submit", "submit login");
    await g.waitSelectorText(
      "h1",
      "Logged In Successfully",
      "success heading",
    );
    await g.waitSelectorText(
      "body",
      "Congratulations",
      "congratulations copy",
    );
    await g.assertElementPresent(
      "a[href*='practice-test-login'], .wp-block-button a",
      "Log out link renders",
    );
    await g.assertUrlContains("logged-in-successfully", "success url");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
