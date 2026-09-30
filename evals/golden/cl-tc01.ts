#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// thinking-tester-contact-list — the classic CRUD app: sign up with a
// fresh account (template fill → unique per replay), land on the list,
// add a contact, open its details, verify the values round-tripped.
await runEdgeGolden(
  "tc01", "contact-list — signup, add contact, open details",
  "https://thinking-tester-contact-list.herokuapp.com/addUser", "#firstName",
  async (b) => {
    const stamp = Date.now();
    const email = `golden-${stamp}@example.com`;
    await b.openPage();
    await b.fillSelector("#firstName", "Golden", "type first name");
    await b.fillSelector("#lastName", "Runner", "type last name");
    await b.fillSelectorReplayValue("#email", email, "golden-{{vars._unique}}@example.com", "type a fresh signup email");
    await b.fillSelector("#password", "golden-pass-123", "type a password");
    await b.clickSelector("#submit", "submit the signup");
    await b.waitSelector("#add-contact", "contact list rendered");
    // The list renders before the SPA's click handlers attach — DOM
    // presence alone doesn't mean the router is live yet.
    await b.waitMs(700, "let the SPA wire its handlers");
    await b.clickSelector("#add-contact", "open the add-contact form");
    await b.waitSelector("#birthdate", "add-contact form rendered");
    await b.fillSelector("#firstName", "Ada", "type the contact's first name");
    await b.fillSelector("#lastName", "Lovelace", "type the contact's last name");
    await b.fillSelector("#email", "ada.lovelace@example.com", "type the contact's email");
    await b.fillSelector("#phone", "5551234567", "type the contact's phone");
    await b.fillSelector("#birthdate", "1815-12-10", "type the contact's birthdate");
    await b.clickSelector("#submit", "submit the contact");
    await b.waitSelectorText(".contactTable", "Ada Lovelace", "contact row appears in the list");
    await b.waitMs(500, "let the row listener attach");
    await b.clickSelector(".contactTableBodyRow", "open the contact row");
    await b.waitSelector("#edit-contact", "contact details rendered");
    await b.assertElementAttribute("#firstName", "text", "equals", "Ada", "details show the first name");
    await b.assertElementAttribute("#email", "text", "equals", "ada.lovelace@example.com", "details show the email");
    await b.clickSelector("#return", "back to the contact list");
    await b.waitSelector("#add-contact", "back on the list");
    // Rows arrive via fetch — their presence means data loaded, but the
    // logout handler binds in a post-commit effect; give it a beat.
    await b.waitSelectorText(".contactTable", "Ada Lovelace", "list re-rendered with the contact");
    await b.waitMs(800, "settle after the list render");
    await b.clickSelector("#logout", "log out");
    await b.waitSelector("#email", "back on the login form");
  },
  { label: "cl" },
);
