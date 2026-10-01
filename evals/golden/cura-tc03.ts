#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// katalon-demo-cura — tc03: book → History sidebar shows the booking →
// logout returns to the landing. Exercises the hamburger-menu nav, a
// server-side persisted record assertion, and the session teardown path.
await runEdgeGolden(
  "tc03", "cura — booking lands on History, logout resets",
  "https://katalon-demo-cura.herokuapp.com/", "#btn-make-appointment",
  async (b) => {
    await b.openPage();
    await b.clickSelector("#btn-make-appointment", "open Make Appointment");
    await b.waitSelector("#txt-username", "login form rendered");
    await b.fillSelector("#txt-username", "John Doe", "fill the demo username");
    await b.fillSelector("#txt-password", "ThisIsNotAPassword", "fill the demo password");
    await b.clickSelector("#btn-login", "submit the login form");
    await b.waitSelector("#combo_facility", "appointment form rendered");
    await b.selectOption("#combo_facility", "Tokyo CURA Healthcare Center", "pick the Tokyo facility");
    // Force-clicks at this scroll depth — agent-browser's live click
    // misfires ~500px down the page (see tc02 for the mechanism).
    await b.clickSelectorForce('input[name="programs"][value="Medicare"]', "choose the Medicare program");
    // bootstrap-datepicker: the calendar pick commits a valid value where
    // typing leaves the required field empty.
    await b.clickSelectorForce("#txt_visit_date", "open the visit-date calendar");
    await b.waitSelector(".datepicker-days td.day", "calendar grid rendered");
    await b.clickSelectorForce(".datepicker-days td.day:not(.old):not(.new)", "pick a day of the month");
    await b.fillSelector("#txt_comment", "history row check", "write a comment");
    await b.clickSelectorForce("#btn-book-appointment", "book the appointment");
    await b.waitSelector("#facility", "confirmation page rendered");
    await b.clickSelectorForce("#menu-toggle", "open the sidebar menu");
    await b.waitSelector(".sidebar-nav a[href*='history.php#history']", "sidebar History link visible");
    await b.clickSelectorForce("a[href*='history.php#history']", "open the History page");
    await b.waitSelector("#history", "history page rendered");
    await b.waitSelectorText("#history", "Tokyo", "a history row shows the booking");
    await b.assertElementAttribute("#history", "text", "contains", "Tokyo CURA Healthcare Center", "history row names the facility");
    await b.clickSelectorForce("#menu-toggle", "reopen the sidebar menu");
    await b.waitSelector(".sidebar-nav a[href*='logout']", "sidebar Logout link visible");
    await b.clickSelectorForce("a[href*='logout']", "log out");
    await b.waitSelector("#btn-make-appointment", "logout returns to the landing hero");
    await b.assertPageError(true, "countEquals", 0, "no page errors");
  },
  { label: "cura" },
);
