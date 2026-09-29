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
