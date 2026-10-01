#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// katalon-demo-cura.herokuapp.com — the classic appointment-booking demo.
// tc01: landing → Make Appointment → login → the appointment form renders.
// The demo creds are printed on the login page itself (John Doe /
// ThisIsNotAPassword) — they are fixture values, not secrets.
await runEdgeGolden(
  "tc01", "cura — landing to appointment form",
  "https://katalon-demo-cura.herokuapp.com/", "#btn-make-appointment",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent("#btn-make-appointment", "hero CTA rendered");
    await b.clickSelector("#btn-make-appointment", "open Make Appointment");
    await b.waitSelector("#txt-username", "login form rendered");
    await b.assertUrlContains("profile.php#login", "Make Appointment routes to login");
    await b.fillSelector("#txt-username", "John Doe", "fill the demo username");
    await b.fillSelector("#txt-password", "ThisIsNotAPassword", "fill the demo password");
    await b.clickSelector("#btn-login", "submit the login form");
    await b.waitSelector("#combo_facility", "appointment form rendered");
    await b.assertUrlContains("#appointment", "login lands on the appointment section");
    await b.assertElementCount("#combo_facility option", "equals", 3, "facility select offers all centers");
    await b.assertPageError(true, "countEquals", 0, "no page errors");
  },
  { label: "cura" },
);
