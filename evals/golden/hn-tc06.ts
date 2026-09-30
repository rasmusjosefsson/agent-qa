#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// /login — a real credential form. Type both fields and assert the live
// values landed; no submit (no account is created).
await runEdgeGolden(
  "tc06",
  "hn login — credential form fields accept input",
  "https://news.ycombinator.com/login",
  "input[name='acct']",
  async (b) => {
    await b.openPage();
    await b.fillSelector("input[name='acct']", "qa-probe", "enter username");
    await b.fillSelector("input[name='pw']", "not-a-real-password", "enter password");
    await b.assertElementAttribute(
      "input[name='acct']",
      "value",
      "equals",
      "qa-probe",
      "username field holds the typed value",
    );
    await b.assertElementPresent(
      "input[type='submit'][value='login']",
      "login submit rendered",
    );
    await b.assertElementPresent(
      "input[type='submit'][value='create account']",
      "create-account submit rendered",
    );
  },
  { label: "hn" },
);
