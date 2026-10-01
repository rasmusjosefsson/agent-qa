#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// katalon-demo-cura — tc02: the full booking journey. Login → select a
// facility → readmission checkbox → Medicaid radio → visit date → comment
// → Book → the confirmation page echoes every submitted value back.
// The date input is a bootstrap-datepicker; typing the date directly is
// the supported path and exercises fill-on-datepicker round-trip.
await runEdgeGolden(
  "tc02", "cura — book an appointment end to end",
  "https://katalon-demo-cura.herokuapp.com/", "#btn-make-appointment",
  async (b) => {
    await b.openPage();
    await b.clickSelector("#btn-make-appointment", "open Make Appointment");
    await b.waitSelector("#txt-username", "login form rendered");
    await b.fillSelector("#txt-username", "John Doe", "fill the demo username");
    await b.fillSelector("#txt-password", "ThisIsNotAPassword", "fill the demo password");
    await b.clickSelector("#btn-login", "submit the login form");
    await b.waitSelector("#combo_facility", "appointment form rendered");
    await b.selectOption("#combo_facility", "Hongkong CURA Healthcare Center", "pick the Hongkong facility");
    await b.checkSelector("#chk_hospotal_readmission", "apply for hospital readmission");
    // Force-clicks for the controls deep in this page: the site scrolls
    // ~500px to reach them and agent-browser's live click misses at that
    // offset (a real-user DOM click is what replay performs anyway).
    await b.clickSelectorForce('input[name="programs"][value="Medicaid"]', "choose the Medicaid program");
    // The visit-date field is a bootstrap-datepicker: typing leaves the
    // widget's committed value empty so the required field blocks submit.
    // The real-user path is picking a day from the calendar grid.
    await b.clickSelectorForce("#txt_visit_date", "open the visit-date calendar");
    await b.waitSelector(".datepicker-days td.day", "calendar grid rendered");
    await b.clickSelectorForce(".datepicker-days td.day:not(.old):not(.new)", "pick a day of the month");
    await b.fillSelector("#txt_comment", "first visit — replay check", "write a comment");
    await b.clickSelectorForce("#btn-book-appointment", "book the appointment");
    await b.waitSelector("#facility", "confirmation page rendered");
    await b.assertElementAttribute("#facility", "text", "equals", "Hongkong CURA Healthcare Center", "confirmation echoes the facility");
    await b.assertElementAttribute("#hospital_readmission", "text", "equals", "Yes", "confirmation echoes readmission");
    await b.assertElementAttribute("#program", "text", "equals", "Medicaid", "confirmation echoes the program");
    await b.assertElementAttribute("#visit_date", "text", "matches", "\\d{2}/\\d{2}/\\d{4}", "confirmation echoes the picked date");
    await b.assertElementAttribute("#comment", "text", "contains", "replay check", "confirmation echoes the comment");
    await b.assertPageError(true, "countEquals", 0, "no page errors");
  },
  { label: "cura" },
);
