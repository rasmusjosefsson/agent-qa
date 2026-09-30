import { runEdgeGolden } from "./edge-pages-lib";

// pta-tc02 — negative login ×2: wrong password then wrong username both
// render the inline error on #error without navigating away.
await runEdgeGolden(
  "pta-tc02",
  "login failure — wrong password and wrong username show inline errors",
  "https://practicetestautomation.com/practice-test-login/",
  "#username",
  async (g) => {
    await g.openPage();
    await g.fillSelector("#username", "student", "valid username");
    await g.fillSelector("#password", "wrongpass", "wrong password");
    await g.clickSelector("#submit", "submit");
    await g.waitSelectorText(
      "#error",
      "Your password is invalid",
      "password error shown",
    );
    await g.assertUrlContains("practice-test-login", "still on the form");
    await g.fillSelector("#username", "baduser", "wrong username");
    await g.fillSelector("#password", "Password123", "valid password");
    await g.clickSelector("#submit", "submit again");
    await g.waitSelectorText(
      "#error",
      "Your username is invalid",
      "username error shown",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
