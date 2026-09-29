#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// thinking-tester-contact-list — auth round-trip: bad login shows the
// inline error, then signup → logout → log back in with the SAME
// templated email ({{vars._unique}} resolves once per run, so the login
// replay uses the account the signup step just minted).
await runEdgeGolden(
  "tc02", "contact-list — bad login then signup/login round-trip",
  "https://thinking-tester-contact-list.herokuapp.com/", "#email",
  async (b) => {
    const stamp = Date.now();
    const email = `golden-${stamp}@example.com`;
    await b.openPage();
    await b.fillSelector("#email", "not-a-user@example.com", "type a wrong email");
    await b.fillSelector("#password", "wrong-pass", "type a wrong password");
    await b.clickSelector("#submit", "submit the bad login");
    await b.waitSelectorText("#error", "Incorrect username or password", "login error surfaced");
    // Nav links bind in a post-commit effect — a click right after render
    // is a silent no-op.
    await b.waitMs(800, "let the SPA wire its handlers");
    await b.clickSelector("#signup", "open the signup form");
    await b.waitSelector("#firstName", "signup form rendered");
    await b.fillSelector("#firstName", "Golden", "type first name");
    await b.fillSelector("#lastName", "Runner", "type last name");
    await b.fillSelectorReplayValue("#email", email, "golden-{{vars._unique}}@example.com", "type a fresh signup email");
    await b.fillSelector("#password", "golden-pass-123", "type a password");
    await b.clickSelector("#submit", "submit the signup");
    await b.waitSelector("#add-contact", "signed up and landed on the list");
    await b.waitMs(700, "let the SPA wire its handlers");
    await b.clickSelector("#logout", "log out");
    await b.waitSelector("#email", "back on the login form");
    await b.fillSelectorReplayValue("#email", email, "golden-{{vars._unique}}@example.com", "type the signup email again");
    await b.fillSelector("#password", "golden-pass-123", "type the password");
    await b.clickSelector("#submit", "log in");
    await b.waitSelector("#add-contact", "logged back in to the list");
    await b.assertElementPresent("#logout", "logout control rendered");
  },
  { label: "cl" },
);
