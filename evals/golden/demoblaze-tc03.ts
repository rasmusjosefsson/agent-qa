#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// demoblaze.com — signup + login round-trip on the real auth endpoint.
// The username is minted live but recorded as {{vars._unique}} so replay
// signs up a fresh user every run.
await runEdgeGolden(
  "tc03", "demoblaze — signup and login",
  "https://www.demoblaze.com/", "#signin2",
  async (b) => {
    const stamp = Date.now();
    const user = `golden-user-${stamp}`;
    await b.openPage();
    await b.clickSelector("#signin2", "open the sign up modal");
    await b.waitSelector("#sign-username", "signup modal rendered");
    await b.fillSelectorReplayValue("#sign-username", user, "golden-user-{{vars._unique}}", "type a fresh username");
    await b.fillSelector("#sign-password", "golden-pass-123", "type a password");
    await b.clickSelector('#signInModal button[onclick="register()"]', "submit the signup");
    await b.assertDialogText("Sign up successful", "signup answered its alert");
    await b.dialogAccept("accept the signup alert");
    await b.assertDialogClosed("signup alert dismissed");
    // demoblaze keeps the sign-up modal open after success — dismiss it
    // before the log in control is clickable.
    await b.clickSelector("#signInModal button.close", "close the sign up modal");
    await b.waitMs(800, "let the bootstrap modal fade out");
    await b.clickSelector("#login2", "open the log in modal");
    await b.waitSelector("#loginusername", "login modal rendered");
    await b.fillSelectorReplayValue("#loginusername", user, "golden-user-{{vars._unique}}", "type the same username");
    await b.fillSelector("#loginpassword", "golden-pass-123", "type the password");
    await b.clickSelector('#logInModal button[onclick="logIn()"]', "submit the login");
    await b.waitSelectorText("#nameofuser", "Welcome", "logged-in banner appeared");
  },
  { label: "demoblaze", keepDialogs: true },
);
import { runEdgeGolden } from "./edge-pages-lib";

// demoblaze-tc03 — cart persists across a full page reload (the server keys
// the cart on the anonymous `user` cookie). Exercises session persistence,
// a mid-scenario reload, and delete cleanup.
await runEdgeGolden(
  "db-tc03",
  "cart survives reload — anonymous session cookie, row still listed",
  "https://www.demoblaze.com/index.html",
  ".card-title a",
  async (g) => {
    await g.openPage();
    await g.clickSelector(".card-title a", "open first product");
    await g.waitSelector(".btn-success", "product page renders");
    await g.clickSelector("a.btn-success", "add to cart");
    await g.waitMs(2000, "addtocart POST settles before the alert");
    await g.assertDialogText("Product added", "add-to-cart alert text");
    await g.dialogAccept("accept product-added alert");
    await g.reload("reload mid-session");
    await g.clickSelector("#cartur", "open cart after reload");
    await g.waitSelector("#tbodyid .success", "cart row survives the reload");
    await g.clickSelector("#tbodyid .success td a", "delete the item");
    await g.waitSelectorAbsent("#tbodyid .success", "cart empty after delete");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
  { keepDialogs: true },
);
